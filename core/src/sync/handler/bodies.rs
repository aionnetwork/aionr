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

use std::sync::{RwLock,Arc};
use std::collections::{HashMap};
use block::Block;
use client::{BlockId};
use header::{Seal,Header};
use aion_types::H256;
use bytes::BufMut;
use rlp::{RlpStream, UntrustedRlp};
use std::time::SystemTime;
use p2p::ChannelBuffer;
use p2p::Node;
use p2p::Mgr;
use sync::route::VERSION;
use sync::route::MODULE;
use sync::route::ACTION;
use sync::helper::{Wrapper,WithStatus};
use sync::storage::SyncStorage;
use sync::handler::headers;
use sync::handler::headers::REQUEST_SIZE;

const HASH_LEN: usize = 32;

pub fn send(p2p: Arc<Mgr>, hash: u64, hashes: Vec<u8>) {
    let mut cb = ChannelBuffer::new();
    cb.head.ver = VERSION::V0.value();
    cb.head.ctrl = MODULE::SYNC.value();
    cb.head.action = ACTION::BODIESREQ.value();
    cb.body = hashes;
    cb.head.len = cb.body.len() as u32;
    p2p.send(p2p.clone(), hash.clone(), cb);
}

pub fn receive_req(p2p: Arc<Mgr>, hash: u64, cb_in: ChannelBuffer) {
    trace!(target: "sync", "bodies/receive_req");

    let mut res = ChannelBuffer::new();

    res.head.ver = VERSION::V0.value();
    res.head.ctrl = MODULE::SYNC.value();
    res.head.action = ACTION::BODIESRES.value();

    let mut res_body = Vec::new();
    let hash_count = cb_in.body.len() / HASH_LEN;
    let mut rest = cb_in.body.as_slice();
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
    res.head.len = res.body.len() as u32;

    p2p.update_node(&hash);

    p2p.send(p2p.clone(), hash, res);
}

pub fn receive_res(
    p2p: Arc<Mgr>,
    hash: u64,
    cb_in: ChannelBuffer,
    queue: Arc<RwLock<HashMap<u64, Wrapper>>>,
)
{
    trace!(target: "sync", "bodies/receive_res");
    if cb_in.body.len() > 0 {
        if let Ok(mut wrappers) = queue.write() {
            if let Some(mut wrapper) = wrappers.get_mut(&hash) {
                if wrapper.with_status == WithStatus::GetHeader {
                    let headers = wrapper.data.clone();
                    if !headers.is_empty() {
                        let rlp = UntrustedRlp::new(cb_in.body.as_slice());

                        let mut bodies = Vec::new();
                        let mut blocks = Vec::new();
                        for block_bodies in rlp.iter() {
                            for block_body in block_bodies.iter() {
                                let mut transactions = Vec::new();
                                if !block_body.is_empty() {
                                    for transaction_rlp in block_body.iter() {
                                        if !transaction_rlp.is_empty() {
                                            if let Ok(transaction) = transaction_rlp.as_val() {
                                                transactions.push(transaction);
                                            }
                                        }
                                    }
                                }
                                bodies.push(transactions);
                            }
                        }

                        if headers.len() == bodies.len() {
                            for i in 0..headers.len() {
                                let rlp = UntrustedRlp::new(&headers[i]);
                                let header: Header = rlp.as_val().expect("should be a head");
                                let block = Block {
                                    header,
                                    transactions: bodies[i].clone(),
                                };
                                blocks.push(block.rlp_bytes(Seal::Without));
                                //                                        if let Ok(mut downloaded_block_hashes) =
                                //                                        SyncStorage::get_downloaded_block_hashes().lock()
                                {
                                    //                                                let hash = block.header.hash();
                                    //                                                if !downloaded_block_hashes.contains_key(&hash) {
                                    //                                                    downloaded_block_hashes.insert(hash, 0);
                                    //                                                } else {
                                    //                                                    trace!(target: "sync", "downloaded_block_hashes: {}.", hash);
                                    //                                                }
                                }
                            }
                        } else {
                            debug!(
                                        target: "sync",
                                        "Count mismatch, headers count: {}, bodies count: {}",
                                        headers.len(),
                                        bodies.len(),
                                    );
                            // TODO: punish the node

                            blocks.clear();
                        }

                        if !blocks.is_empty() {
                            p2p.update_node(&hash);

                            wrapper.timestamp = SystemTime::now();
                            wrapper.with_status = WithStatus::GetBody;
                            wrapper.data = blocks;
                        }
                    }
                } else {
                    error!(target:"sync","bodies: should not be reached!!")
                }
            }
        }
    }
}
