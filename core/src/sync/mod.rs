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

//! Sync module. Handle transmitting and recieving nodes status, transactions, blocks
//!
//! # Tasks
//! * sync_statics: to print nodes info at regular intervals
//! * node_status: to get/send status from/to other nodes
//! * headers: to get/send a number of continuous block headers from/to other nodes
//! * bodies: to get/send a number of continuous block bodies from/to other nodes
//! * import: to import downloaded blocks to verification queue
//! * broadcast: to get/send the newest transactions and block from/to other nodes

mod handler;
mod action;
mod wrappers;
mod node_info;
mod storage;
mod sync_provider;

use std::sync::{Arc,Weak};
use std::time::Duration;
use std::time::Instant;
use itertools::Itertools;
use std::collections::{HashMap};
use client::{BlockId, BlockChainClient, ChainNotify};
use transaction::UnverifiedTransaction;
use aion_types::{H256,U256};
use futures::Future;
use futures::Stream;
use tokio::runtime::TaskExecutor;
use tokio::timer::Interval;
use futures::sync::oneshot;
use futures::sync::oneshot::Sender;
use parking_lot::{Mutex, RwLock};

use p2p::{ ChannelBuffer, Config, Mgr, Callable, PROTOCAL_VERSION, Module};
use sync::action::Action;
use sync::handler::status;
use sync::handler::bodies;
use sync::handler::headers;
use sync::handler::broadcast;
use sync::handler::import;
use sync::node_info::{NodeInfo, Mode};
use sync::storage::SyncStorage;
use sync::sync_provider::SyncStatus;

pub use sync::sync_provider::SyncProvider;

const INTERVAL_TRANSACTIONS_BROADCAST: u64 = 50;
const INTERVAL_STATUS: u64 = 5000;
const INTERVAL_HEADERS: u64 = 100;
const INTERVAL_BODIES: u64 = 100;
const INTERVAL_IMPORT: u64 = 50;
const INTERVAL_STATISICS: u64 = 10;

/// Sync manager
pub struct Sync {
    /// Blockchain kernel interface
    client: Arc<BlockChainClient>,

    /// Oneshots to shutdown threads
    shutdown_hooks: Arc<Mutex<Vec<Sender<()>>>>,

    /// P2p manager
    p2p: Mgr,

    /// Sync local storage cache
    storage: Arc<SyncStorage>,

    /// active nodes info
    node_info: Arc<RwLock<HashMap<u64, RwLock<NodeInfo>>>>,

    /// network best td
    network_best_td: Arc<RwLock<U256>>,

    /// network best block number
    network_best_block_number: Arc<RwLock<u64>>,
}

impl Sync {
    /// constructor
    pub fn new(config: Config, client: Arc<BlockChainClient>) -> Sync {
        let local_best_td: U256 = client.chain_info().total_difficulty;
        let local_best_block_number: u64 = client.chain_info().best_block_number;

        let mut token_rules: Vec<[u32; 2]> = vec![];
        let sync_rule_base =
            ((PROTOCAL_VERSION as u32) << 16) + ((Module::SYNC.value() as u32) << 8);
        token_rules.push([
            sync_rule_base + Action::STATUSREQ.value() as u32,
            sync_rule_base + Action::STATUSRES.value() as u32,
        ]);
        token_rules.push([
            sync_rule_base + Action::HEADERSREQ.value() as u32,
            sync_rule_base + Action::HEADERSRES.value() as u32,
        ]);
        token_rules.push([
            sync_rule_base + Action::BODIESREQ.value() as u32,
            sync_rule_base + Action::BODIESRES.value() as u32,
        ]);

        Sync {
            client,
            p2p: Mgr::new(config, token_rules),
            shutdown_hooks: Arc::new(Mutex::new(Vec::new())),
            storage: Arc::new(SyncStorage::new()),
            node_info: Arc::new(RwLock::new(HashMap::new())),
            network_best_td: Arc::new(RwLock::new(local_best_td)),
            network_best_block_number: Arc::new(RwLock::new(local_best_block_number)),
        }
    }

    /// register callback
    pub fn register_callback(&self, callback: Weak<Callable>) {
        self.p2p.register_callback(callback);
    }

