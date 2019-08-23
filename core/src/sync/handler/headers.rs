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
use std::time::{SystemTime, Duration};
use std::sync::{Arc, RwLock, Mutex};
use std::collections::{HashMap, VecDeque};
use engine::unity_engine::UnityEngine;
use header::Header;
use acore_bytes::to_hex;
use aion_types::{H256, U256};
use client::{BlockChainClient, BlockId};
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::BufMut;
use rlp::{RlpStream, UntrustedRlp};
use lru_cache::LruCache;
use p2p::{ChannelBuffer, Mgr, Node};
use sync::route::{VERSION, MODULE, ACTION};
use sync::wrappers::{HeaderWrapper};
use sync::node_info::NodeInfo;
use rand::{thread_rng, Rng};

pub const NORMAL_REQUEST_SIZE: u32 = 24;
const LARGE_REQUEST_SIZE: u32 = 48;
const REQUEST_COOLDOWN: u64 = 5000;

pub fn sync_headers(
    p2p: Mgr,
    nodes_info: Arc<RwLock<HashMap<u64, NodeInfo>>>,
    local_total_diff: &U256,
    local_best_block_number: u64,
)
{
    let active_nodes = p2p.get_active_nodes();
    let candidates: Vec<Node> =
        filter_nodes_to_sync_headers(active_nodes, nodes_info, local_total_diff);
    if let Some(candidate) = pick_random_node(&candidates) {
        let candidate_hash = candidate.get_hash();
        prepare_send(p2p, candidate_hash, local_best_block_number);
    }
}

fn prepare_send(p2p: Mgr, hash: u64, best_num: u64 /*mode:Mode*/) {
    // TODO mode match
    let start = if best_num > 3 { best_num - 3 } else { 1 };
    let size = NORMAL_REQUEST_SIZE;

    send(p2p.clone(), hash, start, size);
}

fn send(p2p: Mgr, hash: u64, start: u64, size: u32) {
    debug!(target:"sync","headers.rs/send: start {}, size: {}, node hash: {}", start, size, hash);
    let mut cb = ChannelBuffer::new();
    cb.head.ver = VERSION::V0.value();
    cb.head.ctrl = MODULE::SYNC.value();
    cb.head.action = ACTION::HEADERSREQ.value();

    let mut from_buf = [0u8; 8];
    BigEndian::write_u64(&mut from_buf, start);
    cb.body.put_slice(&from_buf);

    let mut size_buf = [0u8; 4];
    BigEndian::write_u32(&mut size_buf, size);
    cb.body.put_slice(&size_buf);

    cb.head.len = cb.body.len() as u32;
    p2p.send(hash, cb);
}

// pub fn send(
//     p2p: Arc<Mgr>,
//     start: u64,
//     chain_info: &BlockChainInfo,
//     ws: Arc<RwLock<HashMap<u64, HeaderWrapper>>>,
// )
// {
//     let working_nodes = get_working_nodes(ws);

//     if let Some(node) = p2p.get_random_active_node(&working_nodes) {

//         if node.total_difficulty > chain_info.total_difficulty
//             && node.block_num - REQUEST_SIZE as u64 >= chain_info.best_block_number
//         {
//             let start = if start > 3 {
//                 start - 3
//             } else if chain_info.best_block_number > 3 {
//                 chain_info.best_block_number - 3
//             } else {
//                 1
//             };
//             debug!(target:"sync","send header req start: {} , size: {} , node_hash: {}", start, REQUEST_SIZE,node.hash);
//             let mut cb = ChannelBuffer::new();
//             cb.head.ver = VERSION::V0.value();
//             cb.head.ctrl = MODULE::SYNC.value();
//             cb.head.action = ACTION::HEADERSREQ.value();

//             let mut from_buf = [0u8; 8];
//             BigEndian::write_u64(&mut from_buf, start);
//             cb.body.put_slice(&from_buf);

//             let mut size_buf = [0u8; 4];
//             BigEndian::write_u32(&mut size_buf, REQUEST_SIZE);
//             cb.body.put_slice(&size_buf);

//             cb.head.len = cb.body.len() as u32;
//             p2p.send(p2p.clone(), node.hash, cb);
//         }
//     }
// }

pub fn receive_req(p2p: Mgr, hash: u64, client: Arc<BlockChainClient>, cb_in: ChannelBuffer) {
    trace!(target: "sync", "headers/receive_req");

    let mut res = ChannelBuffer::new();

    res.head.ver = VERSION::V0.value();
    res.head.ctrl = MODULE::SYNC.value();
    res.head.action = ACTION::HEADERSRES.value();

    let mut res_body = Vec::new();

    let (mut from, req_body_rest) = cb_in.body.split_at(mem::size_of::<u64>());
    let from = from.read_u64::<BigEndian>().unwrap_or(1);
    let (mut size, _) = req_body_rest.split_at(mem::size_of::<u32>());
    let size = size.read_u32::<BigEndian>().unwrap_or(1);
    let mut data = Vec::new();

    if size <= LARGE_REQUEST_SIZE {
        for i in from..(from + size as u64) {
            match client.block_header(BlockId::Number(i)) {
                Some(hdr) => {
                    data.append(&mut hdr.into_inner());
                }
                None => {
                    break;
                }
            }
        }

        if data.len() > 0 {
            let mut rlp = RlpStream::new_list(data.len() as usize);
            rlp.append_raw(&data, data.len() as usize);
            res_body.put_slice(rlp.as_raw());
        }

        res.body.put_slice(res_body.as_slice());
        res.head.len = res.body.len() as u32;

        p2p.update_node(&hash);
        p2p.send(hash, res);
    } else {
        warn!(target:"sync","headers/receive_req max headers size requested");
        return;
    }
}

