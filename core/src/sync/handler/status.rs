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
use std::sync::Arc;
use std::collections::HashMap;

use parking_lot::RwLock;

use aion_types::{H256, U256};
use types::blockchain::info::BlockChainInfo;
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::BufMut;
use sync::node_info::NodeInfo;
use sync::action::Action;
use p2p::{ChannelBuffer, Mgr};

use super::{channel_buffer_template_with_version,channel_buffer_template};

const HASH_LENGTH: usize = 32;

pub fn send_random(p2p: Mgr, node_info: Arc<RwLock<HashMap<u64, RwLock<NodeInfo>>>>) {
    if let Some(hash) = p2p.get_random_active_node_hash() {
        let mut node_info = node_info.write();
        if !node_info.contains_key(&hash) {
            trace!(target: "sync", "new node info: hash:{}", hash);
            node_info.insert(hash, RwLock::new(NodeInfo::new()));
        }
        drop(node_info);
        send(p2p, hash)
    }
}

pub fn send(p2p: Mgr, hash: u64) {
    let cb = channel_buffer_template(Action::STATUSREQ.value());
    p2p.send(hash, cb);
}

pub fn receive_req(p2p: Mgr, chain_info: &BlockChainInfo, hash: u64, version: u16) {
    trace!(target: "sync", "status/receive_req");

    let mut cb = channel_buffer_template_with_version(version, Action::STATUSRES.value());

    let mut res_body = Vec::new();

    let mut best_block_number = [2u8; 8];
    BigEndian::write_u64(&mut best_block_number, chain_info.best_block_number);

    let total_difficulty = chain_info.total_difficulty;
    let best_hash = chain_info.best_block_hash;
    let genesis_hash = chain_info.genesis_hash;

    res_body.put_slice(&best_block_number);

    let mut total_difficulty_buf = [1u8; 32];
    total_difficulty.to_big_endian(&mut total_difficulty_buf);

    res_body.push(32 as u8);
    res_body.put_slice(&total_difficulty_buf.to_vec());
    res_body.put_slice(&best_hash);
    res_body.put_slice(&genesis_hash);

    cb.body.put_slice(res_body.as_slice());
    cb.head.len = cb.body.len() as u32;
    trace!(target:"sync", "status res bc body len: {}", cb.head.len);

    p2p.update_node(&hash);
    p2p.send(hash, cb);
}

pub fn receive_res(
    p2p: Mgr,
    node_info: Arc<RwLock<HashMap<u64, RwLock<NodeInfo>>>>,
    hash: u64,
    cb_in: ChannelBuffer,
    network_best_td: Arc<RwLock<U256>>,
    network_best_block_number: Arc<RwLock<u64>>,
    local_genesis_hash: H256,
)
{
    trace!(target: "sync", "status/receive_res");

    // check channelbuffer len
    if (cb_in.head.len as usize) < mem::size_of::<u64>() + mem::size_of::<u8>() + 2 * HASH_LENGTH {
        debug!(target: "sync", "status res channelbuffer length is too short" );
        return;
    }

    let (mut best_block_num, req_body_rest) = cb_in.body.split_at(mem::size_of::<u64>());
    let best_block_num = best_block_num.read_u64::<BigEndian>().unwrap_or(0);
    let (mut total_difficulty_len, req_body_rest) = req_body_rest.split_at(mem::size_of::<u8>());
    let total_difficulty_len = total_difficulty_len.read_u8().unwrap_or(0) as usize;

    // check total_difficulty_len
    if req_body_rest.len() < total_difficulty_len + 2 * HASH_LENGTH {
        debug!(target: "sync", "status res with wrong total_difficulty length " );
        return;
    }

    let (total_difficulty, req_body_rest) = req_body_rest.split_at(total_difficulty_len);
    let (best_hash, rest) = req_body_rest.split_at(HASH_LENGTH);
    let (genesis_hash, _rest) = rest.split_at(HASH_LENGTH);

    let total_difficulty = U256::from(total_difficulty);
    let best_hash = H256::from(best_hash);
    let genesis_hash = H256::from(genesis_hash);
    if genesis_hash == local_genesis_hash {
        // Update network best block
        let mut network_best_td = network_best_td.write();
        let mut network_best_block_number = network_best_block_number.write();
        if total_difficulty > *network_best_td {
            *network_best_td = total_difficulty;
            *network_best_block_number = best_block_num;
        }

        // Update node info
        // TODO: improve this
        {
            let mut node_info_write = node_info.write();
            if !node_info_write.contains_key(&hash) {
                trace!(target: "sync", "new node info: hash:{}, bn:{}, bh:{}, td:{}", hash, best_block_num, best_hash, total_difficulty);
            }
            let info_lock = node_info_write
                .entry(hash)
                .or_insert(RwLock::new(NodeInfo::new()));
            let mut info = info_lock.write();
            info.best_block_hash = best_hash;
            info.best_block_number = best_block_num;
            info.total_difficulty = total_difficulty;
        }

        p2p.update_node(&hash);
    } else {
        error!(target: "sync", "Bad status res from node:{} invalid genesis, local genesis: {}, node genesis: {}",hash, local_genesis_hash, genesis_hash);
        // TODO: move node to black list
    }
}
