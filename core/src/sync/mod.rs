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

mod event;
mod handler;
mod action;
mod storage;

use client::{BlockChainClient, BlockId, BlockStatus, ChainNotify};
use transaction::UnverifiedTransaction;
use aion_types::H256;
use futures::{Future, Stream};
use rlp::UntrustedRlp;
use std::collections::BTreeMap;
use std::ops::Index;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::runtime::TaskExecutor;
use tokio::timer::Interval;
use p2p::handlers::DefaultHandler;
use p2p::*;

use self::action::SyncAction;
use self::handler::blocks_bodies_handler::BlockBodiesHandler;
use self::handler::blocks_headers_handler::BlockHeadersHandler;
use self::handler::broadcast_handler::BroadcastsHandler;
use self::handler::import_handler::ImportHandler;
use self::handler::status_handler::StatusHandler;
use self::storage::{ActivePeerInfo, PeerInfo, SyncState, SyncStatus, SyncStorage, TransactionStats};
use rustc_hex::ToHex;

const STATUS_REQ_INTERVAL: u64 = 2;
const BLOCKS_BODIES_REQ_INTERVAL: u64 = 50;
const BLOCKS_IMPORT_INTERVAL: u64 = 50;
const STATICS_INTERVAL: u64 = 15;
const BROADCAST_TRANSACTIONS_INTERVAL: u64 = 50;

#[derive(Clone)]
struct SyncMgr;

