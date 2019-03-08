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
use acore::client::{BlockId, BlockStatus};
use aion_types::H256;
use bytes::BufMut;
use rlp::{RlpStream, UntrustedRlp};
use std::sync::Mutex;

use super::super::action::SyncAction;
use super::super::event::SyncEvent;
use super::super::storage::SyncStorage;
use p2p::*;

lazy_static! {
    static ref LOCK: Mutex<u8> = Mutex::new(0);
}

const HASH_LEN: usize = 32;
const REQUEST_SIZE: u64 = 96;

pub struct BlockBodiesHandler;

impl BlockBodiesHandler {
    pub fn send_blocks_bodies_req() {
        let mut req = ChannelBuffer::new();
        req.head.ver = Version::V0.value();
        req.head.ctrl = Control::SYNC.value();
        req.head.action = SyncAction::BLOCKSBODIESREQ.value();

        let header_chain = SyncStorage::get_block_header_chain();
        let block_chain = SyncStorage::get_block_chain();
        let mut best_header_number = header_chain.chain_info().best_block_number;
        let mut best_block_number = block_chain.chain_info().best_block_number;

        let mut headers = Vec::new();
        let mut number = best_block_number + 1;
        let mut hash = H256::from(0);
        while number <= best_header_number {
            if let Some(header) = header_chain.block_header(BlockId::Number(number)) {
                hash = header.hash();
                headers.push(hash);
                req.body.extend_from_slice(&hash);

                if headers.len() == REQUEST_SIZE as usize {
                    if let Some(ref mut node) = P2pMgr::get_an_active_node() {
                        let mut get_headers_with_bodies_requested =
                            SyncStorage::get_headers_with_bodies_requested().lock();
                        {
                            if !get_headers_with_bodies_requested.contains_key(&node.node_hash) {
                                req.head.len = req.body.len() as u32;
                                P2pMgr::send(node.node_hash, req.clone());
                                get_headers_with_bodies_requested
                                    .insert(node.node_hash, headers.clone());
                                debug!(target: "sync", "send_blocks_bodies_req for #{} to #{} - {}, msg: {}.", number - REQUEST_SIZE, number, hash, req);

                                SyncEvent::update_node_state(node, SyncEvent::OnBlockBodiesReq);
                                P2pMgr::update_node(node.node_hash, node);
                            }
                        }
                    }
                    return;
                } else {
                    best_block_number = block_chain.chain_info().best_block_number + 1;
                    best_header_number = header_chain.chain_info().best_block_number;
                    if best_block_number > number {
                        number = best_block_number;
                    } else {
                        number += 1;
                    }
                }
            }
        }

        if headers.len() > 0 {
            if let Some(ref mut node) = P2pMgr::get_an_active_node() {
                let mut get_headers_with_bodies_requested =
                    SyncStorage::get_headers_with_bodies_requested().lock();
                {
                    if !get_headers_with_bodies_requested.contains_key(&node.node_hash) {
                        req.head.len = req.body.len() as u32;
                        P2pMgr::send(node.node_hash, req.clone());
                        get_headers_with_bodies_requested.insert(node.node_hash, headers.clone());
                        debug!(target: "sync", "send_blocks_bodies_req for #{} to #{} - {}, msg: {}.", number as usize - headers.len(), number, hash, req);

                        SyncEvent::update_node_state(node, SyncEvent::OnBlockBodiesReq);
                        P2pMgr::update_node(node.node_hash, node);
                    }
                }
            }
        }
    }

    pub fn handle_blocks_bodies_req(node: &mut Node, req: ChannelBuffer) {
        trace!(target: "sync", "BLOCKSBODIESREQ received.");

        let mut res = ChannelBuffer::new();
        let node_hash = node.node_hash;

        res.head.ver = Version::V0.value();
        res.head.ctrl = Control::SYNC.value();
        res.head.action = SyncAction::BLOCKSBODIESRES.value();

        let mut res_body = Vec::new();
        let hash_count = req.body.len() / HASH_LEN;
        let mut rest = req.body.as_slice();
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
        res.head.set_length(res.body.len() as u32);

        SyncEvent::update_node_state(node, SyncEvent::OnBlockBodiesReq);
        P2pMgr::update_node(node_hash, node);
        P2pMgr::send(node_hash, res);
    }

    pub fn handle_blocks_bodies_res(node: &mut Node, req: ChannelBuffer) {
        trace!(target: "sync", "BLOCKSBODIESRES received from: {}.", node.get_ip_addr());

        let node_hash = node.node_hash;
        if req.body.len() > 0 {
            match SyncStorage::pick_headers_with_bodies_requested(&node_hash) {
                Some(hashes) => {
                    let block_bodies = UntrustedRlp::new(req.body.as_slice());
                    let header_chain = SyncStorage::get_block_header_chain();
                    let block_chain = SyncStorage::get_block_chain();
                    if let Ok(item_count) = block_bodies.item_count() {
                        if hashes.len() == item_count {
                            for i in 0..item_count {
                                let hash = hashes[i];
                                if let Some(header) = header_chain.block_header(BlockId::Hash(hash))
                                {
                                    if let Ok(body) = block_bodies.at(i) {
                                        if let Ok(txs) = body.at(0) {
                                            let mut data = header.into_inner();
                                            data.extend_from_slice(txs.as_raw());
                                            let mut block = RlpStream::new_list(2);
                                            block.append_raw(&data, 2);

                                            if LOCK.lock().is_ok() {
                                                if block_chain.block_status(BlockId::Hash(hash))
                                                    == BlockStatus::Unknown
                                                {
                                                    SyncStorage::insert_requested_time(hash);
                                                    let result = block_chain
                                                        .import_block(block.as_raw().to_vec());
                                                    trace!(target: "sync", "result: {:?}.", result);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                None => {}
            }
        }

        SyncEvent::update_node_state(node, SyncEvent::OnBlockBodiesRes);
        P2pMgr::update_node(node_hash, node);
    }
}
