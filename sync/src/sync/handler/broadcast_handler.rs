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

use acore::client::{BlockChainClient, BlockId, BlockImportError, BlockStatus};
use acore::error::{BlockError, ImportError};
use acore::header::Header as BlockHeader;
use acore::transaction::UnverifiedTransaction;
use aion_types::H256;
use bytes::BufMut;
use kvdb::DBTransaction;
use rlp::{RlpStream, UntrustedRlp};
use std::thread;
use std::time::{Duration, SystemTime};

use super::super::action::SyncAction;
use super::super::event::SyncEvent;
use super::super::storage::SyncStorage;
use p2p::*;

const MAX_NEW_BLOCK_AGE: u64 = 20;

pub struct BroadcastsHandler;

impl BroadcastsHandler {
    pub fn broad_new_transactions() {
        // broadcast new transactions
        let mut transactions = Vec::new();
        let mut size = 0;
        {
            let mut received_transactions = SyncStorage::get_received_transactions().lock();
            while let Some(transaction) = received_transactions.pop_front() {
                transactions.extend_from_slice(&transaction);
                size += 1;
            }
        }

        if size < 1 {
            return;
        }

        let active_nodes = P2pMgr::get_nodes(ALIVE);

        if active_nodes.len() > 0 {
            let mut req = ChannelBuffer::new();
            req.head.ver = Version::V0.value();
            req.head.ctrl = Control::SYNC.value();
            req.head.action = SyncAction::BROADCASTTX.value();

            let mut txs_rlp = RlpStream::new_list(size);
            txs_rlp.append_raw(transactions.as_slice(), size);
            req.body.put_slice(txs_rlp.as_raw());

            req.head.len = req.body.len() as u32;

            let mut node_count = 0;
            for node in active_nodes.iter() {
                P2pMgr::send(node.node_hash, req.clone());
                trace!(target: "sync", "Sync broadcast new transactions sent...");
                node_count += 1;
                if node_count > 10 {
                    break;
                } else {
                    thread::sleep(Duration::from_millis(50));
                }
            }
            debug!(target: "sync", "Sync broadcasted {} new transactions...", size);
        }
    }

    pub fn propagate_new_blocks(block_hash: &H256) {
        // broadcast new blocks
        let active_nodes = P2pMgr::get_nodes(ALIVE);

        if active_nodes.len() > 0 {
            let mut req = ChannelBuffer::new();
            req.head.ver = Version::V0.value();
            req.head.ctrl = Control::SYNC.value();
            req.head.action = SyncAction::BROADCASTBLOCK.value();

            let header_chain = SyncStorage::get_block_header_chain();
            let block_chain = SyncStorage::get_block_chain();
            if let Some(block) = block_chain.block(BlockId::Hash(*block_hash)) {
                let header = block.header();
                let parent_hash = header.parent_hash();
                if header_chain.status(&parent_hash) == BlockStatus::InChain {
                    if header_chain.status(block_hash) != BlockStatus::InChain {
                        if let Some(total_difficulty) =
                            block_chain.block_total_difficulty(BlockId::Hash(parent_hash))
                        {
                            let mut tx = DBTransaction::new();
                            if let Ok(num) = header_chain.insert_with_td(
                                tx,
                                &header,
                                Some(total_difficulty + header.difficulty()),
                                None,
                                false,
                            ) {
                                header_chain.flush();
                                debug!(target: "sync", "New block header #{} - {}, imported from local.", num, block_hash);
                            }
                        }
                    }
                }
                trace!(target: "sync",
                            "New block #{} {}, with {} txs added in chain from local.",
                            header.number(), block_hash, block.transactions_count());
                req.body.put_slice(&block.into_inner());

                req.head.len = req.body.len() as u32;

                for node in active_nodes.iter() {
                    P2pMgr::send(node.node_hash, req.clone());
                    trace!(target: "sync", "Sync broadcast new block sent...");
                }
            }
        }
    }