impl SyncMgr {
    fn enable(executor: &TaskExecutor, max_peers: u32) {
        let status_req_task =
            Interval::new(Instant::now(), Duration::from_secs(STATUS_REQ_INTERVAL))
                .for_each(move |_| {
                    // status req
                    StatusHandler::send_status_req();

                    Ok(())
                })
                .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(status_req_task);

        let blocks_bodies_req_task = Interval::new(
            Instant::now(),
            Duration::from_millis(BLOCKS_BODIES_REQ_INTERVAL),
        )
        .for_each(move |_| {
            // blocks bodies req
            BlockBodiesHandler::send_blocks_bodies_req();

            Ok(())
        })
        .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(blocks_bodies_req_task);

        let blocks_import_task = Interval::new(
            Instant::now(),
            Duration::from_millis(BLOCKS_IMPORT_INTERVAL),
        )
        .for_each(move |_| {
            ImportHandler::import_blocks();

            Ok(())
        })
        .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(blocks_import_task);

        let broadcast_transactions_task = Interval::new(
            Instant::now(),
            Duration::from_millis(BROADCAST_TRANSACTIONS_INTERVAL),
        )
        .for_each(move |_| {
            BroadcastsHandler::broad_new_transactions();

            Ok(())
        })
        .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(broadcast_transactions_task);

        let statics_task = Interval::new(Instant::now(), Duration::from_secs(STATICS_INTERVAL))
            .for_each(move |_| {
                let connected_nodes = P2pMgr::get_nodes(CONNECTED);
                for node in connected_nodes.iter() {
                    if node.mode == Mode::BACKWARD || node.mode == Mode::FORWARD {
                        if node.target_total_difficulty < SyncStorage::get_network_total_diff() {
                            P2pMgr::remove_peer(node.node_hash);
                        }
                    } else if node.last_request_timestamp
                        + Duration::from_secs(STATICS_INTERVAL * 12)
                        < SystemTime::now()
                    {
                        info!(target: "sync", "Disconnect with idle node: {}@{}.", node.get_node_id(), node.get_ip_addr());
                        P2pMgr::remove_peer(node.node_hash);
                    }
                }

                let chain_info = SyncStorage::get_chain_info();
                let block_number_last_time = SyncStorage::get_synced_block_number_last_time();
                let block_number_now = chain_info.best_block_number;
                let sync_speed = (block_number_now - block_number_last_time) / STATICS_INTERVAL;
                let mut active_nodes = P2pMgr::get_nodes(ALIVE);
                let active_nodes_count = active_nodes.len();

                info!(target: "sync", "{:=^127}", " Sync Statics ");
                info!(target: "sync", "Best block number: {}, hash: {}", chain_info.best_block_number, chain_info.best_block_hash);
                info!(target: "sync", "Network Best block number: {}, hash: {}", SyncStorage::get_network_best_block_number(), SyncStorage::get_network_best_block_hash());
                info!(target: "sync", "Max staged block number: {}", SyncStorage::get_max_staged_block_number());
                info!(target: "sync", "Sync speed: {} blks/sec", sync_speed);
                info!(target: "sync",
                    "Total/Connected/Active peers: {}/{}/{}",
                    P2pMgr::get_all_nodes_count(),
                    P2pMgr::get_nodes_count(CONNECTED),
                    active_nodes_count,
                );
                info!(target: "sync", "{:-^127}","");
                info!(target: "sync","      Total Diff    Blk No.    Blk Hash                 Address                 Revision      Conn  Seed  LstReq No.       Mode");
                info!(target: "sync", "{:-^127}","");
                active_nodes.sort_by(|a, b| {
                    if a.target_total_difficulty != b.target_total_difficulty {
                        b.target_total_difficulty.cmp(&a.target_total_difficulty)
                    } else {
                        b.best_block_num.cmp(&a.best_block_num)
                    }
                });
                let mut count: u32 = 0;
                for node in active_nodes.iter() {
                    if let Ok(_) = node.last_request_timestamp.elapsed() {
                        info!(target: "sync",
                            "{:>16}{:>11}{:>12}{:>24}{:>25}{:>10}{:>6}{:>12}{:>11}",
                            format!("{}",node.target_total_difficulty),
                            node.best_block_num,
                            format!("{}",node.best_hash),
                            node.get_display_ip_addr(),
                            String::from_utf8_lossy(&node.revision).trim(),
                            match node.ip_addr.is_server{
                                true => "Outbound",
                                _=>"Inbound"
                            },
                            match node.is_from_boot_list{
                                true => "Y",
                                _ => ""
                            },
                            node.last_request_num,
                            format!("{}",node.mode)
                        );
                        count += 1;
                        if count ==  max_peers {
                            break;
                        }
                    }
                }
                info!(target: "sync", "{:-^127}","");

                if block_number_now + 8 < SyncStorage::get_network_best_block_number()
                    && block_number_now - block_number_last_time < 2
                {
                    SyncStorage::get_block_chain().clear_queue();
                    SyncStorage::get_block_chain().clear_bad();
                    SyncStorage::clear_downloaded_headers();
                    SyncStorage::clear_downloaded_blocks();
                    SyncStorage::clear_downloaded_block_hashes();
                    SyncStorage::clear_requested_blocks();
                    SyncStorage::clear_headers_with_bodies_requested();
                    SyncStorage::set_synced_block_number(
                        SyncStorage::get_chain_info().best_block_number,
                    );
                    let abnormal_mode_nodes_count =
                        P2pMgr::get_nodes_count_with_mode(Mode::BACKWARD)
                            + P2pMgr::get_nodes_count_with_mode(Mode::FORWARD);
                    if abnormal_mode_nodes_count > (active_nodes_count / 5)
                        || active_nodes_count == 0
                    {
                        info!(target: "sync", "Abnormal status, reseting network...");
                        P2pMgr::reset();

                        SyncStorage::clear_imported_block_hashes();
                        SyncStorage::clear_staged_blocks();
                        SyncStorage::set_max_staged_block_number(0);
                    }
                }

                SyncStorage::set_synced_block_number_last_time(block_number_now);
                SyncStorage::set_sync_speed(sync_speed as u16);

                if SyncStorage::get_network_best_block_number()
                    <= SyncStorage::get_synced_block_number()
                {
                    // full synced
                    SyncStorage::clear_staged_blocks();
                }

                Ok(())
            })
            .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(statics_task);
    }

    fn handle(node: &mut Node, req: ChannelBuffer) {
        if node.state_code & HANDSHAKE_DONE != HANDSHAKE_DONE {
            return;
        }

        match Version::from(req.head.ver) {
            Version::V0 => {
                trace!(target: "sync", "Ver 0 package received.");

                match Control::from(req.head.ctrl) {
                    Control::NET => {}
                    Control::SYNC => {
                        trace!(target: "sync", "P2P message received.");

                        match SyncAction::from(req.head.action) {
                            SyncAction::STATUSREQ => {
                                StatusHandler::handle_status_req(node);
                            }
                            SyncAction::STATUSRES => {
                                StatusHandler::handle_status_res(node, req);
                            }
                            SyncAction::BLOCKSHEADERSREQ => {
                                BlockHeadersHandler::handle_blocks_headers_req(node, req);
                            }
                            SyncAction::BLOCKSHEADERSRES => {
                                BlockHeadersHandler::handle_blocks_headers_res(node, req);
                            }
                            SyncAction::BLOCKSBODIESREQ => {
                                BlockBodiesHandler::handle_blocks_bodies_req(node, req);
                            }
                            SyncAction::BLOCKSBODIESRES => {
                                BlockBodiesHandler::handle_blocks_bodies_res(node, req);
                            }
                            SyncAction::BROADCASTTX => {
                                BroadcastsHandler::handle_broadcast_tx(node, req);
                            }
                            SyncAction::BROADCASTBLOCK => {
                                BroadcastsHandler::handle_broadcast_block(node, req);
                            }
                            _ => {
                                trace!(target: "sync", "UNKNOWN received.");
                            }
                        }
                    }
                    _ => {
                        error!(target: "sync", "Invalid message received: {}", req.head);
                    }
                }
            }
            Version::V1 => {
                trace!(target: "sync", "Ver 1 package received.");
            }
            _ => {
                error!(target: "sync", "Invalid Version.");
            }
        };
    }

