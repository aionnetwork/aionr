/*******************************************************************************
 * Copyright (c) 2018-2019 Aion foundation.
 *
 *     This file is part of the aion network project.
 *
 *     The aion network project is free software: you can redistribute it
 *     and/or modify it under the terms of the GNU General Public License
 *     as published by the Free Software Foundation, either version 3 of
 *     the License, or any later version.
 *
 *     The aion network project is distributed in the hope that it will
 *     be useful, but WITHOUT ANY WARRANTY; without even the implied
 *     warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
 *     See the GNU General Public License for more details.
 *
 *     You should have received a copy of the GNU General Public License
 *     along with the aion network project source files.
 *     If not, see <https://www.gnu.org/licenses/>.
 *
 ******************************************************************************/

use std::sync::Arc;
use std::time::SystemTime;

use block::Block;
use client::{BlockId, BlockChainClient};
use aion_types::H256;
use bytes::BufMut;
use rlp::{RlpStream, UntrustedRlp};
use p2p::{ChannelBuffer, Mgr};
use sync::action::Action;
use sync::storage::SyncStorage;
use sync::wrappers::{HeadersWrapper, BlocksWrapper};
use header::Header;

use super::{channel_buffer_template,channel_buffer_template_with_version};

const HASH_LEN: usize = 32;

pub fn sync_bodies(p2p: Mgr, storage: Arc<SyncStorage>) {
    // Get all downloaded headers
    let mut headers_wrappers: Vec<HeadersWrapper> = Vec::new();
    let mut downloaded_headers = storage.downloaded_headers().lock();
    while let Some(headers_wrapper) = downloaded_headers.pop_front() {
        headers_wrappers.push(headers_wrapper);
    }
    drop(downloaded_headers);

    // For each batch of downloaded headers, try to get bodies from the corresponding node
    for headers_wrapper in headers_wrappers {
        let mut hashes: Vec<u8> = Vec::new(); // headers' hashes to request bodies
        let mut headers_requested: Vec<Header> = Vec::new(); // headers request record
        for header in &headers_wrapper.headers {
            let hash = header.hash();
            hashes.put_slice(&hash);
            headers_requested.push(header.clone());
        }

        if hashes.len() == 0 {
            continue;
        }

        let node_hash = headers_wrapper.node_hash;
        let mut headers_with_bodies_requested = storage.headers_with_bodies_requested().lock();
        if !headers_with_bodies_requested.contains_key(&node_hash) {
            drop(headers_with_bodies_requested);
            if send(p2p.clone(), node_hash.clone(), hashes) {
                let mut headers_wrapper_record = headers_wrapper.clone();
                headers_wrapper_record.timestamp = SystemTime::now();
                headers_wrapper_record.headers.clear();
                headers_wrapper_record.headers.extend(headers_requested);
                let mut headers_with_bodies_requested =
                    storage.headers_with_bodies_requested().lock();
                headers_with_bodies_requested.insert(node_hash, headers_wrapper_record);
            }
        }
    }
}

pub fn send(p2p: Mgr, hash: u64, hashes: Vec<u8>) -> bool {
    trace!(target: "sync", "bodies/send req");
    let mut cb = channel_buffer_template(Action::BODIESREQ.value());
    cb.body = hashes;
    cb.head.len = cb.body.len() as u32;
    p2p.send(hash, cb)
}

pub fn receive_req(p2p: Mgr, hash: u64, client: Arc<BlockChainClient>, cb_in: ChannelBuffer) {
    trace!(target: "sync", "bodies/receive_req");

    // check channelbuffer len
    if cb_in.head.len == 0 {
        debug!(target: "sync", "bodies req channelbuffer is empty" );
        return;
    }
    if cb_in.head.len as usize % HASH_LEN != 0 {
        debug!(target: "sync", "bodies res channelbuffer is invalid" );
        return;
    }

    let mut res = channel_buffer_template_with_version(cb_in.head.ver, Action::BODIESRES.value());

    let mut res_body = Vec::new();
    let hash_count = cb_in.head.len / HASH_LEN as u32;
    let mut rest = cb_in.body.as_slice();
    let mut data = Vec::new();
    let mut body_count = 0;
    for _i in 0..hash_count {
        let (hash, next) = rest.split_at(HASH_LEN);

        match client.block_body(BlockId::Hash(H256::from(hash))) {
            Some(bb) => {
                data.append(&mut bb.into_inner());
                body_count += 1;
            }
            None => {}
        }

        rest = next;
    }

    if body_count > 0 {
        let mut rlp = RlpStream::new_list(body_count);
        rlp.append_raw(&data, body_count);
        res_body.put_slice(rlp.as_raw());
    }

    res.body.put_slice(res_body.as_slice());
    res.head.len = res.body.len() as u32;

    p2p.update_node(&hash);
    p2p.send(hash, res);
}

pub fn receive_res(p2p: Mgr, node_hash: u64, cb_in: ChannelBuffer, storage: Arc<SyncStorage>) {
    trace!(target: "sync", "bodies/receive_res");

    // end if no body
    if cb_in.body.len() <= 0 {
        return;
    }

    // get bodies request records. End if no record found
    let headers_wrapper_record = match storage.headers_with_bodies_requested_for_node(&node_hash) {
        Some(headers_wrapper) => headers_wrapper,
        None => {
            return;
        }
    };
    let headers = headers_wrapper_record.headers;
    if headers.is_empty() {
        return;
    }

    // parse bodies
    let rlp = UntrustedRlp::new(cb_in.body.as_slice());
    let mut bodies = Vec::new();
    for block_bodies in rlp.iter() {
        for block_body in block_bodies.iter() {
            let mut transactions = Vec::new();
            if !block_body.is_empty() {
                for transaction_rlp in block_body.iter() {
                    if !transaction_rlp.is_empty() {
                        if let Ok(transaction) = transaction_rlp.as_val() {
                            transactions.push(transaction);
                        }
                    }
                }
            }
            bodies.push(transactions);
        }
    }

    // match bodies with headers
    let mut blocks = Vec::new();
    if headers.len() == bodies.len() {
        trace!(target: "sync", "Node : {}, downloading {} blocks.",  &node_hash, bodies.len());
        // Get the lock before iteration to keep the operation atomic, so that the downloaded blocks in the batch will be consecutive
        let mut downloaded_blocks_hashes = storage.downloaded_blocks_hashes().lock();
        for i in 0..headers.len() {
            let block = Block {
                header: headers[i].clone(),
                transactions: bodies[i].clone(),
            };
            let hash = block.header.hash();
            if !downloaded_blocks_hashes.contains_key(&hash) {
                blocks.push(block);
                downloaded_blocks_hashes.insert(hash, 0);
                debug!(target: "sync", "downloaded block hash: {}.", hash);
            }
        }
    } else {
        debug!(
            target: "sync",
            "Count mismatch, headers count: {}, bodies count: {}, node id hash: {}",
            headers.len(),
            bodies.len(),
            node_hash
        );
    }

    // end if no block to download
    if blocks.is_empty() {
        return;
    }

    // Save blocks
    let mut blocks_wrapper = BlocksWrapper::new();
    blocks_wrapper.node_hash = node_hash;
    blocks_wrapper.blocks.extend(blocks);
    storage.insert_downloaded_blocks(blocks_wrapper);

    // TODO: maybe we should consider reset the header request cooldown here

    p2p.update_node(&node_hash);
}
