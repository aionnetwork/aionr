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
use acore::header::Header as BlockHeader;
use acore_bytes::to_hex;
use byteorder::{BigEndian, ByteOrder};
use bytes::BufMut;
use futures::future::lazy;
use kvdb::DBTransaction;
use rlp::UntrustedRlp;
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

            debug!(target: "sync", "request headers: from number: {}, node: {}, rn: {}, mode: {}.", from, node.get_ip_addr(), node.requested_block_num, node.mode);

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
        let mut headers = Vec::new();

        for header_rlp in rlp.iter() {
            if let Ok(hd) = header_rlp.as_val() {
                let header: BlockHeader = hd;
                let hash = header.hash();
                let number = header.number();

                if number <= SyncStorage::get_synced_block_number() {
                    debug!(target: "sync", "Imported header: {} - {:?}.", number, hash);
                } else {
                    headers.push(header.clone());
                }
                if node.requested_block_num < number {
                    node.requested_block_num = number;
                }
            } else {
                error!(target: "sync", "Invalid header: {}, received from {}@{}", to_hex(header_rlp.as_raw()), node.get_node_id(), node.get_ip_addr());
                P2pMgr::remove_peer(node.node_hash);
                return;
            }
        }

        if !headers.is_empty() {
            node.inc_reputation(10);
            let node_ip = node.get_ip_addr();
            let executor = SyncStorage::get_sync_executor();
            executor.spawn(lazy(move || {
                Self::import_block_header(headers, node_ip);
                Ok(())
            }));
        } else {
            node.inc_reputation(1);
            debug!(target: "sync", "Came too late............");
        }

        SyncEvent::update_node_state(node, SyncEvent::OnBlockHeadersRes);
        P2pMgr::update_node(node_hash, node);
    }

    pub fn import_block_header(headers: Vec<BlockHeader>, node_ip: String) {
        let mut imported = false;
        let header_chain = SyncStorage::get_block_header_chain();
        for header in headers.iter() {
            let hash = header.hash();
            let number = header.number();
            let parent_hash = header.parent_hash();

            if header_chain.status(parent_hash) != BlockStatus::InChain
                || header_chain.status(&hash) == BlockStatus::InChain
            {
                continue;
            }
            let mut tx = DBTransaction::new();
            if let Ok(pending) = header_chain.insert(&mut tx, &header, None) {
                header_chain.apply_pending(tx, pending);
                SyncStorage::set_synced_block_number(number);
                // info!(target: "sync", "New block header #{} - {}, imported.", number, hash);
                imported = true;
            }
        }
        if imported {
            info!(target: "sync", "Import headers from: {}", node_ip);
        }
    }
}