    fn disable() { SyncStorage::reset(); }
}

pub struct NetworkService {
    pub config: NetworkConfig,
}

/// Sync
pub struct Sync {
    /// Network service
    network: NetworkService,
    /// starting block number.
    starting_block_number: u64,
}

impl Sync {
    pub fn new(client: Arc<BlockChainClient>, config: NetworkConfig) -> Arc<Sync> {
        let chain_info = client.chain_info();
        // starting block number is the local best block number during kernel startup.
        let starting_block_number = chain_info.best_block_number;

        SyncStorage::init(client);

        let service = NetworkService {
            config: config.clone(),
        };
        Arc::new(Sync {
            network: service,
            starting_block_number,
        })
    }
}

pub trait SyncProvider: Send + ::std::marker::Sync {
    /// Get sync status
    fn status(&self) -> SyncStatus;

    /// Get peers information
    fn peers(&self) -> Vec<PeerInfo>;

    /// Get the enode if available.
    fn enode(&self) -> Option<String>;

    /// Returns propagation count for pending transactions.
    fn transactions_stats(&self) -> BTreeMap<H256, TransactionStats>;

    /// Get active nodes
    fn active(&self) -> Vec<ActivePeerInfo>;
}

impl SyncProvider for Sync {
    /// Get sync status
    fn status(&self) -> SyncStatus {
        // TODO:  only set start_block_number/highest_block_number.
        SyncStatus {
            state: SyncState::Idle,
            protocol_version: 0,
            network_id: 256,
            start_block_number: self.starting_block_number,
            last_imported_block_number: None,
            highest_block_number: { Some(SyncStorage::get_network_best_block_number()) },
            blocks_received: 0,
            blocks_total: 0,
            num_peers: { P2pMgr::get_nodes_count(ALIVE) },
            num_active_peers: 0,
        }
    }

    /// Get sync peers
    fn peers(&self) -> Vec<PeerInfo> {
        let mut peer_info_list = Vec::new();
        let peer_nodes = P2pMgr::get_all_nodes();
        for peer in peer_nodes.iter() {
            let peer_info = PeerInfo {
                id: Some(peer.get_node_id()),
            };
            peer_info_list.push(peer_info);
        }
        peer_info_list
    }

    fn enode(&self) -> Option<String> { Some(P2pMgr::get_local_node().get_node_id()) }

    fn transactions_stats(&self) -> BTreeMap<H256, TransactionStats> { BTreeMap::new() }

    fn active(&self) -> Vec<ActivePeerInfo> {
        let ac_nodes = P2pMgr::get_nodes(ALIVE);
        ac_nodes
            .into_iter()
            .map(|node| {
                ActivePeerInfo {
                    highest_block_number: node.best_block_num,
                    id: node.node_id.to_hex(),
                    ip: node.ip_addr.ip.to_hex(),
                }
            })
            .collect()
    }
}

/// Trait for managing network
pub trait NetworkManager: Send + ::std::marker::Sync {
    /// Set to allow unreserved peers to connect
    fn accept_unreserved_peers(&self);
    /// Set to deny unreserved peers to connect
    fn deny_unreserved_peers(&self);
    /// Start network
    fn start_network(&self);
    /// Stop network
    fn stop_network(&self);
    /// Query the current configuration of the network
    fn network_config(&self) -> NetworkConfig;
}

impl NetworkManager for Sync {
    fn accept_unreserved_peers(&self) {}

    fn deny_unreserved_peers(&self) {}

    fn start_network(&self) {
        let executor = SyncStorage::get_executor();
        let sync_handler = DefaultHandler {
            callback: SyncMgr::handle,
        };

        P2pMgr::enable(self.network_config());
        debug!(target: "sync", "###### P2P enabled... ######");

        NetManager::enable(&executor, sync_handler);
        debug!(target: "sync", "###### network enabled... ######");

        SyncMgr::enable(&executor, self.network.config.max_peers);
        debug!(target: "sync", "###### SYNC enabled... ######");
    }

