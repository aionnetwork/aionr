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

use acore::client::BlockId;
use acore::engines::pow_equihash_engine::POWEquihashEngine;
use acore::header::Header as BlockHeader;
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::BufMut;
use rlp::{RlpStream, UntrustedRlp};
use std::mem;
use std::time::{Duration, SystemTime};

use super::super::action::SyncAction;
use super::super::event::SyncEvent;
use super::super::storage::{HeadersWrapper, SyncStorage};

use p2p::*;

const BACKWARD_SYNC_STEP: u64 = 64;
const REQUEST_SIZE: u64 = 24;
const LARGE_REQUEST_SIZE: u64 = 48;

pub struct BlockHeadersHandler;

impl BlockHeadersHandler {
    pub fn get_headers_from_random_node() {
        if let Some(mut node) = P2pMgr::get_an_active_node() {
            if node.synced_block_num == 0 {
                node.synced_block_num = SyncStorage::get_synced_block_number() + 1;
            }
            BlockHeadersHandler::get_headers_from_node(&mut node);
        }
    }

    pub fn get_headers_from_node(node: &mut Node) {
        trace!(target: "sync", "get_headers_from_node, node id: {}", node.get_node_id());

        if P2pMgr::get_network_config().sync_from_boot_nodes_only && !node.is_from_boot_list {
            return;
        }

        if node.last_request_timestamp + Duration::from_millis(1000) > SystemTime::now() {
            return;
        }

        if node.target_total_difficulty > node.current_total_difficulty {
            let mut from: u64 = 1;
            let mut size = REQUEST_SIZE;

            match node.mode {
                Mode::LIGHTNING => {
                    // request far forward blocks
                    let mut self_num;
                    let max_staged_block_number = SyncStorage::get_max_staged_block_number();
                    let synced_block_number = SyncStorage::get_synced_block_number();
                    if synced_block_number + LARGE_REQUEST_SIZE * 5 > max_staged_block_number {
                        let sync_speed = SyncStorage::get_sync_speed();
                        let jump_size = if sync_speed <= 40 {
                            400
                        } else if sync_speed > 40 && sync_speed <= 80 {
                            sync_speed as u64 * 12
                        } else if sync_speed > 80 && sync_speed <= 120 {
                            sync_speed as u64 * 10
                        } else {
                            1200
                        };
                        self_num = synced_block_number + jump_size;
                    } else {
                        self_num = max_staged_block_number + 1;
                    }
                    if node.best_block_num > self_num + LARGE_REQUEST_SIZE {
                        size = LARGE_REQUEST_SIZE;
                        from = self_num;
                    } else {
                        // transition to ramp down strategy
                        node.mode = Mode::THUNDER;
                        return;
                    }
                }
                Mode::THUNDER => {
                    let mut self_num = SyncStorage::get_synced_block_number();
                    size = LARGE_REQUEST_SIZE;
                    from = if self_num > 4 { self_num - 3 } else { 1 };
                }
                Mode::NORMAL => {
                    let self_num = SyncStorage::get_synced_block_number();
                    let node_num = node.best_block_num;

                    if node_num >= self_num + BACKWARD_SYNC_STEP {
                        from = if self_num > 4 { self_num - 3 } else { 1 };
                    } else if self_num < BACKWARD_SYNC_STEP {
                        from = if self_num > 16 { self_num - 15 } else { 1 };
                    } else if node_num >= self_num - BACKWARD_SYNC_STEP {
                        from = self_num - 16;
                    } else {
                        return;
                    }
                }
                Mode::BACKWARD => {
                    let self_num = node.synced_block_num;
                    if self_num > BACKWARD_SYNC_STEP {
                        from = self_num - BACKWARD_SYNC_STEP;
                    }
                }
                Mode::FORWARD => {
                    let self_num = node.synced_block_num;
                    from = self_num + 1;
                }
            };

            if node.last_request_num != from {
                node.last_request_timestamp = SystemTime::now();
            }
            node.last_request_num = from;

            debug!(target: "sync", "request headers: from number: {}, node: {}, sn: {}, mode: {}.", from, node.get_ip_addr(), node.synced_block_num, node.mode);

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

    pub fn handle_blocks_headers_req(node: &mut Node, req: ChannelBuffer) {
        trace!(target: "sync", "BLOCKSHEADERSREQ received.");

        let client = SyncStorage::get_block_chain();

        let mut res = ChannelBuffer::new();
        let node_hash = node.node_hash;

        res.head.ver = Version::V0.value();
        res.head.ctrl = Control::SYNC.value();
        res.head.action = SyncAction::BLOCKSHEADERSRES.value();

        let mut res_body = Vec::new();

        let (mut from, req_body_rest) = req.body.split_at(mem::size_of::<u64>());
        let from = from.read_u64::<BigEndian>().unwrap_or(1);
        let (mut size, _) = req_body_rest.split_at(mem::size_of::<u32>());
        let size = size.read_u32::<BigEndian>().unwrap_or(1);
        let chain_info = client.chain_info();
        let last = chain_info.best_block_number;

        let mut header_count = 0;
        let number = from;
        let mut data = Vec::new();
        while number + header_count <= last && header_count < size.into() {
            match client.block_header(BlockId::Number(number + header_count)) {
                Some(hdr) => {
                    data.append(&mut hdr.into_inner());
                    header_count += 1;
                }
                None => {}
            }
        }

        if header_count > 0 {
            let mut rlp = RlpStream::new_list(header_count as usize);

            rlp.append_raw(&data, header_count as usize);
            res_body.put_slice(rlp.as_raw());
        }

        res.body.put_slice(res_body.as_slice());
        res.head.set_length(res.body.len() as u32);

        SyncEvent::update_node_state(node, SyncEvent::OnBlockHeadersReq);
        P2pMgr::update_node(node_hash, node);
        P2pMgr::send(node_hash, res);
    }

    pub fn handle_blocks_headers_res(node: &mut Node, req: ChannelBuffer) {
        trace!(target: "sync", "BLOCKSHEADERSRES received.");

        let node_hash = node.node_hash;
        let rlp = UntrustedRlp::new(req.body.as_slice());
        let mut prev_header = BlockHeader::new();
        let mut hw = HeadersWrapper::new();

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

                            // Skip staged block header
                            if node.mode == Mode::THUNDER {
                                if SyncStorage::is_staged_block_hash(hash) {
                                    debug!(target: "sync", "Skip staged block header #{}: {:?}", number, hash);
                                    // hw.headers.push(header.clone());
                                    break;
                                }
                            }

                            if !SyncStorage::is_imported_block_hash(&hash) {
                                hw.headers.push(header.clone());
                            }
                        }
                        prev_header = header;
                    }
                    Err(e) => {
                        // ignore this batch if any invalidated header
                        error!(target: "sync", "Invalid header: {:?}, header: {}", e, header_rlp);
                    }
                }
            } else {
                error!(target: "sync", "Invalid header: {:?}", header_rlp);
            }
        }

        if !hw.headers.is_empty() {
            hw.node_hash = node_hash;
            hw.timestamp = SystemTime::now();
            SyncStorage::insert_downloaded_headers(hw);
        } else {
            debug!(target: "sync", "Came too late............");
        }

        SyncEvent::update_node_state(node, SyncEvent::OnBlockHeadersRes);
        P2pMgr::update_node(node_hash, node);
    }
}
