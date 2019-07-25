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

use std::mem;
use std::time::SystemTime;
use aion_types::{H256, U256};
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::BufMut;
use sync::route::VERSION;
use sync::route::MODULE;
use sync::route::ACTION;
use sync::event::SyncEvent;
use sync::storage::SyncStorage;
use sync::handler::headers;
use p2p::P2pMgr;
use p2p::ChannelBuffer;
use p2p::Node;
use p2p::Mode;

const HASH_LENGTH: usize = 32;

pub fn send(node_hash: u64) {
    let mut req = ChannelBuffer::new();
    req.head.ver = VERSION::V0.value();
    req.head.ctrl = MODULE::SYNC.value();
    req.head.action = ACTION::STATUSREQ.value();
    req.head.len = 0;
    P2pMgr::send(node_hash, req);
}

pub fn receive_req(node: &mut Node) {
    trace!(target: "sync", "STATUSREQ received.");

    let mut res = ChannelBuffer::new();
    let node_hash = node.node_hash;

    res.head.ver = VERSION::V0.value();
    res.head.ctrl = MODULE::SYNC.value();
    res.head.action = ACTION::STATUSRES.value();

    let mut res_body = Vec::new();
    let chain_info = SyncStorage::get_chain_info();

    let mut best_block_number = [0; 8];
    BigEndian::write_u64(&mut best_block_number, chain_info.best_block_number);

    let total_difficulty = chain_info.total_difficulty;
    let best_hash = chain_info.best_block_hash;
    let genesis_hash = chain_info.genesis_hash;

    res_body.put_slice(&best_block_number);

    let mut total_difficulty_buf = [0u8; 32];
    total_difficulty.to_big_endian(&mut total_difficulty_buf);

    res_body.push(32 as u8);
    res_body.put_slice(&total_difficulty_buf.to_vec());
    res_body.put_slice(&best_hash);
    res_body.put_slice(&genesis_hash);

    res.body.put_slice(res_body.as_slice());
    res.head.len = res.body.len() as u32;
    SyncEvent::update_node_state(node, SyncEvent::OnStatusReq);
    P2pMgr::update_node(node_hash, node);
    P2pMgr::send(node_hash, res);
}

pub fn receive_res(node: &mut Node, req: ChannelBuffer) {
    trace!(target: "sync", "STATUSRES received.");

    let node_hash = node.node_hash;
    let (mut best_block_num, req_body_rest) = req.body.split_at(mem::size_of::<u64>());
    let best_block_num = best_block_num.read_u64::<BigEndian>().unwrap_or(0);
    let (mut total_difficulty_len, req_body_rest) = req_body_rest.split_at(mem::size_of::<u8>());
    let total_difficulty_len = total_difficulty_len.read_u8().unwrap_or(0) as usize;
    let (total_difficulty, req_body_rest) = req_body_rest.split_at(total_difficulty_len);
    let (best_hash, req_body_rest) = req_body_rest.split_at(HASH_LENGTH);
    let (_genesis_hash, _) = req_body_rest.split_at(HASH_LENGTH);

    node.best_hash = H256::from(best_hash);
    node.best_block_num = best_block_num;
    if node.mode != Mode::BACKWARD && node.mode != Mode::FORWARD {
        let chain_info = SyncStorage::get_chain_info();
        node.synced_block_num = chain_info.best_block_number;
        node.current_total_difficulty = chain_info.total_difficulty;
    }
    node.target_total_difficulty = U256::from(total_difficulty);
    SyncEvent::update_node_state(node, SyncEvent::OnStatusRes);
    P2pMgr::update_node(node_hash, node);

    SyncStorage::update_network_status(
        node.best_block_num,
        node.best_hash,
        node.target_total_difficulty,
    );

    let sync_from_boot_nodes_only = P2pMgr::get_network_config().sync_from_boot_nodes_only;
    if sync_from_boot_nodes_only {
        if !node.is_from_boot_list {
            return;
        }
    }

    if SyncStorage::get_network_best_block_number() <= SyncStorage::get_synced_block_number() {
        node.last_request_timestamp = SystemTime::now();
        P2pMgr::update_node(node.node_hash, node);
    } else {
        if let Some(mut node) = P2pMgr::get_an_active_node() {
            if node.synced_block_num == 0 {
                node.synced_block_num = SyncStorage::get_synced_block_number() + 1;
            }
            headers::get_headers_from_node(&mut node);
        }
    }

    // if SyncStorage::get_network_best_block_number() <= SyncStorage::get_synced_block_number() {
    //     node.last_request_timestamp = SystemTime::now();
    //     P2pMgr::update_node(node.node_hash, node);
    // } else {
    //     BlockHeadersHandler::get_headers_from_random_node();
    // }
    // ------
    // FIX: syncing should be determined by difficuly but not block number
    // ------
    // if node.target_total_difficulty <= node.current_total_difficulty {
    //     node.last_request_timestamp = SystemTime::now();
    //     P2pMgr::update_node(node.node_hash, node);
    // } else {
    //     BlockHeadersHandler::get_headers_from_node(node);
    // }
}
