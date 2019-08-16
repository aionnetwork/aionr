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

use std::sync::{RwLock,Arc};
use std::collections::{HashMap,BTreeMap};
use block::Block;
use client::BlockId;
use header::{Seal,Header};
use aion_types::H256;
use bytes::BufMut;
use rlp::{RlpStream, UntrustedRlp};
use std::time::SystemTime;
use p2p::ChannelBuffer;
use p2p::Node;
use p2p::send as p2p_send;
use sync::route::VERSION;
use sync::route::MODULE;
use sync::route::ACTION;
use sync::event::SyncEvent;
use sync::helper::{Wrapper,WithStatus};
use sync::storage::BlocksWrapper;
use sync::storage::SyncStorage;
use sync::handler::headers;

const HASH_LEN: usize = 32;

pub fn send() {
    //    let mut req = ChannelBuffer::new();
    //    req.head.ver = VERSION::V0.value();
    //    req.head.ctrl = MODULE::SYNC.value();
    //    req.head.action = ACTION::BODIESREQ.value();
    //
    //    let mut hws = Vec::new();
    //    if let Ok(mut downloaded_headers) = SyncStorage::get_downloaded_headers().try_lock() {
    //        while let Some(hw) = downloaded_headers.pop_front() {
    //            if !hw.headers.is_empty() {
    //                hws.push(hw);
    //            }
    //        }
    //    }
    //
    //    for hw in hws.iter() {
    //        let mut req = req.clone();
    //        req.body.clear();
    //
    //        let mut header_requested = Vec::new();
    //        for header in hw.headers.iter() {
    //            if !SyncStorage::is_downloaded_block_hashes(&header.hash())
    //                && !SyncStorage::is_imported_block_hash(&header.hash())
    //            {
    //                req.body.put_slice(&header.hash());
    //                header_requested.push(header.clone());
    //            }
    //        }
    //
    //        let body_len = req.body.len();
    //        if body_len > 0 {
    //            if let Ok(ref mut headers_with_bodies_requested) =
    //                SyncStorage::get_headers_with_bodies_requested().lock()
    //            {
    //                if !headers_with_bodies_requested.contains_key(&hw.node_hash) {
    //                    req.head.len = body_len as u32;
    //
    //                    p2p_send(hw.node_hash, req);
    //
    //                    trace!(target: "sync", "Sync blocks bodies req sent...");
    //                    let mut hw = hw.clone();
    //                    hw.timestamp = SystemTime::now();
    //                    hw.headers.clear();
    //                    hw.headers.extend(header_requested);
    //                    headers_with_bodies_requested.insert(hw.node_hash, hw);
    //                }
    //            }
    //        }
    //    }
}

pub fn receive_req(hash: u64, cb_in: ChannelBuffer, nodes: Arc<RwLock<HashMap<u64, Node>>>) {
    trace!(target: "sync", "bodies/receive_req");

    let mut res = ChannelBuffer::new();

    res.head.ver = VERSION::V0.value();
    res.head.ctrl = MODULE::SYNC.value();
    res.head.action = ACTION::BODIESRES.value();

    let mut res_body = Vec::new();
    let hash_count = cb_in.body.len() / HASH_LEN;
    let mut rest = cb_in.body.as_slice();
    let mut data = Vec::new();
    let mut body_count = 0;
    let client = SyncStorage::get_block_chain();
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

    //    SyncEvent::update_node_state(node, SyncEvent::OnBlockBodiesReq);
    //    update_node(node_hash, node);
    p2p_send(&hash, res, nodes);
}