    fn stop_network(&self) {
        SyncMgr::disable();
        P2pMgr::disable();
    }

    fn network_config(&self) -> NetworkConfig { NetworkConfig::from(self.network.config.clone()) }
}

impl ChainNotify for Sync {
    fn new_blocks(
        &self,
        imported: Vec<H256>,
        _invalid: Vec<H256>,
        enacted: Vec<H256>,
        _retracted: Vec<H256>,
        sealed: Vec<H256>,
        _proposed: Vec<Vec<u8>>,
        _duration: u64,
    )
    {
        if P2pMgr::get_all_nodes_count() == 0 {
            return;
        }

        if !imported.is_empty() {
            let min_imported_block_number = SyncStorage::get_synced_block_number() + 1;
            let mut max_imported_block_number = 0;
            let client = SyncStorage::get_block_chain();
            for hash in imported.iter() {
                // ImportHandler::import_staged_blocks(&hash);
                let block_id = BlockId::Hash(*hash);
                if client.block_status(block_id) == BlockStatus::InChain {
                    if let Some(block_number) = client.block_number(block_id) {
                        if max_imported_block_number < block_number {
                            max_imported_block_number = block_number;
                        }
                    }
                }
            }

            // The imported blocks are not new or not yet in chain. Do not notify in this case.
            if max_imported_block_number < min_imported_block_number {
                return;
            }

            let synced_block_number = SyncStorage::get_synced_block_number();
            if max_imported_block_number <= synced_block_number {
                let mut hashes = Vec::new();
                for block_number in max_imported_block_number..synced_block_number + 1 {
                    let block_id = BlockId::Number(block_number);
                    if let Some(block_hash) = client.block_hash(block_id) {
                        hashes.push(block_hash);
                    }
                }
                if hashes.len() > 0 {
                    SyncStorage::remove_imported_block_hashes(hashes);
                }
            }

            SyncStorage::set_synced_block_number(max_imported_block_number);

            for block_number in min_imported_block_number..max_imported_block_number + 1 {
                let block_id = BlockId::Number(block_number);
                if let Some(blk) = client.block(block_id) {
                    let block_hash = blk.hash();
                    ImportHandler::import_staged_blocks(&block_hash);
                    if let Some(time) = SyncStorage::get_requested_time(&block_hash) {
                        info!(target: "sync",
                            "New block #{} {}, with {} txs added in chain, time elapsed: {:?}.",
                            block_number, block_hash, blk.transactions_count(), SystemTime::now().duration_since(time).expect("importing duration"));
                    }
                }
            }
        }

        if enacted.is_empty() {
            for hash in enacted.iter() {
                debug!(target: "sync", "enacted hash: {:?}", hash);
                ImportHandler::import_staged_blocks(&hash);
            }
        }

        if !sealed.is_empty() {
            debug!(target: "sync", "Propagating blocks...");
            SyncStorage::insert_imported_block_hashes(sealed.clone());
            BroadcastsHandler::propagate_new_blocks(
                sealed.index(0),
                SyncStorage::get_block_chain(),
            );
        }
    }

    fn start(&self) {
        info!(target: "sync", "starting...");
    }

    fn stop(&self) {
        info!(target: "sync", "stopping...");
    }

    fn broadcast(&self, _message: Vec<u8>) {}

    fn transactions_received(&self, transactions: &[Vec<u8>]) {
        if transactions.len() == 1 {
            let transaction_rlp = transactions[0].clone();
            if let Ok(tx) = UntrustedRlp::new(&transaction_rlp).as_val() {
                let transaction: UnverifiedTransaction = tx;
                let hash = transaction.hash();
                let sent_transaction_hashes_mutex = SyncStorage::get_sent_transaction_hashes();
                let mut lock = sent_transaction_hashes_mutex.lock();

                if let Ok(ref mut sent_transaction_hashes) = lock {
                    if !sent_transaction_hashes.contains_key(hash) {
                        sent_transaction_hashes.insert(hash.clone(), 0);
                        SyncStorage::insert_received_transaction(transaction_rlp);
                    }
                }
            }
        }
    }
}

/// Configuration for IPC service.
#[derive(Debug, Clone)]
pub struct ServiceConfiguration {
    /// Network configuration.
    pub net: NetworkConfig,
}
