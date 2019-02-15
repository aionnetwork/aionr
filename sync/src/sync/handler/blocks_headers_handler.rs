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

use acore::client::BlockStatus;
use acore::engines::pow_equihash_engine::POWEquihashEngine;
use acore::header::Header as BlockHeader;
use acore_bytes::to_hex;
use byteorder::{BigEndian, ByteOrder};
use bytes::BufMut;
use kvdb::DBTransaction;
use rlp::UntrustedRlp;
use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

use super::super::action::SyncAction;
use super::super::event::SyncEvent;
use super::super::storage::{SyncStorage, MAX_CACHED_BLOCK_HASHED};

use p2p::*;

const BACKWARD_SYNC_STEP: u64 = 128;

pub struct BlockHeadersHandler;

impl BlockHeadersHandler {
    pub fn get_headers_from_node(node: &mut Node, mut from: u64, size: u64) {
        trace!(target: "sync", "get_headers_from_node, node id: {}", node.get_node_id());

        if P2pMgr::get_network_config().sync_from_boot_nodes_only && !node.is_from_boot_list {
            return;
        }

        if node.last_request_timestamp + Duration::from_millis(50) > SystemTime::now() {
            return;
        }

        if node.requested_block_num == 0 {
            node.requested_block_num = SyncStorage::get_synced_block_number() + 1;
        }

        if SyncStorage::get_synced_block_number() + ((MAX_CACHED_BLOCK_HASHED / 4) as u64)
            <= node.requested_block_num
        {
            debug!(target: "sync", "get_headers_from_node, {} - {}", SyncStorage::get_synced_block_number(), node.requested_block_num);

            return;
        }

        if node.target_total_difficulty > node.current_total_difficulty {
            if from == 0 {
                match node.mode {
                    Mode::NORMAL => {
                        if node.requested_block_num + 128 < SyncStorage::get_synced_block_number() {
                            node.requested_block_num = SyncStorage::get_synced_block_number() + 128;
                        }

                        let self_num = node.requested_block_num;
                        from = if self_num > 2 { self_num - 1 } else { 1 };
                    }
                    Mode::BACKWARD => {
                        let self_num = node.requested_block_num;
                        if self_num > BACKWARD_SYNC_STEP {
                            from = self_num - BACKWARD_SYNC_STEP;
                        }
                    }
                    Mode::FORWARD => {
                        let self_num = node.requested_block_num;
                        from = self_num + 1;
                    }
                };
            }

            if node.last_request_num == from {
                return;
            } else {
                node.last_request_timestamp = SystemTime::now();
            }
            node.last_request_num = from;

            info!(target: "sync", "request headers: from number: {}, node: {}, rn: {}, mode: {}.", from, node.get_ip_addr(), node.requested_block_num, node.mode);

            SyncStorage::set_requested_block_number_last_time(from + size);
            Self::send_blocks_headers_req(node.node_hash, from, size as u32);
            P2pMgr::update_node(node.node_hash, node);
        }
    }

    fn send_blocks_headers_req(node_hash: u64, from: u64, size: u32) {
        let mut req = ChannelBuffer::new();
        req.head.ver = Version::V0.value();
        req.head.ctrl = Control::SYNC.value();
        req.head.action = SyncAction::BLOCKSHEADERSREQ.value();

        let mut from_buf = [0; 8];
        BigEndian::write_u64(&mut from_buf, from);
        req.body.put_slice(&from_buf);

        let mut size_buf = [0; 4];
        BigEndian::write_u32(&mut size_buf, size);
        req.body.put_slice(&size_buf);

        req.head.len = req.body.len() as u32;

        P2pMgr::send(node_hash, req);
    }

    pub fn handle_blocks_headers_req(_node: &mut Node, _req: ChannelBuffer) {
        trace!(target: "sync", "BLOCKSHEADERSREQ received.");
    }