pub fn receive_res(
    hash: u64,
    cb_in: ChannelBuffer,
    nodes: Arc<RwLock<HashMap<u64, Node>>>,
    hws: Arc<RwLock<BTreeMap<u64, Wrapper>>>,
)
{
    trace!(target: "sync", "bodies/receive_res");

    if cb_in.body.len() > 0 {
        if let Ok(mut wrappers) = hws.write() {
            if let Some((hash, wrapper)) = wrappers
                .clone()
                .iter()
                .filter(|(_, w)| {
                    if let WithStatus::WaitForBody(_) = w.with_status {
                        true
                    } else {
                        false
                    }
                })
                .next()
            {
                let mut new_wrapper = wrapper.clone();
                match wrapper.with_status {
                    WithStatus::WaitForBody(ref hw) => {
                        let headers = hw;
                        if !headers.is_empty() {
                            let rlp = UntrustedRlp::new(cb_in.body.as_slice());

                            let mut bodies = Vec::new();
                            let mut blocks = Vec::new();
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

                            if headers.len() == bodies.len() {
                                for i in 0..headers.len() {
                                    let rlp = UntrustedRlp::new(&headers[i]);
                                    let header: Header = rlp.as_val().expect("should be a head");
                                    let block = Block {
                                        header,
                                        transactions: bodies[i].clone(),
                                    };
                                    blocks.push(block.rlp_bytes(Seal::Without));
                                    //                                        if let Ok(mut downloaded_block_hashes) =
                                    //                                        SyncStorage::get_downloaded_block_hashes().lock()
                                    {
                                        //                                                let hash = block.header.hash();
                                        //                                                if !downloaded_block_hashes.contains_key(&hash) {
                                        //                                                    downloaded_block_hashes.insert(hash, 0);
                                        //                                                } else {
                                        //                                                    trace!(target: "sync", "downloaded_block_hashes: {}.", hash);
                                        //                                                }
                                    }
                                }
                            } else {
                                debug!(
                                        target: "sync",
                                        "Count mismatch, headers count: {}, bodies count: {}",
                                        headers.len(),
                                        bodies.len(),
                                    );
                                blocks.clear();
                            }

                            if !blocks.is_empty() {
                                if let Some(w) = wrappers.get_mut(hash) {
                                    (*w).timestamp = SystemTime::now();
                                    (*w).with_status = WithStatus::GetBody(blocks);
                                }
                                //                                    if node.mode == Mode::LIGHTNING {
                                //                                        if let Some(block) = blocks.get(0) {
                                //                                            let block_number = block.header.number();
                                //                                            let max_staged_block_number =
                                //                                                SyncStorage::get_max_staged_block_number();
                                //                                            if block_number < = max_staged_block_number {
                                //                                                debug!(target: "sync", "Block #{} is out of staging scope: [#{} - Lastest)", block_number, max_staged_block_number);
                                //                                                return;
                                //                                            } else {
                                //                                                let mut block_hashes_to_stage = Vec::new();
                                //                                                let mut blocks_to_stage = Vec::new();
                                //
                                //                                                let parent_hash = block.header.parent_hash();
                                //                                                let parent_number = block_number - 1;
                                //                                                if let Ok(mut staged_blocks) =
                                //                                                SyncStorage::get_staged_blocks().lock()
                                //                                                    {
                                //                                                        if staged_blocks.len() < 32
                                //                                                            & &!staged_blocks.contains_key(&parent_hash)
                                //                                                            {
                                //                                                                for blk in blocks.iter() {
                                //                                                                    let hash = blk.header.hash();
                                //                                                                    block_hashes_to_stage.push(hash);
                                //                                                                    blocks_to_stage.push(blk.rlp_bytes(Seal::With));
                                //                                                                }
                                //
                                //                                                                let max_staged_block_number =
                                //                                                                    parent_number + blocks_to_stage.len() as u64;
                                //
                                //                                                                info!(target: "sync", "Staged blocks from {} to {} with parent: {}", parent_number + 1, max_staged_block_number, parent_hash);
                                //                                                                debug!(target: "sync", "cache size: {}", staged_blocks.len());
                                //
                                //                                                                SyncStorage::insert_staged_block_hashes(
                                //                                                                    block_hashes_to_stage,
                                //                                                                );
                                //
                                //                                                                staged_blocks.insert(*parent_hash, blocks_to_stage);
                                //
                                //                                                                if max_staged_block_number
                                //                                                                    > SyncStorage::get_max_staged_block_number()
                                //                                                                    {
                                //                                                                        SyncStorage::set_max_staged_block_number(
                                //                                                                            max_staged_block_number,
                                //                                                                        );
                                //                                                                    }
                                //                                                            }
                                //                                                    }
                                //                                            }
                                //                                        }
                                //                                    } else {
                                //                                        let mut bw = BlocksWrapper::new();
                                //                                        bw.node_id_hash = node.node_hash;
                                //                                        bw.blocks.extend(blocks);
                                //                                        SyncStorage::insert_downloaded_blocks(bw);
                                //                                    }

                                //                                    if node.mode == Mode::NORMAL | | node.mode == Mode::THUNDER {
                                //                                        headers::get_headers_from_node(node);
                                //                                    }
                            }
                        }
                    }
                    _ => {
                        //do nothing
                    }
                }
            }
        }
    }
}

//    SyncEvent::update_node_state(node, SyncEvent::OnBlockBodiesRes);
//    update_node(node_hash, node);
