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

use acore::client::{BlockChainClient, BlockId, BlockStatus};
use acore::header::Header as BlockHeader;
use acore_bytes::to_hex;
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::BufMut;
use kvdb::DBTransaction;
use parking_lot::Mutex;
use rlp::{RlpStream, UntrustedRlp};
use std::mem;
use std::time::{Duration, SystemTime};

use super::super::action::SyncAction;
use super::super::event::{SyncEvent, STATUS_GOT};
use super::super::storage::SyncStorage;
use super::blocks_bodies_handler::BlockBodiesHandler;

use p2p::*;

lazy_static! {
    static ref MUTEX: Mutex<u8> = { Mutex::new(0) };
}

const REQUEST_SIZE: u64 = 32;

pub struct BlockHeadersHandler;

impl BlockHeadersHandler {
    pub fn get_headers(node: &mut Node, mut from: u64) {
        if from == 0 && node.last_request_timestamp + Duration::from_millis(50) > SystemTime::now()
        {
            return;
        }
        if P2pMgr::get_network_config().sync_from_boot_nodes_only && !node.is_from_boot_list
            || node.state_code & STATUS_GOT != STATUS_GOT
        {
            return;
        }

        if node.target_total_difficulty >= SyncStorage::get_total_difficulty()
            && node.best_block_num > SyncStorage::get_synced_block_number()
        {
            if from == 0 {
                let header_chain = SyncStorage::get_block_header_chain();
                from = header_chain.best_block().number;

                if from == 0 {
                    from = 1;
                }
            }

            if SyncStorage::get_synced_block_number() + 1024 < from {
                return;
            }

            if node.requested_block_num == from + REQUEST_SIZE {
                node.inc_repeated();
                if node.last_request_timestamp + Duration::from_secs(10) > SystemTime::now() {
                    debug!(target: "sync", "emiyou {}", node.requested_block_num);
                    return;
                }
            } else {
                node.last_request_timestamp = SystemTime::now();
                node.reset_repeated();
            }
            node.requested_block_num = from + REQUEST_SIZE;

            debug!(target: "sync", "request headers: from number: {}, node: {}, rn: {}.", from, node.get_ip_addr(), node.requested_block_num);

            Self::send_blocks_headers_req(node.node_hash, from, REQUEST_SIZE as u32);
        } else {
            node.reset_repeated();
        }
        P2pMgr::update_node(node.node_hash, node);
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
        let block_chain = SyncStorage::get_block_chain();
        let mut hashes = Vec::new();
        let mut from = 0;
        let mut is_side_chain = false;
        let mut number = 0;

        for header_rlp in rlp.iter() {
            node.reset_repeated();
            if let Ok(hd) = header_rlp.as_val() {
                let header: BlockHeader = hd;
                let hash = header.hash();
                number = header.number();
                let parent_hash = header.parent_hash();

                {
                    let _lock = MUTEX.lock();

                    if header_chain.status(parent_hash) == BlockStatus::InChain {
                        if header_chain.status(&hash) != BlockStatus::InChain {
                            if !is_side_chain {
                                if number < SyncStorage::get_synced_block_number() {
                                    is_side_chain = if node.target_total_difficulty
                                        >= SyncStorage::get_network_total_diff()
                                    {
                                        true
                                    } else {
                                        false
                                    };
                                }
                            }

                            let mut total_difficulty =
                                header_chain.score(BlockId::Hash(*parent_hash));
                            if total_difficulty.is_none() {
                                total_difficulty =
                                    block_chain.block_total_difficulty(BlockId::Hash(*parent_hash));
                            }
                            if let Some(total_difficulty) = total_difficulty {
                                if let Ok(num) = header_chain.insert_with_td(
                                    DBTransaction::new(),
                                    &header.encoded(),
                                    Some(total_difficulty + *header.difficulty()),
                                    None,
                                    is_side_chain,
                                ) {
                                    if block_chain.block_status(BlockId::Hash(hash))
                                        != BlockStatus::InChain
                                    {
                                        hashes.push(hash);
                                    }
                                    info!(target: "sync", "New block header #{} - {}, imported from {}@{}.", num, hash, node.get_ip_addr(), node.get_node_id());
                                }
                            } else {
                                warn!(target: "sync", "Late coming block header #{} - {} from {}@{}, {}.", number, hash, node.get_ip_addr(), node.get_node_id(), is_side_chain);
                            }
                        }

                        if is_side_chain {
                            from = number + 1;
                            if node.synced_block_num == 0 {
                                node.synced_block_num = number;
                            }
                        }
                    } else {
                        if number <= header_chain.best_block().number {
                            if node.target_total_difficulty >= SyncStorage::get_network_total_diff()
                            {
                                if header_chain.status(parent_hash) != BlockStatus::InChain
                                    && header_chain.status(&hash) != BlockStatus::InChain
                                {
                                    info!(target: "sync", "Side chain found from {}@{}, #{} - {} with parent #{} - {}.", node.get_ip_addr(), node.get_node_id(), number, hash, number - 1, parent_hash);
                                    from = if number > REQUEST_SIZE {
                                        number - REQUEST_SIZE
                                    } else {
                                        1
                                    };
                                    node.requested_block_num = 0;
                                    break;
                                }
                            }
                            SyncEvent::update_node_state(node, SyncEvent::OnBlockHeadersRes);
                            P2pMgr::update_node(node_hash, node);
                            return;
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
                info!(target: "sync", "peer node removed.");
                return;
            }
        }

        // header_chain.flush();

        if is_side_chain && hashes.len() > 0 {
            BlockBodiesHandler::send_blocks_bodies_req(node, hashes);
        }

        SyncEvent::update_node_state(node, SyncEvent::OnBlockHeadersRes);
        P2pMgr::update_node(node_hash, node);

        if from > 0 {
            BlockHeadersHandler::get_headers(node, from);
        } else {
            if SyncStorage::get_synced_block_number() + 128
                < SyncStorage::get_network_best_block_number()
            {
                if let Some(ref mut peer_node) = P2pMgr::get_an_active_node() {
                    BlockHeadersHandler::get_headers(peer_node, number);
                }
                ::std::thread::sleep(Duration::from_millis(50));
                BlockHeadersHandler::get_headers(node, 0);
            }
        }
    }
}
