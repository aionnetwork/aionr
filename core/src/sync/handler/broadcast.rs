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

use std::sync::Arc;
use parking_lot::RwLock;
use std::thread;
use std::time::{Duration,SystemTime};
// use std::sync::RwLock;
use std::collections::{HashMap};
// use lru_cache::LruCache;
use client::{BlockChainClient, BlockId, BlockImportError};
use types::error::{BlockError, ImportError};
use header::Header;
use transaction::UnverifiedTransaction;
use aion_types::H256;
use bytes::BufMut;
use rlp::{RlpStream, UntrustedRlp};
use p2p::{ChannelBuffer,Mgr};
use sync::action::Action;
use sync::node_info::NodeInfo;
use sync::storage::SyncStorage;
use engine::unity_engine::UnityEngine;
use acore_bytes::to_hex;

use super::channel_buffer_template;

const MAX_NEW_BLOCK_AGE: u64 = 20;
// const MAX_RE_BROADCAST: usize = 10;

/// Broadcast new transactions
pub fn broad_new_transactions(p2p: Mgr, storage: Arc<SyncStorage>) {
    // Get transactions waiting in the queue
    let mut transactions = Vec::new();
    let mut size = 0;
    let mut received_transactions = storage.received_transactions().lock();
    while let Some(transaction) = received_transactions.pop_front() {
        transactions.extend_from_slice(&transaction);
        size += 1;
    }

    // End of no transaction to broadcast
    if size < 1 {
        return;
    }

    // Broadcast to active nodes
    let active_nodes = p2p.get_active_nodes();
    if active_nodes.len() > 0 {
        let mut req = channel_buffer_template(Action::BROADCASTTX.value());

        let mut txs_rlp = RlpStream::new_list(size);
        txs_rlp.append_raw(transactions.as_slice(), size);
        req.body.put_slice(txs_rlp.as_raw());

        req.head.len = req.body.len() as u32;

        let mut node_count = 0;
        for node in active_nodes.iter() {
            p2p.send(node.hash, req.clone());
            trace!(target: "sync_broadcast", "Sync broadcast new transactions sent...");
            node_count += 1;
            // TODO: To reconsider why only broadcast to 10 nodes at maximum
            if node_count > 10 {
                break;
            } else {
                thread::sleep(Duration::from_millis(50));
            }
        }
        debug!(target: "sync_broadcast", "Sync broadcasted {} new transactions...", size);
    }
}

/// Broadcast new blocks
pub fn propagate_new_blocks(p2p: Mgr, block_hash: &H256, client: Arc<BlockChainClient>) {
    let active_nodes = p2p.get_active_nodes();
    if active_nodes.len() > 0 {
        let mut req = channel_buffer_template(Action::BROADCASTBLOCK.value());

        if let Some(block_rlp) = client.block(BlockId::Hash(block_hash.clone())) {
            req.body.put_slice(&block_rlp.into_inner());

            req.head.len = req.body.len() as u32;

            for node in active_nodes.iter() {
                p2p.send(node.hash, req.clone());
                trace!(target: "sync_broadcast", "Sync broadcast new block sent...");
            }
        }
    }
}