pub fn receive_res(
    p2p: Mgr,
    hash: u64,
    cb_in: ChannelBuffer,
    downloaded_headers: &Mutex<VecDeque<HeaderWrapper>>,
    cached_downloaded_block_hashes: Arc<Mutex<LruCache<H256, u8>>>,
    cached_imported_block_hashes: Arc<Mutex<LruCache<H256, u8>>>,
)
{
    trace!(target: "sync", "headers/receive_res");

    let rlp = UntrustedRlp::new(cb_in.body.as_slice());
    let mut prev_header = Header::new();
    let mut header_wrapper = HeaderWrapper::new();
    let mut headers = Vec::new();
    for header_rlp in rlp.iter() {
        if let Ok(header) = header_rlp.as_val() {
            let result = UnityEngine::validate_block_header(&header);
            match result {
                Ok(()) => {
                    // break if not consisting
                    if prev_header.number() != 0
                        && (header.number() != prev_header.number() + 1
                            || prev_header.hash() != *header.parent_hash())
                    {
                        error!(target: "sync",
                            "<inconsistent-block-headers num={}, prev+1={}, hash={}, p_hash={}>, hash={}>",
                            header.number(),
                            prev_header.number() + 1,
                            header.parent_hash(),
                            prev_header.hash(),
                            header.hash(),
                        );
                        break;
                    } else {
                        let block_hash = header.hash();

                        // let number = header.number();

                        // Skip staged block header
                        // if node.mode == Mode::THUNDER {
                        //     if SyncStorage::is_staged_block_hash(hash) {
                        //         debug!(target: "sync", "Skip staged block header #{}: {:?}", number, hash);
                        //         // hw.headers.push(header.clone());
                        //         break;
                        //     }
                        // }
                        // TODO: to do better
                        let is_downloaded =
                            if let Ok(mut hashes) = cached_downloaded_block_hashes.lock() {
                                hashes.contains_key(&block_hash)
                            } else {
                                warn!(target: "sync", "downloaded_block_hashes lock failed");
                                false
                            };
                        let is_imported =
                            if let Ok(mut hashes) = cached_imported_block_hashes.lock() {
                                hashes.contains_key(&block_hash)
                            } else {
                                warn!(target: "sync", "imported_block_hashes lock failed");
                                false
                            };

                        if !is_downloaded && !is_imported {
                            headers.push(header.clone());
                        }
                    }
                    prev_header = header;
                }
                Err(e) => {
                    // ignore this batch if any invalidated header
                    error!(target: "sync", "Invalid header: {:?}, header: {}", e, to_hex(header_rlp.as_raw()));
                }
            }
        } else {
            error!(target: "sync", "Invalid header: {}", to_hex(header_rlp.as_raw()));
        }
    }

    if !headers.is_empty() {
        header_wrapper.node_hash = hash;
        header_wrapper.headers = headers;
        header_wrapper.timestamp = SystemTime::now();
        p2p.update_node(&hash);
        if let Ok(mut downloaded_headers) = downloaded_headers.lock() {
            downloaded_headers.push_back(header_wrapper);
        } else {
            println!("!!!!!!!!!!!!!")
        }
    } else {
        debug!(target: "sync", "Came too late............");
    }
}

fn filter_nodes_to_sync_headers(
    nodes: Vec<Node>,
    nodes_info: Arc<RwLock<HashMap<u64, NodeInfo>>>,
    local_total_diff: &U256,
) -> Vec<Node>
{
    let time_now = SystemTime::now();
    match nodes_info.read() {
        Ok(nodes_info_read) => {
            nodes
                .into_iter()
                .filter(|node| {
                    let node_hash = node.get_hash();
                    nodes_info_read.get(&node_hash).map_or(false, |node_info| {
                        &node_info.total_difficulty > local_total_diff
                            && node_info.last_headers_request_time
                                + Duration::from_millis(REQUEST_COOLDOWN)
                                <= time_now
                    })
                })
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

fn pick_random_node(nodes: &Vec<Node>) -> Option<Node> {
    let count = nodes.len();
    if count > 0 {
        let mut rng = thread_rng();
        let random_index: usize = rng.gen_range(0, count);
        Some(nodes[random_index].clone())
    } else {
        None
    }
}
