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
use acore::client::{BlockChainClient, ChainNotify};
use acore::spec::Spec;
use acore::views::HeaderView;
use aion_types::H256;
use futures::{Future, Stream};
use kvdb::KeyValueDB;
use std::collections::BTreeMap;
use std::sync::Arc;
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
use rustc_hex::ToHex;

pub mod action;
pub mod error;
mod event;
mod handler;
pub mod storage;

const STATUS_REQ_INTERVAL: u64 = 5;
const GET_BLOCK_HEADERS_INTERVAL: u64 = 50;
const STATICS_INTERVAL: u64 = 15;
const BROADCAST_TRANSACTIONS_INTERVAL: u64 = 50;
const REPUTATION_HANDLE_INTERVAL: u64 = 30;
const SYNC_STATIC_CAPACITY: usize = 25;
const REQUEST_SIZE: u64 = 96;

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
            Duration::from_millis(GET_BLOCK_HEADERS_INTERVAL),
        ).for_each(move |_| {
            let size = REQUEST_SIZE;

            for _ in 0..4 {
                if let Some(mut node) = P2pMgr::get_an_active_node() {
                    let from = SyncStorage::get_synced_block_number() + 1;
                    BlockHeadersHandler::get_headers_from_node(&mut node, from, size);
                }
            }

            Ok(())
        })
            .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(get_block_headers_task);

        let broadcast_transactions_task = Interval::new(
            Instant::now(),
            Duration::from_millis(BROADCAST_TRANSACTIONS_INTERVAL),
        ).for_each(move |_| {
            BroadcastsHandler::broad_new_transactions();

            Ok(())
        })
            .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(broadcast_transactions_task);

        let reputation_handle_task = Interval::new(
            Instant::now(),
            Duration::from_secs(REPUTATION_HANDLE_INTERVAL),
        ).for_each(move |_| {
            let mut active_nodes = P2pMgr::get_nodes(ALIVE);
            active_nodes.sort_by(|a, b| {
                if a.reputation != b.reputation {
                    b.reputation.cmp(&a.reputation)
                } else {
                    b.best_block_num.cmp(&a.best_block_num)
                }
            });

            let mut top16_nodes: Vec<_> = active_nodes
                .iter()
                .map(|ref node| node.node_hash)
                .collect::<Vec<_>>();
            if top16_nodes.len() > 16 {
                top16_nodes.split_off(16);
                P2pMgr::replace_top16_node_hashes(top16_nodes);
            } else {
                P2pMgr::replace_top16_node_hashes(top16_nodes);
            }
            Ok(())
        })
            .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(reputation_handle_task);

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
                let mut count = 0;
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
                        if count == SYNC_STATIC_CAPACITY {
                            break;
                        }
                    }
                }
                info!(target: "sync", "{:-^127}","");

                SyncStorage::set_synced_block_number_last_time(block_number_now);
                SyncStorage::set_sync_speed(sync_speed as u16);

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
                                trace!(target: "sync", "BROADCASTTX received.");
                            }
                            SyncAction::BROADCASTBLOCK => {
                                trace!(target: "sync", "BROADCASTBLOCK received.");
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
        SyncStorage::reset();
    }
}

/// Sync configuration
#[derive(Debug, Clone, Copy)]
pub struct SyncConfig {
    /// Max blocks to download ahead
    pub max_download_ahead_blocks: usize,
    pub genesis_hash: H256,
}

impl Default for SyncConfig {
    fn default() -> SyncConfig {
        SyncConfig {
            max_download_ahead_blocks: 20000,
            genesis_hash: H256::from(0),
        }
    }
}

/// Sync initialization parameters.
pub struct Params {
    /// Configuration.
    pub config: SyncConfig,
    /// Blockchain client.
    pub client: Arc<BlockChainClient>,
    /// Network layer configuration.
    pub network_config: NetworkConfig,
    /// Spec
    pub spec: Spec,
    /// DB
    pub db: Arc<KeyValueDB>,
}

pub struct NetworkService {
    pub config: NetworkConfig,
}

/// Sync
pub struct Sync {
    /// Network service
    network: NetworkService,
}

impl Sync {
    /// Create handler with the network service
    pub fn get_instance(params: Params) -> Arc<Sync> {
        SyncStorage::init(params.client, params.spec, params.db);

        let genesis_hash = params.config.genesis_hash;
        info!(target: "sync", "Genesis hash: {:?}", genesis_hash);

        if let Ok(light_client) = SyncStorage::get_light_client().write() {
            let db = light_client.get_db();
            info!(target: "sync", "keys {:?}", db.keys());

            let number = db
                .iter("headers")
                .max_by_key(|(_, v)| HeaderView::new(v.to_vec().as_slice()).number());
            info!(target: "sync", "number {:?}", number);
        }

        let service = NetworkService {
            config: params.network_config.clone(),
        };
        Arc::new(Sync { network: service })
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
            start_block_number: 0,
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

    fn enode(&self) -> Option<String> {
        Some(P2pMgr::get_local_node().get_node_id())
    }

    fn transactions_stats(&self) -> BTreeMap<H256, TransactionStats> {
        BTreeMap::new()
    }

    fn active(&self) -> Vec<ActivePeerInfo> {
        let ac_nodes = P2pMgr::get_nodes(ALIVE);
        ac_nodes
            .into_iter()
            .map(|node| ActivePeerInfo {
                highest_block_number: node.best_block_num,
                id: node.node_id.to_hex(),
                ip: node.ip_addr.ip.to_hex(),
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

        P2pMgr::enable(self.network_config());
        debug!(target: "sync", "###### P2P enabled... ######");

        NetManager::enable(sync_handler);
        debug!(target: "sync", "###### network enabled... ######");

        SyncMgr::enable();
        debug!(target: "sync", "###### SYNC enabled... ######");
    }

    fn stop_network(&self) {
        SyncMgr::disable();
        P2pMgr::disable();
    }

    fn network_config(&self) -> NetworkConfig {
        NetworkConfig::from(self.network.config.clone())
    }
}

impl ChainNotify for Sync {
    fn new_blocks(
        &self,
        _imported: Vec<H256>,
        _invalid: Vec<H256>,
        _enacted: Vec<H256>,
        _retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _proposed: Vec<Vec<u8>>,
        _duration: u64,
    ) {
        info!(target: "sync", "new_blocks in chain...");
    }

    fn start(&self) {
        info!(target: "sync", "starting...");
    }

    fn stop(&self) {
        info!(target: "sync", "stopping...");
    }

    fn broadcast(&self, _message: Vec<u8>) {}

    fn transactions_received(&self, _transactions: &[Vec<u8>]) {}
}

/// Configuration for IPC service.
#[derive(Debug, Clone)]
pub struct ServiceConfiguration {
    /// Sync config.
    pub sync: SyncConfig,
    /// Network configuration.
    pub net: NetworkConfig,
}