    /// run sync instance
    pub fn run(&self, executor: TaskExecutor) {
        // init p2p
        let p2p = &self.p2p.clone();
        let mut p2p_0 = p2p.clone();
        p2p_0.run(executor.clone());

        let mut shutdown_hooks = self.shutdown_hooks.lock();

        // interval statics
        let node_info = self.node_info.clone();
        let p2p_statics = p2p.clone();
        let client_statics = self.client.clone();
        let storage_statics = self.storage.clone();
        let (tx, rx) = oneshot::channel::<()>();
        executor.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_STATISICS)
            ).for_each(move |_| {
                let (total_len, active_nodes) = p2p_statics.get_statics_info();
                {
                    let local_best_number = client_statics.chain_info().best_block_number;
                    let local_best_hash = client_statics.chain_info().best_block_hash;
                    let local_total_difficulty = client_statics.chain_info().total_difficulty;
                    let active_len = active_nodes.len();
                    info!(target: "sync_statics", "total/active {}/{}, local_best_num {}, hash {}, diff {}", total_len, active_len, local_best_number, local_best_hash, local_total_difficulty);
                    let (recorded_blocks_size, recorded_blocks_capacity) = storage_statics.recorded_blocks_hashes_statics();
                    let (staged_blocks_size, staged_blocks_capacity) = storage_statics.staged_blocks_statics();
                    debug!(target: "sync_statics", "recorded cache size/capacity {}/{}", recorded_blocks_size, recorded_blocks_capacity);
                    debug!(target: "sync_statics", "staged cache size/capacity {}/{}", staged_blocks_size, staged_blocks_capacity);
                    debug!(target: "sync_statics", "lightning syncing height: {}", storage_statics.lightning_base());
                    info!(target: "sync_statics", "{:-^130}", "");
                    info!(target: "sync_statics", "                                 td         bn          bh                    addr                 rev      conn  seed       mode");
                    info!(target: "sync_statics", "{:-^130}", "");

                    if active_len > 0 {
                        let mut nodes_info = HashMap::new();
                        let nodes = node_info.read();
                        for (hash, info_lock) in nodes.iter() {
                            let info = info_lock.read();
                            nodes_info.insert(hash.clone(), info.clone());
                        }
                        drop(nodes);

                        for (hash, info) in nodes_info.iter()
                            .sorted_by(|a, b|
                                if a.1.total_difficulty != b.1.total_difficulty {
                                    b.1.total_difficulty.cmp(&a.1.total_difficulty)
                                } else {
                                    b.1.best_block_number.cmp(&a.1.best_block_number)
                                })
                            .iter()
                            {
                                if let Some((addr, revision, connection, seed)) = active_nodes.get(*hash) {
                                    info!(target: "sync_statics",
                                          "{:>35}{:>11}{:>12}{:>24}{:>20}{:>10}{:>6}{:>11}",
                                          format!("{}", info.total_difficulty),
                                          format!("{}", info.best_block_number),
                                          format!("{}", info.best_block_hash),
                                          addr,
                                          revision,
                                          connection,
                                          seed,
                                          format!("{}", info.mode)
                                    );
                                }
                            }
                    }

                    info!(target: "sync_statics", "{:-^130}", "");
                }
                Ok(())
            })
            .map_err(|err| error!(target: "sync_statics", "executor statics: {:?}", err))
            .select(rx.map_err(|_| {}))
            .map(|_| ())
            .map_err(|_| ())
        );
        shutdown_hooks.push(tx);

        // status thread
        let p2p_status = p2p.clone();
        let node_info_status = self.node_info.clone();
        let (tx, rx) = oneshot::channel::<()>();
        executor.spawn(
            Interval::new(Instant::now(), Duration::from_millis(INTERVAL_STATUS))
                .for_each(move |_| {
                    status::send_req(p2p_status.clone(), node_info_status.clone());
                    Ok(())
                })
                .map_err(|err| error!(target: "sync_status", "executor status: {:?}", err))
                .select(rx.map_err(|_| {}))
                .map(|_| ())
                .map_err(|_| ()),
        );
        shutdown_hooks.push(tx);

        // sync headers thread
        let p2p_header = p2p.clone();
        let node_info_header = self.node_info.clone();
        let client_header = self.client.clone();
        let storage_header = self.storage.clone();
        let (tx, rx) = oneshot::channel::<()>();
        executor.spawn(
            Interval::new(Instant::now(), Duration::from_millis(INTERVAL_HEADERS))
                .for_each(move |_| {
                    let chain_info = client_header.chain_info();
                    let local_total_diff: U256 = chain_info.total_difficulty;
                    let local_best_block_number: u64 = chain_info.best_block_number;
                    headers::sync_headers(
                        p2p_header.clone(),
                        node_info_header.clone(),
                        &local_total_diff,
                        local_best_block_number,
                        storage_header.clone(),
                    );
                    Ok(())
                })
                .map_err(|err| error!(target: "sync_headers", "executor header: {:?}", err))
                .select(rx.map_err(|_| {}))
                .map(|_| ())
                .map_err(|_| ()),
        );
        shutdown_hooks.push(tx);

        // sync bodies thread
        let p2p_body = p2p.clone();
        let storage_body = self.storage.clone();
        let (tx, rx) = oneshot::channel::<()>();
        executor.spawn(
            Interval::new(Instant::now(), Duration::from_millis(INTERVAL_BODIES))
                .for_each(move |_| {
                    bodies::sync_bodies(p2p_body.clone(), storage_body.clone());
                    Ok(())
                })
                .map_err(|err| error!(target: "sync_bodies", "executor body: {:?}", err))
                .select(rx.map_err(|_| {}))
                .map(|_| ())
                .map_err(|_| ()),
        );
        shutdown_hooks.push(tx);

        // import thread
        let client_import = self.client.clone();
        let storage_import = self.storage.clone();
        let node_info_import = self.node_info.clone();
        let (tx, rx) = oneshot::channel::<()>();
        executor.spawn(
            Interval::new(Instant::now(), Duration::from_millis(INTERVAL_IMPORT))
                .for_each(move |_| {
                    import::import_blocks(
                        client_import.clone(),
                        storage_import.clone(),
                        node_info_import.clone(),
                    );
                    Ok(())
                })
                .map_err(|err| error!(target: "sync_import", "executor import: {:?}", err))
                .select(rx.map_err(|_| {}))
                .map(|_| ())
                .map_err(|_| ()),
        );
        shutdown_hooks.push(tx);

        let executor_broadcast = executor.clone();
        let p2p_broadcast = p2p.clone();
        let storage_broadcast = self.storage.clone();
        let (tx, rx) = oneshot::channel::<()>();
        executor_broadcast.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_millis(INTERVAL_TRANSACTIONS_BROADCAST),
            )
            .for_each(move |_| {
                broadcast::broad_new_transactions(p2p_broadcast.clone(), storage_broadcast.clone());

                Ok(())
            })
            .map_err(|e| error!(target: "sync_broadcast","interval errored; err={:?}", e))
            .select(rx.map_err(|_| {}))
            .map(|_| ())
            .map_err(|_| ()),
        );
        shutdown_hooks.push(tx);
    }

    /// shutdown routine
    pub fn shutdown(&self) {
        info!(target:"sync_shutdown", "sync shutdown start");
        // Shutdown runtime tasks
        let mut shutdown_hooks = self.shutdown_hooks.lock();
        while !shutdown_hooks.is_empty() {
            if let Some(shutdown_hook) = shutdown_hooks.pop() {
                match shutdown_hook.send(()) {
                    Ok(_) => {
                        debug!(target: "sync_shutdown", "shutdown signal sent");
                    }
                    Err(err) => {
                        debug!(target: "sync_shutdown", "shutdown err: {:?}", err);
                    }
                }
            }
        }
        // Shutdown p2p
        &self.p2p.shutdown();
        &self.p2p.clear_callback();
        info!(target:"sync_shutdown", "sync shutdown finished");
    }

    /// get local node info to fill back to config file
    pub fn get_local_node_info(&self) -> &String { self.p2p.get_local_node_info() }

    /// Determine if the node is doing a major sync
    fn is_syncing(&self) -> bool {
        let local_best_block_number = self.client.chain_info().best_block_number;
        let network_best_block_number = *self.network_best_block_number.read();
        let local_total_difficulty = self.client.chain_info().total_difficulty;
        let network_total_difficulty = *self.network_best_td.read();
        if local_best_block_number + 4 < network_best_block_number
            && local_total_difficulty <= network_total_difficulty
        {
            true
        } else {
            false
        }
    }
}

