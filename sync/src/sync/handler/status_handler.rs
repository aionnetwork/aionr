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

use aion_types::{H256, U256};
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::BufMut;
use std::mem;

use super::super::action::SyncAction;
use super::super::event::SyncEvent;
use super::super::storage::SyncStorage;
use p2p::*;

const BEST_HASH_LENGTH: usize = 32;
const GENESIS_HASH_LENGTH: usize = 32;

pub struct StatusHandler;

impl StatusHandler {
    pub fn send_status_req_to_node(node_hash: u64) {
        let mut req = ChannelBuffer::new();
        req.head.ver = Version::V0.value();
        req.head.ctrl = Control::SYNC.value();
        req.head.action = SyncAction::STATUSREQ.value();
        req.head.len = 0;

        P2pMgr::send(node_hash, req);
    }

    pub fn send_status_req() {
        let active_nodes = P2pMgr::get_nodes(ALIVE);
        for node in active_nodes.iter() {
            trace!(target: "sync","Sync status req sent...");
            Self::send_status_req_to_node(node.node_hash);
        }
    }

    pub fn handle_status_req(node: &mut Node) {
        trace!(target: "sync", "STATUSREQ received.");

        let mut res = ChannelBuffer::new();
        let node_hash = node.node_hash;

        res.head.ver = Version::V0.value();
        res.head.ctrl = Control::SYNC.value();
        res.head.action = SyncAction::STATUSRES.value();

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
        res.head.set_length(res.body.len() as u32);
        SyncEvent::update_node_state(node, SyncEvent::OnStatusReq);
        P2pMgr::update_node(node_hash, node);
        P2pMgr::send(node_hash, res);
    }

    pub fn handle_status_res(node: &mut Node, req: ChannelBuffer) {
        trace!(target: "sync", "STATUSRES received.");

        let node_hash = node.node_hash;
        let (mut best_block_num, req_body_rest) = req.body.split_at(mem::size_of::<u64>());
        let best_block_num = best_block_num.read_u64::<BigEndian>().unwrap_or(0);
        let (mut total_difficulty_len, req_body_rest) =
            req_body_rest.split_at(mem::size_of::<u8>());
        let total_difficulty_len = total_difficulty_len.read_u8().unwrap_or(0) as usize;
        let (total_difficulty, req_body_rest) = req_body_rest.split_at(total_difficulty_len);
        let (best_hash, req_body_rest) = req_body_rest.split_at(BEST_HASH_LENGTH);
        let (_genesis_hash, _) = req_body_rest.split_at(GENESIS_HASH_LENGTH);

        node.best_hash = H256::from(best_hash);
        node.best_block_num = best_block_num;
        // if node.mode != Mode::BACKWARD && node.mode != Mode::FORWARD {
        //     let chain_info = SyncStorage::get_chain_info();
        //     node.synced_block_num = chain_info.best_block_number;
        //     node.current_total_difficulty = chain_info.total_difficulty;
        // }
        node.target_total_difficulty = U256::from(total_difficulty);
        SyncEvent::update_node_state(node, SyncEvent::OnStatusRes);
        node.inc_reputation(1);
        P2pMgr::update_node(node_hash, node);

        if SyncStorage::get_synced_block_number() == 0 {
            // let init_synced_block_number = if node.best_block_num > STAGED_BLOCK_COUNT { node.best_block_num - STAGED_BLOCK_COUNT } else { STAGED_BLOCK_COUNT };
            
            let chain_info = SyncStorage::get_chain_info();
            let init_synced_block_number = chain_info.best_block_number;
            
            SyncStorage::set_synced_block_number(init_synced_block_number);
            SyncStorage::set_starting_block_number(init_synced_block_number + 1);
        }

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

        // BlockHeadersHandler::get_headers_from_node(node, 1);
    }
}
