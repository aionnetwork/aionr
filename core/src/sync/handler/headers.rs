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
use std::time::{ SystemTime};
use std::sync::{RwLock,Arc};
use std::collections::{HashMap};
use engine::unity_engine::UnityEngine;
use header::{Header as BlockHeader,Seal};
use acore_bytes::to_hex;
use client::{BlockChainClient, BlockId};
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::BufMut;
use rlp::{RlpStream, UntrustedRlp};
use p2p::ChannelBuffer;
use p2p::Node;
use p2p::Mgr;
use sync::handler::bodies;
use sync::route::VERSION;
use sync::route::MODULE;
use sync::route::ACTION;
use sync::header_wrapper::{HeaderWrapper};

pub const NORMAL_REQUEST_SIZE: u32 = 24;
const LARGE_REQUEST_SIZE: u32 = 48;

pub fn prepare_send(p2p: Arc<Mgr>, hash: u64, synced_number: Arc<RwLock<u64>> /*mode:Mode*/) {
    // TODO mode match
    if let Ok(synced_number) = synced_number.read() {
        let start = if *synced_number > 3 {
            *synced_number - 3
        } else {
            1
        };
        let size = NORMAL_REQUEST_SIZE;

        send(p2p.clone(), hash, start, size);
    }
}

fn send(p2p: Arc<Mgr>, hash: u64, start: u64, size: u32) {
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
    p2p.send(p2p.clone(), hash, cb);
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

pub fn receive_req(p2p: Arc<Mgr>, hash: u64, client: Arc<BlockChainClient>, cb_in: ChannelBuffer) {
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

    let chain_info = client.chain_info();
    let last = chain_info.best_block_number;

    let mut header_count = 0;
    let number = from;
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
            let mut rlp = RlpStream::new_list(header_count as usize);
            rlp.append_raw(&data, header_count as usize);
            res_body.put_slice(rlp.as_raw());
        }

        res.body.put_slice(res_body.as_slice());
        res.head.len = res.body.len() as u32;

        p2p.update_node(&hash);
        p2p.send(p2p.clone(), hash, res);
    } else {
        warn!(target:"sync","headers/receive_req max headers size requested");
        return;
    }
}

pub fn receive_res(
    p2p: Arc<Mgr>,
    hash: u64,
    cb_in: ChannelBuffer,
    hws: Arc<RwLock<HashMap<u64, HeaderWrapper>>>,
)
{
    trace!(target: "sync", "headers/receive_res");

    let rlp = UntrustedRlp::new(cb_in.body.as_slice());
    let mut prev_header = BlockHeader::new();
    let mut hw = HeaderWrapper::new();
    let mut headers = Vec::new();
    let mut hashes = Vec::new();
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
                        //                        let hash = header.hash();
                        //                        let number = header.number();

                        // Skip staged block header
                        //                        if node.mode == Mode::THUNDER {
                        //                            if SyncStorage::is_staged_block_hash(hash) {
                        //                                debug!(target: "sync", "Skip staged block header #{}: {:?}", number, hash);
                        //                                // hw.headers.push(header.clone());
                        //                                break;
                        //                            }
                        //                        }

                        //                        if !SyncStorage::is_downloaded_block_hashes(&hash)
                        //                            && !SyncStorage::is_imported_block_hash(&hash)
                        //                        {
                        hashes.put_slice(&header.hash());
                        headers.push(header.clone().rlp(Seal::Without));
                        //                        }
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
        hw.headers = headers;
        hw.timestamp = SystemTime::now();
        p2p.update_node(&hash);
        if let Ok(mut hws) = hws.try_write() {
            hws.insert(hash.clone(), hw);
            bodies::send(p2p.clone(), hash, hashes);
        }
    } else {
        debug!(target: "sync", "Came too late............");
    }
}
