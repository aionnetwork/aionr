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
use acore::client::{BlockChainClient, BlockId, BlockImportError, BlockStatus};
use acore::error::{BlockError, ImportError};
use aion_types::H256;
use bytes::BufMut;
use rlp::{RlpStream, UntrustedRlp};

use super::super::action::SyncAction;
use super::super::event::SyncEvent;
use super::super::storage::SyncStorage;
use super::super::SyncMgr;
use p2p::*;

const HASH_LEN: usize = 32;
const REQUEST_SIZE: u64 = 96;

pub struct BlockBodiesHandler;

impl BlockBodiesHandler {
    pub fn get_blocks_bodies(node: &mut Node, from: u64) {
        if node.synced_block_num > 0
            || node.best_block_num <= SyncStorage::get_synced_block_number()
        {
            return;
        }

        if SyncStorage::get_synced_block_number() + 2 > SyncStorage::get_network_best_block_number()
        {
            return;
        }

        let header_chain = SyncStorage::get_block_header_chain();
        let mut best_header_number = header_chain.best_block().number;
        let mut best_block_number = SyncStorage::get_chain_info().best_block_number;
        let mut headers = Vec::new();
        let mut number;

        if from == 0 {
            number = best_block_number + 1;
        } else {
            number = from;
        }

        debug!(target: "sync", "send_blocks_bodies_req, {} - {} - {} - {} - {}.", node.synced_block_num, SyncStorage::get_synced_block_number(), SyncStorage::get_network_best_block_number(), number, best_header_number);
        if number == best_header_number {
            return;
        }

        while number <= best_header_number {
            if let Some(hash) = header_chain.block_hash(BlockId::Number(number)) {
                headers.push(hash);

                if headers.len() == REQUEST_SIZE as usize {
                    debug!(target: "sync", "send_blocks_bodies_req, #{} ~ #{}, to {}.", number - REQUEST_SIZE, number, node.get_ip_addr());
                    Self::send_blocks_bodies_req(node, headers);
                    return;
                } else {
                    best_header_number = header_chain.best_block().number;
                    best_block_number = SyncStorage::get_chain_info().best_block_number;

                    if from == 0 {
                        if number < best_block_number {
                            number = best_block_number + 1;
                        } else {
                            number += 1;
                        }
                    } else {
                        number += 1;
                    }
                }
            } else {
                break;
            }
        }

        if headers.len() > 0 {
            debug!(target: "sync", "send_blocks_bodies_req, from #{} to #{}.", number - headers.len() as u64, number);
            Self::send_blocks_bodies_req(node, headers);
        } else {
            if let Some(hash) = header_chain.block_hash(BlockId::Number(best_header_number)) {
                Self::send_blocks_bodies_req(node, vec![hash]);
            }
            debug!(target: "sync", "send_blocks_bodies_req, {} - {}.", best_header_number, best_block_number);
        }
    }

