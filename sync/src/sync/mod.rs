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
use acore::client::{
    cht, header_chain::HeaderChain, BlockChainClient, BlockId, BlockStatus, ChainNotify, Client,
};
use acore::transaction::UnverifiedTransaction;
use aion_types::H256;
use futures::future::{loop_fn, Loop};
use futures::{Future, Stream};
use kvdb::DBTransaction;
use rlp::UntrustedRlp;
use std::collections::BTreeMap;
use std::ops::Index;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use tokio::timer::Interval;

use net::handler::default_handler::DefaultHandler;
use net::NetManager;
use p2p::*;

use net::event::HANDSHAKE_DONE;

use self::action::SyncAction;
// use self::error::*;
use self::handler::blocks_bodies_handler::BlockBodiesHandler;
use self::handler::blocks_headers_handler::BlockHeadersHandler;
use self::handler::broadcast_handler::BroadcastsHandler;
use self::handler::status_handler::StatusHandler;
use self::storage::{
    ActivePeerInfo, PeerInfo, SyncState, SyncStatus, SyncStorage, TransactionStats,
};
use light::LightSyncManager;
use rustc_hex::ToHex;

pub mod action;
pub mod error;
mod event;
mod handler;
pub mod storage;

const STATUS_REQ_INTERVAL: u64 = 2;
const GET_BLOCK_HEADERS_INTERVAL: u64 = 20;
const BLOCKS_BODIES_REQ_INTERVAL: u64 = 20;
const STATICS_INTERVAL: u64 = 15;
const BROADCAST_TRANSACTIONS_INTERVAL: u64 = 50;
const REPUTATION_HANDLE_INTERVAL: u64 = 1800;
const SYNC_STATIC_CAPACITY: usize = 25;
const CLEAR_BLACK_LIST_INTERVAL: u64 = 3600;

#[derive(Clone)]
struct SyncMgr;

