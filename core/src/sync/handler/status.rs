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
use sync::route::{VERSION,MODULE,ACTION};
use p2p::{ChannelBuffer, Mgr};

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
    let mut cb = ChannelBuffer::new();
    cb.head.ver = VERSION::V0.value();
    cb.head.ctrl = MODULE::SYNC.value();
    cb.head.action = ACTION::STATUSREQ.value();
    cb.head.len = 0;
    p2p.send(hash, cb);
}

pub fn receive_req(p2p: Mgr, chain_info: &BlockChainInfo, hash: u64) {
    trace!(target: "sync", "status/receive_req");

    let mut cb = ChannelBuffer::new();

    cb.head.ver = VERSION::V0.value();
    cb.head.ctrl = MODULE::SYNC.value();
    cb.head.action = ACTION::STATUSRES.value();

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
    network_best_block_number: Arc<RwLock<u64>>,
    local_genesis_hash: H256,
)
{
    trace!(target: "sync", "status/receive_res");
    let (mut best_block_num, req_body_rest) = cb_in.body.split_at(mem::size_of::<u64>());
    let best_block_num = best_block_num.read_u64::<BigEndian>().unwrap_or(0);
    let (mut total_difficulty_len, req_body_rest) = req_body_rest.split_at(mem::size_of::<u8>());
    let total_difficulty_len = total_difficulty_len.read_u8().unwrap_or(0) as usize;
    let (total_difficulty, req_body_rest) = req_body_rest.split_at(total_difficulty_len);
    let (best_hash, req_body_rest) = req_body_rest.split_at(HASH_LENGTH);
    let (genesis_hash, _) = req_body_rest.split_at(HASH_LENGTH);
    let td = U256::from(total_difficulty);
    let bh = H256::from(best_hash);
    let genesis_hash = H256::from(genesis_hash);
    if genesis_hash == local_genesis_hash {
        // Update network best block
        let mut network_best_number = network_best_block_number.write();
        if best_block_num > *network_best_number {
            *network_best_number = best_block_num;
        }

        // Update node info
        // TODO: improve this
        let mut node_info_write = node_info.write();
        if !node_info_write.contains_key(&hash) {
            trace!(target: "sync", "new node info: hash:{}, bn:{}, bh:{}, td:{}", hash, best_block_num, bh, td);
        }
        let info_lock = node_info_write
            .entry(hash)
            .or_insert(RwLock::new(NodeInfo::new()));
        let mut info = info_lock.write();
        info.best_block_hash = bh;
        info.best_block_number = best_block_num;
        info.total_difficulty = td;
        drop(info);

        p2p.update_node(&hash);
    } else {
        error!(target: "sync", "Bad status res from node:{} invalid genesis, local genesis: {}, node genesis: {}",hash, local_genesis_hash, genesis_hash);
        // TODO: move node to black list
    }
}