impl SyncProvider for Sync {
    /// Get sync status for rpc request
    fn status(&self) -> SyncStatus {
        // TODO:  only set start_block_number/highest_block_number.
        SyncStatus {
            protocol_version: PROTOCAL_VERSION as u8,
            network_id: self.p2p.get_net_id(),
            start_block_number: self.client.chain_info().best_block_number,
            highest_block_number: Some(*self.network_best_block_number.read()),
            num_peers: self.p2p.get_active_nodes_len() as usize,
        }
    }
}

impl ChainNotify for Sync {
    // TODO: this function, which has registered in client notify, doesn't work
    fn new_blocks(
        &self,
        imported: Vec<H256>,
        _invalid: Vec<H256>,
        enacted: Vec<H256>,
        retracted: Vec<H256>,
        sealed: Vec<H256>,
        _proposed: Vec<Vec<u8>>,
        _duration: u64,
    )
    {
        if !imported.is_empty() {
            for hash in &imported {
                let client = self.client.clone();
                let block_id = BlockId::Hash(*hash);
                if let Some(block_number) = client.block_number(block_id) {
                    debug!(target: "sync_notify", "New block #{}, hash: {}.", block_number, hash);
                }
                import::import_staged_blocks(hash, client, self.storage.clone());
            }
        }

        // If retracted is not empty, it means a chain reorg occurred.
        // Reset mode of all connecting nodes to NORMAL.
        // TODO: need more thoughts whether this is a good idea
        if !retracted.is_empty() {
            debug!(target: "sync_notify", "Chain reorg. Reset the syncing mode of all connecting nodes to NORMAL.");
            for (_, node_info_lock) in &*self.node_info.read() {
                let mut node_info = node_info_lock.write();
                node_info.mode = Mode::Normal;
            }
        }

        // Add sealed blocks into imported blocks cache
        if !sealed.is_empty() {
            self.storage.insert_recorded_blocks_hashes(sealed.clone());
        }

        // Broadcast the new main-chain blocks unless the node is syncing
        if !self.is_syncing() && !enacted.is_empty() {
            trace!(target: "sync_notify", "Propagating blocks...");
            broadcast::propagate_new_blocks(self.p2p.clone(), enacted, self.client.clone());
        }
    }

