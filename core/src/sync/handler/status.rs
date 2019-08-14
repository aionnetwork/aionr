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
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::BufMut;
use sync::route::VERSION;
use sync::route::MODULE;
use sync::route::ACTION;
use sync::storage::SyncStorage;
use p2p::send as p2p_send;
use p2p::ChannelBuffer;
use p2p::Node;

const HASH_LENGTH: usize = 32;

pub fn receive_req(hash: u64, cb_in: Option<ChannelBuffer>, nodes: Arc<RwLock<HashMap<u64, Node>>>) {
    debug!(target: "sync", "status/receive_req");

    let mut cb = ChannelBuffer::new();

    cb.head.ver = VERSION::V0.value();
    cb.head.ctrl = MODULE::SYNC.value();
    cb.head.action = ACTION::STATUSRES.value();

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

    cb.body.put_slice(res_body.as_slice());
    cb.head.len = cb.body.len() as u32;
    
    p2p_send(hash, cb, nodes);
}

pub fn receive_res(hash: u64, cb_in: Option<ChannelBuffer>, nodes: Arc<RwLock<HashMap<u64, Node>>>) {
    trace!(target: "sync", "status/receive_res");

    match nodes.try_write() {
        Ok(mut write) => {
            match write.get_mut(&hash) {
                Some(mut node) => {
                   
                    let req = cb_in.unwrap();
                    let (mut best_block_num, req_body_rest) = req.body.split_at(mem::size_of::<u64>());
                    let best_block_num = best_block_num.read_u64::<BigEndian>().unwrap_or(0);
                    let (mut total_difficulty_len, req_body_rest) = req_body_rest.split_at(mem::size_of::<u8>());
                    let total_difficulty_len = total_difficulty_len.read_u8().unwrap_or(0) as usize;
                    let (total_difficulty, req_body_rest) = req_body_rest.split_at(total_difficulty_len);
                    let (best_hash, req_body_rest) = req_body_rest.split_at(HASH_LENGTH);
                    let (_genesis_hash, _) = req_body_rest.split_at(HASH_LENGTH);

                    node.block_hash = H256::from(best_hash);
                    node.block_num = best_block_num;
                    node.total_difficulty = U256::from(total_difficulty);
                },
                None => {
                    // TODO: 
                }
            }
        },
        Err(err) => {
            //TODO:
        }
    }

    // if node.mode != Mode::BACKWARD && node.mode != Mode::FORWARD {
    //     let chain_info = SyncStorage::get_chain_info();
    //     node.synced_block_num = chain_info.best_block_number;
    //     node.current_total_difficulty = chain_info.total_difficulty;
    // }
    

    // internal comparing node td vs current network status td
    //SyncStorage::update_network_status(
    //    node.block_num,
    //    node.block_hash,
    //    node.total_difficulty,
    //);

    // immediately send request when incoming response indicates node td > local chain td
    // even headers::get_headers_from_node has condition check internally
    // TODO: leave condition check in one place
    //if node.total_difficulty >= SyncStorage::get_chain_info().total_difficulty {
        // headers::get_headers_from_node(node);
    //}
}