impl SyncMgr {
    fn enable() {
        let executor = SyncStorage::get_sync_executor();

        let status_req_task =
            Interval::new(Instant::now(), Duration::from_secs(STATUS_REQ_INTERVAL))
                .for_each(move |_| {
                    // status req
                    StatusHandler::send_status_req();

                    Ok(())
                })
                .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(status_req_task);

        let get_block_headers_task = Interval::new(
            Instant::now(),
            Duration::from_secs(GET_BLOCK_HEADERS_INTERVAL),
        )
        .for_each(move |_| {
            if let Some(ref mut node) = P2pMgr::get_an_alive_node() {
                BlockHeadersHandler::get_headers(node, 0);
            }

            Ok(())
        })
        .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(get_block_headers_task);

        let blocks_bodies_req_task = Interval::new(
            Instant::now(),
            Duration::from_secs(BLOCKS_BODIES_REQ_INTERVAL),
        )
        .for_each(move |_| {
            // blocks bodies req
            if let Some(ref mut node) = P2pMgr::get_an_alive_node() {
                BlockBodiesHandler::get_blocks_bodies(node, 0);
            }
            Ok(())
        })
        .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(blocks_bodies_req_task);

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

        let flush_task = loop_fn(0, |block_number| {
            let block_chain = SyncStorage::get_block_chain();
            block_chain.import_verified_blocks();

            let synced_block_number = block_chain.chain_info().best_block_number;

            if block_number == synced_block_number {
                if let Some(ref mut node) = P2pMgr::get_an_alive_node() {
                    BlockBodiesHandler::get_blocks_bodies(node, 0);
                }
                thread::sleep(Duration::from_millis(200));
            }
            thread::sleep(Duration::from_millis(300));
            if SyncStorage::is_syncing() {
                Ok(Loop::Continue(synced_block_number))
            } else {
                Ok(Loop::Break(()))
            }
        });
        executor.spawn(flush_task);

        let clear_task = Interval::new(
            Instant::now(),
            Duration::from_secs(CLEAR_BLACK_LIST_INTERVAL),
        )
        .for_each(move |_| {
            P2pMgr::clear_black_list();
            Ok(())
        })
        .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(clear_task);

        let reputation_handle_task = Interval::new(
            Instant::now(),
            Duration::from_secs(REPUTATION_HANDLE_INTERVAL),
        )
        .for_each(move |_| {
            let mut active_nodes = P2pMgr::get_nodes(ALIVE);
            active_nodes.sort_by(|a, b| {
                if a.reputation != b.reputation {
                    b.reputation.cmp(&a.reputation)
                } else {
                    b.best_block_num.cmp(&a.best_block_num)
                }
            });

            let mut top8_nodes: Vec<_> = active_nodes
                .iter()
                .map(|ref node| node.node_hash)
                .collect::<Vec<_>>();
            if top8_nodes.len() > 8 {
                top8_nodes.split_off(8);
                P2pMgr::replace_top8_node_hashes(top8_nodes);
            } else {
                P2pMgr::refresh_top8_node_hashes(top8_nodes);
            }
            SyncStorage::clear_headers_with_bodies_requested();
            P2pMgr::reset_reputation();
            Ok(())
        })
        .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(reputation_handle_task);

        let statics_task = Interval::new(Instant::now(), Duration::from_secs(STATICS_INTERVAL))
            .for_each(move |_| {
                let connected_nodes = P2pMgr::get_nodes(CONNECTED);
                for node in connected_nodes.iter() {
                    if node.last_request_timestamp + Duration::from_secs(STATICS_INTERVAL * 10)
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
                info!(target: "sync", "Best block number: {}, hash: {}, total difficulty: {}", chain_info.best_block_number, chain_info.best_block_hash, chain_info.total_difficulty);
                info!(target: "sync", "Network Best block number: {}, hash: {}", SyncStorage::get_network_best_block_number(), SyncStorage::get_network_best_block_hash());
                info!(target: "sync", "Max staged block number: {}", SyncStorage::get_block_header_chain().chain_info().best_block_number);
                info!(target: "sync", "Sync speed: {} blks/sec", sync_speed);
                info!(target: "sync",
                    "Total/Connected/Active peers: {}/{}/{}",
                    P2pMgr::get_all_nodes_count(),
                    P2pMgr::get_nodes_count(CONNECTED),
                    active_nodes_count,
                );
                info!(target: "sync", "{:-^127}","");
                info!(target: "sync","      Total Diff    Blk No.    Blk Hash                 Address                 Revision      Conn  Seed");
                info!(target: "sync", "{:-^127}","");
                active_nodes.sort_by(|a, b| {
                    if a.target_total_difficulty != b.target_total_difficulty {
                        b.target_total_difficulty.cmp(&a.target_total_difficulty)
                    } else {
                        b.best_block_num.cmp(&a.best_block_num)
                    }
                });
                let mut count = 0;
                for node in active_nodes.iter() {
                    if let Ok(_) = node.last_request_timestamp.elapsed() {
                        info!(target: "sync",
                            "{:>16}{:>11}{:>12}{:>24}{:>25}{:>10}{:>6}",
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
                            }
                        );
                        count += 1;
                        if count == SYNC_STATIC_CAPACITY {
                            break;
                        }
                    }
                }
                info!(target: "sync", "{:-^127}","");

                if SyncStorage::get_network_best_block_number() > 0
                    && block_number_now + 8 < SyncStorage::get_network_best_block_number()
                    && block_number_now <= block_number_last_time
                {
                    {
                        let block_chain = SyncStorage::get_block_chain();
                        block_chain.clear_queue(false);
                    }
                    SyncStorage::clear_headers_with_bodies_requested();
                }

                SyncStorage::set_synced_block_number_last_time(block_number_now);
                SyncStorage::set_sync_speed(sync_speed as u16);

                Ok(())
            })
            .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(statics_task);
    }