    fn start(&self) {
        info!(target: "sync_notify", "starting...");
    }

    fn stop(&self) {
        info!(target: "sync_notify", "stopping...");
    }

    fn broadcast(&self, _message: Vec<u8>) {}

    fn transactions_received(&self, transactions: &[Vec<u8>]) {
        use rlp::UntrustedRlp;
        if transactions.len() == 1 {
            let transaction_rlp = transactions[0].clone();
            if let Ok(tx) = UntrustedRlp::new(&transaction_rlp).as_val() {
                let transaction: UnverifiedTransaction = tx;
                let hash = transaction.hash();
                let recorded_transaction_hashes_mutex = self.storage.recorded_transaction_hashes();
                let mut recorded_transaction_hashes = recorded_transaction_hashes_mutex.lock();

                if !recorded_transaction_hashes.contains_key(hash) {
                    recorded_transaction_hashes.insert(hash.clone(), 0);
                    self.storage.insert_received_transaction(transaction_rlp);
                }
            }
        }
    }
}

impl Callable for Sync {
    fn handle(&self, hash: u64, cb: ChannelBuffer) {
        let p2p = self.p2p.clone();
        match Action::from(cb.head.action) {
            Action::STATUSREQ => {
                if cb.head.len != 0 {
                    // TODO: kill the node
                }
                let chain_info = &self.client.chain_info();
                status::receive_req(p2p, chain_info, hash, cb.head.ver)
            }
            Action::STATUSRES => {
                let genesis_hash = self.client.chain_info().genesis_hash;
                status::receive_res(
                    p2p,
                    self.node_info.clone(),
                    hash,
                    cb,
                    self.network_best_td.clone(),
                    self.network_best_block_number.clone(),
                    genesis_hash,
                )
            }
            Action::HEADERSREQ => {
                let client = self.client.clone();
                headers::receive_req(p2p, hash, client, cb)
            }
            Action::HEADERSRES => headers::receive_res(p2p, hash, cb, self.storage.clone()),
            Action::BODIESREQ => {
                let client = self.client.clone();
                bodies::receive_req(p2p, hash, client, cb)
            }
            Action::BODIESRES => bodies::receive_res(p2p, hash, cb, self.storage.clone()),
            Action::BROADCASTTX => {
                let client = self.client.clone();
                broadcast::handle_broadcast_tx(
                    p2p,
                    hash,
                    cb,
                    client,
                    self.node_info.clone(),
                    self.storage.clone(),
                    self.network_best_block_number.clone(),
                )
            }
            Action::BROADCASTBLOCK => {
                let client = self.client.clone();
                broadcast::handle_broadcast_block(
                    p2p,
                    hash,
                    cb,
                    client,
                    self.storage.clone(),
                    self.network_best_block_number.clone(),
                )
            }
            // TODO: kill the node
            Action::UNKNOWN => (),
        };
    }

    fn disconnect(&self, hash: u64) {
        debug!(target: "sync_disconnect", "stop syncing from disconnected node: {}", &hash);
        let mut node_info = self.node_info.write();
        node_info.remove(&hash);
        drop(node_info);
        trace!(target: "sync_disconnect", "finish dropping node_info");

        let mut headers = self.storage.headers_with_bodies_requested().lock();
        headers.remove(&hash);
        drop(headers);
        trace!(target: "sync_disconnect", "finish dropping headers_with_bodies_requested");

        let mut headers = self.storage.downloaded_headers().lock();
        headers.retain(|x| x.node_hash != hash);

        trace!(target: "sync_disconnect", "finish cleaning disconnected node: {}", &hash);
    }
}