/// Handle block received from broadcast
pub fn handle_broadcast_block(
    p2p: Mgr,
    node_hash: u64,
    req: ChannelBuffer,
    client: Arc<BlockChainClient>,
    storage: Arc<SyncStorage>,
    network_best_block_number: Arc<RwLock<u64>>,
)
{
    trace!(target: "sync_broadcast", "BROADCASTBLOCK received.");
    let network_best_number = *network_best_block_number.read();
    let best_block_number = client.chain_info().best_block_number;

    if best_block_number + 4 < network_best_number {
        // Ignore BROADCASTBLOCK message until full synced
        trace!(target: "sync_broadcast", "Syncing..., ignore BROADCASTBLOCK message.");
        return;
    }
    drop(network_best_block_number);

    let block_rlp = UntrustedRlp::new(req.body.as_slice());
    if let Ok(header_rlp) = block_rlp.at(0) {
        if let Ok(h) = header_rlp.as_val() {
            let header: Header = h;
            let hash = header.hash();
            let number = header.number();

            // Only accept side chain blocks within MAX_NEW_BLOCK_AGE
            if best_block_number > number && best_block_number - number > MAX_NEW_BLOCK_AGE {
                trace!(target: "sync_broadcast", "Ignored ancient new block {:?}", hash);
                return;
            }

            let parent_hash = header.parent_hash();
            // Proceed only when the parent block is in chain
            if client.block_header(BlockId::Hash(*parent_hash)).is_some() {
                let mut imported_blocks_hashes = storage.imported_blocks_hashes().lock();
                if !imported_blocks_hashes.contains_key(&hash) {
                    // Do basic header validation before proceed
                    let result = UnityEngine::validate_block_header(&header);
                    match result {
                        Ok(()) => {
                            if let Ok(body_rlp) = block_rlp.at(1) {
                                // parse body
                                let mut transactions = Vec::new();
                                if !body_rlp.is_empty() {
                                    for transaction_rlp in body_rlp.iter() {
                                        if !transaction_rlp.is_empty() {
                                            if let Ok(transaction) = transaction_rlp.as_val() {
                                                transactions.push(transaction);
                                            }
                                        }
                                    }
                                }
                                // Do block integrity validation before proceed
                                let result =
                                    UnityEngine::validate_block_body(&header, &transactions);
                                match result {
                                    Ok(_) => {
                                        let result =
                                            client.import_block(block_rlp.as_raw().to_vec());

                                        match result {
                                            Ok(_) => {
                                                trace!(target: "sync_broadcast", "New broadcast block imported {:?} ({})", hash, number);
                                                imported_blocks_hashes.insert(hash, 0);
                                                let active_nodes = p2p.get_active_nodes();
                                                for n in active_nodes.iter() {
                                                    // Re-broadcast this block
                                                    trace!(target: "sync_broadcast", "Sync broadcast new block sent...");
                                                    p2p.send(n.hash, req.clone());
                                                }
                                            }
                                            Err(BlockImportError::Import(
                                                ImportError::AlreadyInChain,
                                            )) => {
                                                trace!(target: "sync_broadcast", "New block already in chain {:?}", hash);
                                            }
                                            Err(BlockImportError::Import(
                                                ImportError::AlreadyQueued,
                                            )) => {
                                                trace!(target: "sync_broadcast", "New block already queued {:?}", hash);
                                            }
                                            Err(BlockImportError::Block(
                                                BlockError::UnknownParent(p),
                                            )) => {
                                                info!(target: "sync_broadcast", "New block with unknown parent ({:?}) {:?}", p, hash);
                                            }
                                            Err(e) => {
                                                error!(target: "sync_broadcast", "Bad new block {:?} : {:?}", hash, e);
                                            }
                                        };
                                    }
                                    Err(_e) => {
                                        debug!(target: "sync_broadcast", "Incomplete block body #{} from node {}.", number, node_hash);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            // ignore this batch if any invalidated header
                            debug!(target: "sync_broadcast", "Invalid header: {:?}, header: {}", e, to_hex(header_rlp.as_raw()));
                        }
                    }
                }
            }

            p2p.update_node(&node_hash);
        }
    }
}

/// Handle transaction received from broadcast
pub fn handle_broadcast_tx(
    p2p: Mgr,
    node_hash: u64,
    req: ChannelBuffer,
    client: Arc<BlockChainClient>,
    node_info: Arc<RwLock<HashMap<u64, RwLock<NodeInfo>>>>,
    storage: Arc<SyncStorage>,
    network_best_block_number: Arc<RwLock<u64>>,
)
{
    trace!(target: "sync_broadcast", "BROADCASTTX received.");

    if let Some(node_info) = node_info.read().get(&node_hash) {
        if node_info
            .read()
            .last_broadcast_timestamp
            .elapsed()
            .expect("should get correct last_broadcast_timestamp ")
            < Duration::from_millis(20)
        {
            // ignore frequent broadcasting
            return;
        }
    } else {
        trace!(target: "sync_broadcast", "Syncing..., ignore BROADCASTTX message.");
        return;
    }

    let network_best_number = *network_best_block_number.read();
    let best_block_number = client.chain_info().best_block_number;

    if best_block_number + 4 < network_best_number {
        // Ignore BROADCASTTX message until full synced
        trace!(target: "sync_broadcast", "Syncing..., ignore BROADCASTTX message.");
        return;
    }
    drop(network_best_block_number);

    let transactions_rlp = UntrustedRlp::new(req.body.as_slice());
    let mut transactions = Vec::new();
    let mut recorded_transaction_hashes = storage.recorded_transaction_hashes().lock();
    for transaction_rlp in transactions_rlp.iter() {
        if !transaction_rlp.is_empty() {
            if let Ok(t) = transaction_rlp.as_val() {
                let tx: UnverifiedTransaction = t;
                let hash = tx.hash().clone();

                if !recorded_transaction_hashes.contains_key(&hash) {
                    transactions.push(tx);
                    recorded_transaction_hashes.insert(hash, 0);
                    storage.insert_received_transaction(transaction_rlp.as_raw().to_vec());
                }
            }
        }
    }
    drop(recorded_transaction_hashes);

    if transactions.len() > 0 {
        client.import_queued_transactions(transactions);
    }
    if let Some(node_info) = node_info.read().get(&node_hash) {
        node_info.write().last_broadcast_timestamp = SystemTime::now();
    }

    p2p.update_node(&node_hash);
}
