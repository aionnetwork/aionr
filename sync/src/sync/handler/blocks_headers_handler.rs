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

use acore::client::{BlockId, BlockStatus};
use acore::header::Header as BlockHeader;
use acore_bytes::to_hex;
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::BufMut;
use kvdb::DBTransaction;
use rlp::{RlpStream, UntrustedRlp};
use std::mem;
use std::time::{Duration, SystemTime};

use super::super::action::SyncAction;
use super::super::event::{SyncEvent, STATUS_GOT};
use super::super::storage::SyncStorage;

use p2p::*;

const REQUEST_SIZE: u64 = 96;

pub struct BlockHeadersHandler;

impl BlockHeadersHandler {
    pub fn get_headers(mut from: u64) {
        if let Some(mut node) = P2pMgr::get_an_active_node() {
            if P2pMgr::get_network_config().sync_from_boot_nodes_only
                && !node.is_from_boot_list
                && node.state_code & STATUS_GOT == STATUS_GOT
            {
                return;
            }

            if node.target_total_difficulty >= node.current_total_difficulty {
                if node.last_request_timestamp + Duration::from_millis(50) > SystemTime::now() {
                    return;
                }
                if from == 0 {
                    from = SyncStorage::get_block_header_chain()
                        .chain_info()
                        .best_block_number + 1;
                    if SyncStorage::get_synced_block_number() + 512 < from {
                        return;
                    }
                }

                if node.requested_block_num == from {
                    return;
                } else {
                    node.last_request_timestamp = SystemTime::now();
                }
                node.requested_block_num = from;

                debug!(target: "sync", "request headers: from number: {}, node: {}, rn: {}.", from, node.get_ip_addr(), node.requested_block_num);

                Self::send_blocks_headers_req(node.node_hash, from, REQUEST_SIZE as u32);
                SyncStorage::set_requested_block_number_last_time(from + REQUEST_SIZE as u64);
                P2pMgr::update_node(node.node_hash, &mut node);
            }
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
        let header_chain = SyncStorage::get_block_header_chain();
        let mut hases = Vec::new();

        for header_rlp in rlp.iter() {
            if let Ok(hd) = header_rlp.as_val() {
                let header: BlockHeader = hd;
                let hash = header.hash();
                let number = header.number();
                let parent_hash = header.parent_hash();

                if number < SyncStorage::get_synced_block_number() {
                    trace!(target: "sync", "Imported header: {} - {:?}.", number, hash);
                } else {
                    if header_chain.status(parent_hash) == BlockStatus::InChain {
                        if header_chain.status(&hash) != BlockStatus::InChain {
                            let mut tx = DBTransaction::new();
                            if let Ok(pending) = header_chain.insert(&mut tx, &header, None) {
                                header_chain.apply_pending(tx, pending);
                                hases.push(hash);
                                trace!(target: "sync", "New block header #{} - {}, imported from {}@{}.", number, hash, node.get_ip_addr(), node.get_node_id());
                            }
                        } else {
                            trace!(target: "sync", "The block is inchain already.");
                        }
                    } else {
                        if number <= header_chain.chain_info().best_block_number {
                            if node.target_total_difficulty >= SyncStorage::get_network_total_diff()
                            {
                                info!(target: "sync", "Side chain found from {}@{}.", node.get_ip_addr(), node.get_node_id());
                                let from = if number > REQUEST_SIZE {
                                    number - REQUEST_SIZE
                                } else {
                                    1
                                };
                                BlockHeadersHandler::get_headers(from);
                            } else {
                                P2pMgr::remove_peer(node_hash);
                                P2pMgr::add_black_ip(node.get_ip());
                            }
                        }
                    }
                }
                if node.requested_block_num < number {
                    node.requested_block_num = number;
                }
            } else {
                error!(target: "sync", "Invalid header: {}, received from {}@{}", to_hex(header_rlp.as_raw()), node.get_node_id(), node.get_ip_addr());
                P2pMgr::remove_peer(node.node_hash);
                P2pMgr::add_black_ip(node.get_ip());
                info!(target: "sync", "header removed.");
                return;
            }
        }

        SyncEvent::update_node_state(node, SyncEvent::OnBlockHeadersRes);
        P2pMgr::update_node(node_hash, node);

        BlockHeadersHandler::get_headers(0);
    }
}
