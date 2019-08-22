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
use std::sync::RwLock;
use std::collections::HashMap;
use aion_types::{H256, U256};
use types::blockchain::info::BlockChainInfo;
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::BufMut;
use sync::wrappers::HeaderWrapper;
use sync::node_info::NodeInfo;
use sync::route::{VERSION,MODULE,ACTION};
use sync::handler::headers;
use sync::handler::headers::NORMAL_REQUEST_SIZE;
use p2p::{ChannelBuffer,  Mgr};

const HASH_LENGTH: usize = 32;

pub fn send_random(p2p: Mgr) {
    if let Some(hash) = p2p.get_random_active_node_hash() {
        send(p2p, hash)
    }
}

pub fn send(p2p: Mgr, hash: u64) {
    let mut cb = ChannelBuffer::new();
    cb.head.ver = VERSION::V0.value();
    cb.head.ctrl = MODULE::SYNC.value();
    cb.head.action = ACTION::STATUSREQ.value();
    cb.head.len = 0;
    let mut p2p_0 = p2p.send(hash, cb);
}

pub fn receive_req(p2p: Mgr, chain_info: &BlockChainInfo, hash: u64) {
    debug!(target: "sync", "status/receive_req");

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
    chain_info: &BlockChainInfo,
    node_info: Arc<RwLock<HashMap<u64, NodeInfo>>>,
    hws: Arc<RwLock<HashMap<u64, HeaderWrapper>>>,
    hash: u64,
    cb_in: ChannelBuffer,
)
{
    trace!(target: "sync", "status/receive_res");
    match node_info.try_write() {
        Ok(mut write) => {
            trace!(target: "sync", "cb_body_len{}",cb_in.head.len);
            let (mut best_block_num, req_body_rest) = cb_in.body.split_at(mem::size_of::<u64>());
            let best_block_num = best_block_num.read_u64::<BigEndian>().unwrap_or(0);
            let (mut total_difficulty_len, req_body_rest) =
                req_body_rest.split_at(mem::size_of::<u8>());
            let total_difficulty_len = total_difficulty_len.read_u8().unwrap_or(0) as usize;
            let (total_difficulty, req_body_rest) = req_body_rest.split_at(total_difficulty_len);
            let (best_hash, req_body_rest) = req_body_rest.split_at(HASH_LENGTH);
            let (_genesis_hash, _) = req_body_rest.split_at(HASH_LENGTH);
            let td = U256::from(total_difficulty);
            let bh = H256::from(best_hash);
            if let Some(mut node_info) = write.get_mut(&hash) {
                node_info.block_hash = bh;
                node_info.block_number = best_block_num;
                node_info.total_difficulty = td;
                p2p.update_node(&hash);

                if chain_info.total_difficulty < node_info.total_difficulty {
                    if let Ok(wrappers) = hws.read() {
                        if wrappers.keys().find(|x| **x == hash).is_none() {
                            headers::prepare_send(p2p.clone(), hash, chain_info.best_block_number);
                        }
                    } else {
                        println!("ininin");
                    }
                }
                return;
            }
            {
                // TODO:

                trace!(target: "sync", "new node info: hash:{}, bn:{}, bh:{}, td:{}", hash, best_block_num, bh, td);

                write.insert(
                    hash,
                    NodeInfo {
                        block_hash: bh,
                        block_number: best_block_num,
                        total_difficulty: td,
                    },
                );
                p2p.update_node(&hash);

                if chain_info.total_difficulty < td {
                    if let Ok(wrappers) = hws.read() {
                        if wrappers.keys().find(|x| **x == hash).is_none() {
                            headers::prepare_send(p2p.clone(), hash, chain_info.best_block_number);
                        }
                    } else {
                        println!("ininin");
                    }
                }
                warn!(target: "sync", "status/res cannot get node info with hash:{}" ,hash);
            }
        }
        Err(err) => {
            //TODO:

            warn!(target: "sync", "status/res cannot get node info map");
        }
    }
}