    fn handle(node: &mut Node, req: ChannelBuffer) {
        if node.state_code & HANDSHAKE_DONE != HANDSHAKE_DONE || !SyncStorage::is_syncing() {
            return;
        }

        match Version::from(req.head.ver) {
            Version::V0 => {
                trace!(target: "sync", "Ver 0 package received.");

                match Control::from(req.head.ctrl) {
                    Control::NET | Control::LIGHT => {
                        error!(target: "sync", "unreachable control!");
                    }
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

    fn disable() {
        SyncStorage::set_is_syncing(false);
        let block_chain = SyncStorage::get_block_chain();
        block_chain.clear_queue(true);
        SyncStorage::reset();
    }

    fn build_header_chain(to: u64) {
        let header_chain = SyncStorage::get_block_header_chain();
        if to == 0 {
            return;
        }

        let cht_number =
            cht::block_to_cht_number(to).expect(&format!("Invalid block number #{} !", to));
        let from = cht::start_number(cht_number);

        let block_chain = SyncStorage::get_block_chain();
        for number in from..to + 1 {
            if let Some(ref header) = block_chain.block_header(BlockId::Number(number)) {
                let block_total_difficulty =
                    block_chain.block_total_difficulty(BlockId::Number(number));
                let hash = header.hash();
                if header_chain.status(&hash) != BlockStatus::InChain {
                    let mut tx = DBTransaction::new();
                    if let Ok(num) =
                        header_chain.insert_with_td(tx, header, block_total_difficulty, None, false)
                    {
                        debug!(target: "sync", "New block header {} imported.", num);
                    }
                }
            }
        }
    }
}

/// Sync configuration
#[derive(Debug, Clone, Copy)]
pub struct SyncConfig {
    /// Max blocks to download ahead
    pub max_download_ahead_blocks: usize,
}

impl Default for SyncConfig {
    fn default() -> SyncConfig {
        SyncConfig {
            max_download_ahead_blocks: 20000,
        }
    }
}

/// Sync initialization parameters.
pub struct Params {
    /// Configuration.
    pub config: SyncConfig,
    /// Blockchain client.
    pub client: Arc<Client>,
    /// Network layer configuration.
    pub network_config: NetworkConfig,
    /// Header Chain
    pub header_chain: Arc<HeaderChain>,
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
    /// Create handler with the network service
    pub fn get_instance(params: Params) -> Arc<Sync> {
        let chain_info = params.client.chain_info();

        SyncStorage::init(params.client, params.header_chain);

        SyncStorage::set_is_syncing(true);
        // starting block number is the local best block number during kernel startup.
        let starting_block_number = chain_info.best_block_number;

        SyncStorage::set_synced_block_number(starting_block_number);
        SyncStorage::set_synced_block_number_last_time(starting_block_number);
        let best_block_number = SyncStorage::get_synced_block_number();

        let best_header_number = SyncStorage::get_block_header_chain().best_block().number;
        if best_header_number < best_block_number {
            SyncMgr::build_header_chain(best_block_number);
        }

        let service = NetworkService {
            config: params.network_config.clone(),
        };
        Arc::new(Sync {
            network: service,
            starting_block_number: starting_block_number,
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
        let sync_handler = DefaultHandler {
            callback: SyncMgr::handle,
        };
        let light_handler = DefaultHandler {
            callback: LightSyncManager::handle,
        };
        P2pMgr::enable(self.network_config());
        debug!(target: "sync", "###### P2P enabled... ######");

        NetManager::enable(sync_handler, light_handler);
        debug!(target: "sync", "###### network enabled... ######");

        SyncMgr::enable();
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
        _enacted: Vec<H256>,
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
            let client = SyncStorage::get_block_chain();
            let min_imported_block_number = SyncStorage::get_synced_block_number() + 1;
            let mut max_imported_block_number = 0;
            for hash in imported.iter() {
                let block_id = BlockId::Hash(*hash);
                let block_status = client.block_status(block_id);
                if block_status == BlockStatus::InChain || block_status == BlockStatus::Queued {
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

            SyncStorage::set_synced_block_number(max_imported_block_number);

            let total_difficulty = client.chain_info().total_difficulty;
            SyncStorage::set_total_difficulty(total_difficulty);

            for block_number in min_imported_block_number..max_imported_block_number + 1 {
                let block_id = BlockId::Number(block_number);
                if let Some(blk) = client.block(block_id) {
                    let block_hash = blk.hash();
                    if let Some(time) = SyncStorage::get_requested_time(&block_hash) {
                        info!(target: "sync",
                            "New block #{} {}, with {} txs added in chain, time elapsed: {:?}.",
                            block_number, block_hash, blk.transactions_count(), SystemTime::now().duration_since(time).expect("importing duration"));
                    }
                }
            }
        }

        if !sealed.is_empty() {
            debug!(target: "sync", "Propagating blocks...");
            if SyncStorage::get_synced_block_number() + 4
                < SyncStorage::get_network_best_block_number()
            {
                // Ignore Propagated blocks
                trace!(target: "sync", "Syncing..., ignore propagated blocks.");
                return;
            }
            BroadcastsHandler::propagate_new_blocks(sealed.index(0));
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
                let mut sent_transaction_hashes = sent_transaction_hashes_mutex.lock();

                if !sent_transaction_hashes.contains_key(&hash) {
                    sent_transaction_hashes.insert(hash, 0);
                    SyncStorage::insert_received_transaction(transaction_rlp);
                }
            }
        }
    }
}

/// Configuration for IPC service.
#[derive(Debug, Clone)]
pub struct ServiceConfiguration {
    /// Sync config.
    pub sync: SyncConfig,
    /// Network configuration.
    pub net: NetworkConfig,
}