    pub fn handle_broadcast_block(node: &mut Node, req: ChannelBuffer) {
        trace!(target: "sync", "BROADCASTBLOCK received.");

        if SyncStorage::get_synced_block_number() + 4 < SyncStorage::get_network_best_block_number()
        {
            // Ignore BROADCASTBLOCK message until full synced
            trace!(target: "sync", "Syncing..., ignore BROADCASTBLOCK message.");
            return;
        }

        let block_rlp = UntrustedRlp::new(req.body.as_slice());
        if let Ok(header_rlp) = block_rlp.at(0) {
            if let Ok(h) = header_rlp.as_val() {
                let header: BlockHeader = h;
                let last_imported_number = SyncStorage::get_synced_block_number();
                let hash = header.hash();
                let number = header.number();

                if last_imported_number > header.number()
                    && last_imported_number > MAX_NEW_BLOCK_AGE + header.number()
                {
                    trace!(target: "sync", "Ignored ancient new block {:?}", header.hash());
                    return;
                }

                let parent_hash = header.parent_hash();
                let header_chain = SyncStorage::get_block_header_chain();
                let block_chain = SyncStorage::get_block_chain();
                if header_chain.status(&parent_hash) == BlockStatus::InChain {
                    if header_chain.status(&hash) != BlockStatus::InChain {
                        if let Some(total_difficulty) =
                            block_chain.block_total_difficulty(BlockId::Hash(*parent_hash))
                        {
                            let mut tx = DBTransaction::new();
                            if let Ok(num) = header_chain.insert_with_td(
                                tx,
                                &header.encoded(),
                                Some(total_difficulty + *header.difficulty()),
                                None,
                                false,
                            ) {
                                header_chain.flush();
                                debug!(target: "sync", "New block header #{} - {}, imported from {}@{}.", num, hash, node.get_ip_addr(), node.get_node_id());
                            }
                        }
                    }
                }

                match block_chain.block_header(BlockId::Hash(*parent_hash)) {
                    Some(_) => {
                        SyncStorage::insert_requested_time(hash);
                        let result = block_chain.import_block(block_rlp.as_raw().to_vec());

                        match result {
                            Ok(_) => {
                                trace!(target: "sync", "New broadcast block imported {:?} ({})", hash, number);
                                if node.target_total_difficulty
                                    >= SyncStorage::get_network_total_diff()
                                {
                                    SyncStorage::set_synced_block_number(number);
                                }

                                let active_nodes = P2pMgr::get_nodes(ALIVE);
                                for n in active_nodes.iter() {
                                    // broadcast new block
                                    trace!(target: "sync", "Sync broadcast new block sent...");
                                    P2pMgr::send(n.node_hash, req.clone());
                                }
                                node.inc_reputation(10);
                                P2pMgr::update_node(node.node_hash, node);
                            }
                            Err(BlockImportError::Import(ImportError::AlreadyInChain)) => {
                                trace!(target: "sync", "New block already in chain {:?}", hash);
                                node.inc_reputation(1);
                                P2pMgr::update_node(node.node_hash, node);
                            }
                            Err(BlockImportError::Import(ImportError::AlreadyQueued)) => {
                                trace!(target: "sync", "New block already queued {:?}", hash);
                                node.inc_reputation(1);
                                P2pMgr::update_node(node.node_hash, node);
                            }
                            Err(BlockImportError::Block(BlockError::UnknownParent(p))) => {
                                info!(target: "sync", "New block with unknown parent ({:?}) {:?}", p, hash);
                                node.dec_reputation(10);
                                P2pMgr::update_node(node.node_hash, node);
                            }
                            Err(e) => {
                                error!(target: "sync", "Bad new block {:?} : {:?}", hash, e);
                                node.dec_reputation(50);
                                P2pMgr::update_node(node.node_hash, node);
                            }
                        };
                    }
                    None => {}
                };
                SyncEvent::update_node_state(node, SyncEvent::OnBroadCastBlock);
            }
        }
    }

    pub fn handle_broadcast_tx(node: &mut Node, req: ChannelBuffer) {
        trace!(target: "sync", "BROADCASTTX received.");

        if node.last_broadcast_timestamp + Duration::from_millis(20) > SystemTime::now() {
            // ignore frequent broadcasting
            return;
        }

        if SyncStorage::get_synced_block_number() + 4 < SyncStorage::get_network_best_block_number()
        {
            // Ignore BROADCASTTX message until full synced
            trace!(target: "sync", "Syncing..., ignore BROADCASTTX message.");
            return;
        }

        let transactions_rlp = UntrustedRlp::new(req.body.as_slice());
        let mut transactions = Vec::new();
        {
            let mut transaction_hashes = SyncStorage::get_sent_transaction_hashes().lock();
            for transaction_rlp in transactions_rlp.iter() {
                if !transaction_rlp.is_empty() {
                    if let Ok(t) = transaction_rlp.as_val() {
                        let tx: UnverifiedTransaction = t;
                        let hash = tx.hash();

                        if !transaction_hashes.contains_key(&hash) {
                            transactions.push(tx);
                            transaction_hashes.insert(hash, 0);
                            SyncStorage::insert_received_transaction(
                                transaction_rlp.as_raw().to_vec(),
                            );
                        }
                    }
                }
            }
        }

        if transactions.len() > 0 {
            let client = SyncStorage::get_block_chain();
            client.import_queued_transactions(transactions);
            node.inc_reputation(1);
            P2pMgr::update_node(node.node_hash, node);
        }
        node.last_broadcast_timestamp = SystemTime::now();

        SyncEvent::update_node_state(node, SyncEvent::OnBroadCastTx);
        P2pMgr::update_node(node.node_hash, node);
    }
}