    pub fn send_blocks_bodies_req(node: &mut Node, hashes: Vec<H256>) {
        let mut get_headers_with_bodies_requested =
            SyncStorage::get_headers_with_bodies_requested().lock();
        {
            if !get_headers_with_bodies_requested.contains_key(&node.node_hash) {
                let mut req = ChannelBuffer::new();
                req.head.ver = Version::V0.value();
                req.head.ctrl = Control::SYNC.value();
                req.head.action = SyncAction::BLOCKSBODIESREQ.value();

                for hash in hashes.iter() {
                    req.body.extend_from_slice(&hash);
                }

                req.head.len = req.body.len() as u32;
                P2pMgr::send(node.node_hash, req.clone());
                get_headers_with_bodies_requested.insert(node.node_hash, hashes.clone());

                SyncEvent::update_node_state(node, SyncEvent::OnBlockBodiesReq);
                P2pMgr::update_node(node.node_hash, node);
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
        let mut number = 1;

        match SyncStorage::pick_headers_with_bodies_requested(&node_hash) {
            Some(hashes) => {
                let block_bodies = UntrustedRlp::new(req.body.as_slice());
                let header_chain = SyncStorage::get_block_header_chain();
                if let Ok(item_count) = block_bodies.item_count() {
                    let count = if hashes.len() < item_count {
                        hashes.len()
                    } else {
                        if item_count > 0 {
                            item_count
                        } else {
                            debug!(target: "sync", "Body res is empty");
                            return;
                        }
                    };
                    let block_chain = SyncStorage::get_block_chain();
                    let batch_status = block_chain.block_status(BlockId::Hash(hashes[count - 1]));
                    if batch_status == BlockStatus::Unknown {
                        for i in 0..count {
                            let hash = hashes[i];
                            let status = block_chain.block_status(BlockId::Hash(hash));
                            if status == BlockStatus::Unknown {
                                if let Some(header) = header_chain.block_header(BlockId::Hash(hash))
                                {
                                    if let Ok(body) = block_bodies.at(i) {
                                        if let Ok(txs) = body.at(0) {
                                            let parent_hash = header.parent_hash();
                                            let difficulty = header.difficulty();
                                            number = header.number();
                                            let mut data = header.into_inner();
                                            data.extend_from_slice(txs.as_raw());
                                            let mut block = RlpStream::new_list(2);
                                            block.append_raw(&data, 2);

                                            SyncStorage::insert_requested_time(hash);
                                            let result =
                                                block_chain.import_block(block.as_raw().to_vec());
                                            match result {
                                                Ok(_) => {
                                                    node.inc_reputation(2);
                                                    debug!(target: "sync", "Imported block #{} - {} - {}", number, hash, node.get_ip_addr());
                                                    if node.target_total_difficulty
                                                        >= SyncStorage::get_network_total_diff()
                                                    {
                                                        node.synced_block_num = 0;
                                                    }
                                                }
                                                Err(BlockImportError::Import(
                                                    ImportError::AlreadyInChain,
                                                ))
                                                | Err(BlockImportError::Import(
                                                    ImportError::AlreadyQueued,
                                                )) => {
                                                    node.inc_reputation(1);
                                                    debug!(target: "sync", "Skipped block #{} - {} - {}", number, hash, node.get_ip_addr());
                                                }
                                                Err(BlockImportError::Block(
                                                    BlockError::UnknownParent(_),
                                                )) => {
                                                    if number == 1 {
                                                        error!(target: "sync", "Invalid genesis !!!");
                                                        return;
                                                    } else if node.target_total_difficulty
                                                        + difficulty * 2
                                                        < SyncStorage::get_network_total_diff()
                                                    {
                                                        error!(target: "sync", "Invalid peer {}@{} !!!", node.get_ip_addr(), node.get_node_id());
                                                        P2pMgr::remove_peer(node.node_hash);
                                                        return;
                                                    } else {
                                                        if let Some(_parent_header) = header_chain
                                                            .block_header(BlockId::Hash(
                                                                parent_hash,
                                                            )) {
                                                            if number > 1 {
                                                                debug!(target: "sync", "Try to get parent block : #{} - {} - {}", number - 1, parent_hash, node.synced_block_num);
                                                                Self::send_blocks_bodies_req(
                                                                    node,
                                                                    vec![parent_hash],
                                                                );
                                                            }
                                                        } else {
                                                            SyncMgr::build_header_chain(number);
                                                            info!(target: "sync", "Attempting build header chain based on #{} - {}.", number, hash);
                                                        }
                                                        SyncEvent::update_node_state(
                                                            node,
                                                            SyncEvent::OnBlockBodiesRes,
                                                        );
                                                        P2pMgr::update_node(node_hash, node);
                                                        return;
                                                    }
                                                }
                                                Err(e) => {
                                                    warn!(target: "sync", "Bad block #{} - {:?} - {}, {:?}", number, hash, node.get_ip_addr(), e);
                                                    node.inc_repeated();
                                                    block_chain.clear_bad();
                                                    P2pMgr::remove_peer(node_hash);

                                                    let from = if number > REQUEST_SIZE * 2 {
                                                        number - REQUEST_SIZE * 2
                                                    } else {
                                                        1
                                                    };
                                                    if let Some(ref mut peer_node) =
                                                        P2pMgr::get_an_active_node()
                                                    {
                                                        warn!(target: "sync", "Try to get block : #{}", from);
                                                        BlockBodiesHandler::get_blocks_bodies(
                                                            peer_node, from,
                                                        );
                                                    }

                                                    return;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        debug!(target: "sync", "BLOCKSBODIESRES from: {}, batch_status: {:?}, {:?}.", node.get_ip_addr(), batch_status, block_chain.block_number(BlockId::Hash(hashes[count - 1])));
                        SyncEvent::update_node_state(node, SyncEvent::OnBlockBodiesRes);
                        P2pMgr::update_node(node_hash, node);
                        return;
                    }
                }
            }
            None => {}
        }

        SyncEvent::update_node_state(node, SyncEvent::OnBlockBodiesRes);
        P2pMgr::update_node(node_hash, node);

        if SyncStorage::get_synced_block_number() + 128
            < SyncStorage::get_network_best_block_number()
        {
            if let Some(ref mut peer_node) = P2pMgr::get_an_active_node() {
                BlockBodiesHandler::get_blocks_bodies(peer_node, number + 1);
            }
        }
    }
}
