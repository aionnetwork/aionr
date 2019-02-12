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
use acore::block::Block;
use acore::client::{BlockId, BlockImportError, BlockStatus};
use acore::error::ImportError;
use acore::header::Seal;
use bytes::BufMut;
use p2p::*;
use rlp::UntrustedRlp;

use super::super::action::SyncAction;
use super::super::event::SyncEvent;
use super::super::storage::{HeadersWrapper, SyncStorage};

pub struct BlockBodiesHandler;

impl BlockBodiesHandler {
    pub fn send_blocks_bodies_req(node_hash: u64, hw: HeadersWrapper) {
        if SyncStorage::insert_headers_with_bodies_requested(hw.clone()) {
            let mut req = ChannelBuffer::new();
            req.head.ver = Version::V0.value();
            req.head.ctrl = Control::SYNC.value();
            req.head.action = SyncAction::BLOCKSBODIESREQ.value();
            for hash in hw.hashes.iter() {
                req.body.put_slice(hash);
            }
            req.head.set_length(req.body.len() as u32);
            trace!(target: "sync", "Sync blocks body req sent...");
            P2pMgr::send(node_hash, req);
        }
    }

    pub fn handle_blocks_bodies_req(node: &mut Node, _req: ChannelBuffer) {
        trace!(target: "sync", "BLOCKSBODIESREQ received.");
        SyncEvent::update_node_state(node, SyncEvent::OnBlockBodiesReq);
    }

    pub fn handle_blocks_bodies_res(node: &mut Node, req: ChannelBuffer) {
        info!(target: "sync", "BLOCKSBODIESRES received from: {}.", node.get_ip_addr());
        if let Some(hw) = SyncStorage::pick_headers_with_bodies_requested(&node.node_hash) {
            let headers = hw.headers;
            if !headers.is_empty() {
                let rlp = UntrustedRlp::new(req.body.as_slice());

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
                        let block = Block {
                            header: headers[i].clone(),
                            transactions: bodies[i].clone(),
                        };
                        blocks.push(block);
                    }
                } else {
                    debug!(
                        target: "sync",
                        "Count mismatch, headers count: {}, bodies count: {}, node id: {}",
                        headers.len(),
                        bodies.len(),
                        node.get_node_id()
                        );
                    blocks.clear();
                }

                if !blocks.is_empty() {
                    let client = SyncStorage::get_block_chain();
                    info!(target: "sync", "chain info: {:?}.", client.chain_info());
                    for block in blocks.iter() {
                        let parent_hash = block.header.parent_hash().clone();
                        let hash = block.header.hash();
                        let number = block.header.number();
                        let mut is_import_directly = false;
                        let result;
                        if let Some(parent_block) = client.block(BlockId::Hash(parent_hash)) {
                            let grant_parent_hash = parent_block.parent_hash();
                            let grant_parent_status =
                                client.block_status(BlockId::Hash(grant_parent_hash));
                            if grant_parent_status == BlockStatus::InChain {
                                is_import_directly = true;
                                info!(target: "sync", "parent block {:?} and grant parent block {:?} inchain, import_block #{} - {:?}.", parent_hash, grant_parent_hash, number, hash);
                            } else {
                                info!(target: "sync", "parent block {:?} inchain BUT grant parent block {:?} NOT inchain, try_import_block #{} - {:?}.", parent_hash, grant_parent_hash, number, hash);
                            }
                        } else {
                            info!(target: "sync", "parent block {:?} NOT inchain, try_import_block #{} - {:?}.", parent_hash, number, hash);
                        }

                        if is_import_directly {
                            //result = client.import_block(block.rlp_bytes(Seal::With));
                            result = client.try_import_block(block.rlp_bytes(Seal::With), node.current_total_difficulty);
                        } else {
                            result = client.try_import_block(block.rlp_bytes(Seal::With), node.current_total_difficulty);
                        }

                        match result {
                            Ok(_)
                            | Err(BlockImportError::Import(ImportError::AlreadyInChain))
                            | Err(BlockImportError::Import(ImportError::AlreadyQueued)) => {
                                client.block_status(BlockId::Hash(block.header.hash()));
                                warn!(target: "sync", "New block #{} - {}, imported.", number, hash);
                            }
                            Err(e) => {
                                warn!(target: "sync", "import_block failed: {:?}", e);
                            }
                        }
                    }
                }
            }
        } else {
            info!(target: "sync", "node entry found: {}.", node.get_ip_addr());
        }
        SyncEvent::update_node_state(node, SyncEvent::OnBlockBodiesRes);
        P2pMgr::update_node(node.node_hash, node);
    }
}