    pub fn handle_blocks_headers_res(node: &mut Node, req: ChannelBuffer) {
        trace!(target: "sync", "BLOCKSHEADERSRES received.");

        let node_hash = node.node_hash;
        let rlp = UntrustedRlp::new(req.body.as_slice());
        let mut prev_header = BlockHeader::new();
        let mut headers = VecDeque::new();

        for header_rlp in rlp.iter() {
            if let Ok(header) = header_rlp.as_val() {
                let result = POWEquihashEngine::validate_block_header(&header);
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
                            let hash = header.hash();
                            let number = header.number();

                            if number <= SyncStorage::get_synced_block_number() {
                                debug!(target: "sync", "Imported header: {} - {:?}.", number, hash);
                            } else {
                                //if SyncStorage::is_block_hash_confirmed(hash) {
                                headers.push_back(header.clone());

                                // if let Ok(header_chain) =
                                //     SyncStorage::get_block_header_chain().read()
                                // {
                                //     if header_chain.status(header.parent_hash())
                                //         != BlockStatus::InChain
                                //         || header_chain.status(&hash) == BlockStatus::InChain
                                //     {
                                //         break;
                                //     }
                                //     let mut tx = DBTransaction::new();
                                //     if let Ok(pending) = header_chain.insert(&mut tx, &header, None)
                                //     {
                                //         header_chain.apply_pending(tx, pending);
                                //         SyncStorage::set_synced_block_number(number);
                                //         // info!(target: "sync", "New block header #{} - {}, imported.", number, hash);
                                //     }
                                // }

                                debug!(target: "sync", "Confirmed header: {} - {:?}, to be imported.", number, hash);
                            }
                            // else {
                            //     debug!(target: "sync", "Downloaded header: {} - {:?}, under confirmation.", number, hash);
                            // }
                            if node.requested_block_num < number {
                                node.requested_block_num = number;
                            }
                        }
                        prev_header = header;
                    }
                    Err(e) => {
                        // ignore this batch if any invalidated header
                        error!(target: "sync", "Invalid header: {:?}, header: {}, received from {}@{}", e, to_hex(header_rlp.as_raw()), node.get_node_id(), node.get_ip_addr());
                        P2pMgr::remove_peer(node.node_hash);
                    }
                }
            } else {
                error!(target: "sync", "Invalid header: {}, received from {}@{}", to_hex(header_rlp.as_raw()), node.get_node_id(), node.get_ip_addr());
                P2pMgr::remove_peer(node.node_hash);
            }
        }

        if !headers.is_empty() {
            node.inc_reputation(10);
            if let Ok(mut downloaded_headers) = SyncStorage::get_downloaded_headers().lock() {
                for header in headers.iter() {
                    if ! downloaded_headers.contains(header) {
                        downloaded_headers.push_back(header.clone());
                    }
                }
            }
        } else {
            node.inc_reputation(1);
            debug!(target: "sync", "Came too late............");
        }

        SyncEvent::update_node_state(node, SyncEvent::OnBlockHeadersRes);
        P2pMgr::update_node(node_hash, node);
    }

    pub fn import_block_header() {
        let mut headers = Vec::new();
        if let Ok(ref mut downloaded_headers) = SyncStorage::get_downloaded_headers().try_lock() {
            while let Some(header) = downloaded_headers.pop_front() {
                headers.push(header);
            }
        }

        let header_chain = SyncStorage::get_block_header_chain();
        for header in headers.iter() {
            let hash = header.hash();
            let number = header.number();
            let parent_hash = header.parent_hash();

            if header_chain.status(parent_hash) != BlockStatus::InChain
                || header_chain.status(&hash) == BlockStatus::InChain
            {
                break;
            }
            let mut tx = DBTransaction::new();
            if let Ok(pending) = header_chain.insert(&mut tx, &header, None) {
                header_chain.apply_pending(tx, pending);
                SyncStorage::set_synced_block_number(number);
                // info!(target: "sync", "New block header #{} - {}, imported.", number, hash);
            }

            /*
                if let Ok(ref mut downloaded_blocks) = SyncStorage::get_downloaded_blocks().lock() {
                    if number == 1 || number == SyncStorage::get_starting_block_number() {
                    } else if let Some(parent_bw) = downloaded_blocks.get_mut(&(number - 1)) {
                        if parent_bw
                            .block_hashes
                            .iter()
                            .filter(|h| *h == parent_hash)
                            .next()
                            .is_none()
                        {
                            continue;
                        }
                    } else {
                        debug!(target: "sync", "number {}, starting_block_number: {}", number, SyncStorage::get_starting_block_number());
                        continue;
                    }

                    if let Some(bw_old) = downloaded_blocks.get_mut(&number) {
                        if &bw_old.parent_hash == parent_hash {
                            let mut index = 0;
                            for h in bw_old.block_hashes.iter() {
                                if h == &hash {
                                    debug!(target: "sync", "Already imported block header #{}-{}", number, hash);
                                    continue;
                                }
                                index += 1;
                            }

                            if index == bw_old.block_hashes.len() {
                                bw_old.block_hashes.extend(vec![hash]);

                                count += 1;
                                local_status.total_difficulty =
                                    local_status.total_difficulty + header.difficulty().clone();
                                local_status.synced_block_number = number;
                                local_status.synced_block_hash = hash;
                                debug!(target: "sync", "Block header #{} - {:?} imported(side chain against {:?}).", number, hash, bw_old.block_hashes);
                            }
                        }
                        continue;
                    }

                    let bw = BlockWrapper {
                        block_number: number,
                        parent_hash: header.parent_hash().clone(),
                        block_hashes: vec![hash],
                        block_headers: None,
                    };

                    downloaded_blocks.insert(number, bw);

                    count += 1;
                    if number > 0 {
                        hw.node_hash = node_hash;
                        hw.hashes.push(hash);
                        hw.headers.push(header.clone());
                    }
                    local_status.total_difficulty =
                        local_status.total_difficulty + header.difficulty().clone();
                    local_status.synced_block_number = number;
                    local_status.synced_block_hash = hash;

                    debug!(target: "sync", "Block header #{} - {:?} imported", number, hash);
                }
                */
        }
    }
}
