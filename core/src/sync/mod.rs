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

mod handler;
mod route;
mod wrappers;
mod node_info;
mod storage;
#[cfg(test)]
mod test;

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use itertools::Itertools;
// use std::time::SystemTime;
use std::collections::{HashMap};
use client::{BlockId, BlockChainClient, ChainNotify};
use transaction::UnverifiedTransaction;
use aion_types::{H256,U256};
use futures::Future;
use futures::Stream;
use lru_cache::LruCache;
use tokio::runtime::TaskExecutor;
use tokio::timer::Interval;
use futures::sync::oneshot;
use futures::sync::oneshot::Sender;
use parking_lot::{Mutex, RwLock};
// use bytes::BufMut;
//use byteorder::{BigEndian,ByteOrder};

use p2p::ChannelBuffer;
use p2p::Config;
use p2p::Mgr;
use p2p::Callable;
use sync::route::VERSION;
use sync::route::MODULE;
use sync::route::ACTION;
use sync::handler::status;
use sync::handler::bodies;
use sync::handler::headers;
use sync::handler::broadcast;
use sync::handler::import;
use sync::node_info::{NodeInfo, Mode};
use sync::storage::SyncStorage;

const _HEADERS_CAPACITY: u64 = 256;
const _STATUS_REQ_INTERVAL: u64 = 2;
const _BLOCKS_BODIES_REQ_INTERVAL: u64 = 50;
const _BLOCKS_IMPORT_INTERVAL: u64 = 50;
const BROADCAST_TRANSACTIONS_INTERVAL: u64 = 50;
const INTERVAL_STATUS: u64 = 5000;
const INTERVAL_HEADERS: u64 = 100;
const INTERVAL_BODIES: u64 = 100;
const INTERVAL_STATISICS: u64 = 10;
const MAX_TX_CACHE: usize = 20480;
const MAX_BLOCK_CACHE: usize = 32;

pub struct Sync {
    _config: Arc<Config>,

    client: Arc<BlockChainClient>,

    shutdown_hooks: Arc<Mutex<Vec<Sender<()>>>>,

    p2p: Mgr,

    /// Sync local storage cache
    storage: Arc<SyncStorage>,

    /// active nodes info
    node_info: Arc<RwLock<HashMap<u64, RwLock<NodeInfo>>>>,

    /// local best td
    _local_best_td: Arc<RwLock<U256>>,

    /// local best block number
    _local_best_block_number: Arc<RwLock<u64>>,

    /// network best td
    _network_best_td: Arc<RwLock<U256>>,

    /// network best block number
    network_best_block_number: Arc<RwLock<u64>>,

    /// cache tx hash which has been stored and broadcasted
    _cached_tx_hashes: Arc<Mutex<LruCache<H256, u8>>>,

    /// cache block hash which has been committed and broadcasted
    _cached_block_hashes: Arc<Mutex<LruCache<H256, u8>>>,
}

impl Sync {
    pub fn new(config: Config, client: Arc<BlockChainClient>) -> Sync {
        let local_best_td: U256 = client.chain_info().total_difficulty;
        let local_best_block_number: u64 = client.chain_info().best_block_number;
        let config = Arc::new(config);

        let mut token_rules: Vec<[u32; 2]> = vec![];
        let sync_rule_base =
            ((VERSION::V0.value() as u32) << 16) + ((MODULE::SYNC.value() as u32) << 8);
        token_rules.push([
            sync_rule_base + ACTION::STATUSREQ.value() as u32,
            sync_rule_base + ACTION::STATUSRES.value() as u32,
        ]);
        token_rules.push([
            sync_rule_base + ACTION::HEADERSREQ.value() as u32,
            sync_rule_base + ACTION::HEADERSRES.value() as u32,
        ]);
        token_rules.push([
            sync_rule_base + ACTION::BODIESREQ.value() as u32,
            sync_rule_base + ACTION::BODIESRES.value() as u32,
        ]);

        Sync {
            _config: config.clone(),
            client,
            p2p: Mgr::new(config, token_rules),
            shutdown_hooks: Arc::new(Mutex::new(Vec::new())),
            storage: Arc::new(SyncStorage::new()),
            node_info: Arc::new(RwLock::new(HashMap::new())),
            network_best_block_number: Arc::new(RwLock::new(local_best_block_number)),
            _local_best_td: Arc::new(RwLock::new(local_best_td)),
            _local_best_block_number: Arc::new(RwLock::new(local_best_block_number)),
            _network_best_td: Arc::new(RwLock::new(local_best_td)),
            _cached_tx_hashes: Arc::new(Mutex::new(LruCache::new(MAX_TX_CACHE))),
            _cached_block_hashes: Arc::new(Mutex::new(LruCache::new(MAX_BLOCK_CACHE))),
        }
    }

    pub fn register_callback(&self, callback: Arc<Callable>) {
        self.p2p.register_callback(callback);
    }

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
        let (tx, rx) = oneshot::channel::<()>();
        executor.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_STATISICS)
            ).for_each(move |_| {
                let (total_len, active_nodes) = p2p_statics.get_statics_info();
                {
                    let local_best_number = client_statics.chain_info().best_block_number;
                    let active_len = active_nodes.len();
                    info!(target: "sync", "total/active {}/{}  ,local_best_num {}", total_len, active_len, local_best_number);

                    info!(target: "sync", "{:-^127}", "");
                    info!(target: "sync", "                              td         bn          bh                    addr                 rev      conn  seed       mode");
                    info!(target: "sync", "{:-^127}", "");

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
                                    info!(target: "sync",
                                          "{:>32}{:>11}{:>12}{:>24}{:>20}{:>10}{:>6}{:>11}",
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

                    info!(target: "sync", "{:-^127}", "");
                }
                Ok(())
            })
            .map_err(|err| error!(target: "sync", "executor statics: {:?}", err))
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
                    status::send_random(p2p_status.clone(), node_info_status.clone());
                    Ok(())
                })
                .map_err(|err| error!(target: "sync", "executor status: {:?}", err))
                .select(rx.map_err(|_| {}))
                .map(|_| ())
                .map_err(|_| ()),
        );
        shutdown_hooks.push(tx);

        // sync headers thread
        let p2p_header = p2p.clone();
        let node_info_header = self.node_info.clone();
        let client_header = self.client.clone();
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
                    );
                    Ok(())
                })
                .map_err(|err| error!(target: "sync", "executor header: {:?}", err))
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
                .map_err(|err| error!(target: "sync", "executor body: {:?}", err))
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
            Interval::new(Instant::now(), Duration::from_millis(INTERVAL_BODIES))
                .for_each(move |_| {
                    import::import_blocks(
                        client_import.clone(),
                        storage_import.clone(),
                        node_info_import.clone(),
                    );
                    Ok(())
                })
                .map_err(|err| error!(target: "sync", "executor import: {:?}", err))
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
                Duration::from_millis(BROADCAST_TRANSACTIONS_INTERVAL),
            )
            .for_each(move |_| {
                broadcast::broad_new_transactions(p2p_broadcast.clone(), storage_broadcast.clone());

                Ok(())
            })
            .map_err(|e| error!("interval errored; err={:?}", e))
            .select(rx.map_err(|_| {}))
            .map(|_| ())
            .map_err(|_| ()),
        );
        shutdown_hooks.push(tx);
    }

    pub fn shutdown(&self) {
        // Shutdown p2p
        &self.p2p.shutdown();
        info!(target:"sync", "sync shutdown start");
        // Shutdown runtime tasks
        let mut shutdown_hooks = self.shutdown_hooks.lock();
        while !shutdown_hooks.is_empty() {
            if let Some(shutdown_hook) = shutdown_hooks.pop() {
                match shutdown_hook.send(()) {
                    Ok(_) => {
                        debug!(target: "sync", "shutdown signal sent");
                    }
                    Err(err) => {
                        error!(target: "sync", "shutdown: {:?}", err);
                    }
                }
            }
        }
        info!(target:"sync", "sync shutdown finished");
    }
}

pub trait SyncProvider: Send + ::std::marker::Sync {
    // /// Get sync status
    // fn status(&self) -> SyncStatus;

    // /// Get peers information
    // fn peers(&self) -> Vec<PeerInfo>;

    /// Get the enode if available.
    fn enode(&self) -> Option<String>;

    // /// Returns propagation count for pending transactions.
    // fn transactions_stats(&self) -> BTreeMap<H256, TransactionStats>;

    // /// Get active nodes
    // fn active(&self) -> Vec<ActivePeerInfo>;
}

impl SyncProvider for Sync {
    // /// Get sync status
    // fn status(&self) -> SyncStatus {
    //     // TODO:  only set start_block_number/highest_block_number.
    //     SyncStatus {
    //         state: SyncState::Idle,
    //         protocol_version: 0,
    //         network_id: 256,
    //         start_block_number: self.client.chain_info().best_block_number,
    //         last_imported_block_number: None,
    //         highest_block_number: match self.network_best_block_number.read() {
    //             Ok(number) => Some(*number),
    //             Err(_) => None,
    //         },
    //         blocks_received: 0,
    //         blocks_total: 0,
    //         //num_peers: { get_nodes_count(ALIVE.value()) },
    //         num_peers: 0,
    //         num_active_peers: 0,
    //     }
    // }

    // /// Get sync peers
    // fn peers(&self) -> Vec<PeerInfo> {
    // let mut peer_info_list = Vec::new();
    // let peer_nodes = get_all_nodes();
    // for peer in peer_nodes.iter() {
    //     let peer_info = PeerInfo {
    //         id: Some(peer.get_node_id()),
    //     };
    //     peer_info_list.push(peer_info);
    // }
    // peer_info_list
    //     Vec::new()
    // }

    fn enode(&self) -> Option<String> {
        // Some(get_local_node().get_node_id())
        None
    }

    // fn transactions_stats(&self) -> BTreeMap<H256, TransactionStats> { BTreeMap::new() }

    // fn active(&self) -> Vec<ActivePeerInfo> {
    // let nodes = &self.p2p.get_active_nodes();
    // nodes
    //     .into_iter()
    //     .map(|node| {
    //         ActivePeerInfo {
    //             highest_block_number: node.block_num,
    //             id: node.id.to_hex(),
    //             ip: node.addr.ip.to_hex(),
    //         }
    //     })
    //     .collect()
    //     vec![]
    // }
}

impl ChainNotify for Sync {
    // TODO: this function, which has registered in client notify, doesn't work
    fn new_blocks(
        &self,
        imported: Vec<H256>,
        _invalid: Vec<H256>,
        _enacted: Vec<H256>,
        retracted: Vec<H256>,
        sealed: Vec<H256>,
        _proposed: Vec<Vec<u8>>,
        _duration: u64,
    )
    {
        // if get_all_nodes_count() == 0 {
        //     return;
        // }

        // TODO: to think whether we still need to do the following or not.
        if !imported.is_empty() {
            // let chain_info = client.chain_info();
            // let min_imported_block_number = chain_info.best_block_number + 1;
            // let mut max_imported_block_number = 0;
            // info!(target: "sync", "{} new blocks saved.", imported.len());
            for hash in &imported {
                let client = self.client.clone();
                let block_id = BlockId::Hash(*hash);
                if let Some(block_number) = client.block_number(block_id) {
                    trace!(target: "sync", "New block #{}, hash: {}.", block_number, hash);
                }
                import::import_staged_blocks(hash, client, self.storage.clone());
                // if client.block_status(block_id) == BlockStatus::InChain {
                //     if let Some(block_number) = client.block_number(block_id) {
                //         if max_imported_block_number < block_number {
                //             max_imported_block_number = block_number;
                //         }
                //     }
                // }
            }

            // The imported blocks are not new or not yet in chain. Do not notify in this case.
            // if max_imported_block_number < min_imported_block_number {
            //     return;
            // }

            // TODO: to understand why we need to do this
            // let synced_block_number = chain_info.best_block_number;
            // if max_imported_block_number <= synced_block_number {
            //     let mut hashes = Vec::new();
            //     for block_number in max_imported_block_number..synced_block_number + 1 {
            //         let block_id = BlockId::Number(block_number);
            //         if let Some(block_hash) = client.block_hash(block_id) {
            //             hashes.push(block_hash);
            //         }
            //     }
            //     if hashes.len() > 0 {
            //         SyncStorage::remove_imported_block_hashes(hashes);
            //     }
            // }

            // for block_number in min_imported_block_number..max_imported_block_number + 1 {
            //     let block_id = BlockId::Number(block_number);
            //     if let Some(blk) = client.block(block_id) {
            //         let block_hash = blk.hash();
            //         info!(target: "sync",
            //                 "New block #{} {}, with {} txs added in chain.",
            //                 block_number, block_hash, blk.transactions_count());
            //         // import::import_staged_blocks(&block_hash);
            //         // if let Some(time) = SyncStorage::get_requested_time(&block_hash) {
            //         //     info!(target: "sync",
            //         //         "New block #{} {}, with {} txs added in chain, time elapsed: {:?}.",
            //         //         block_number, block_hash, blk.transactions_count(), SystemTime::now().duration_since(time).expect("importing duration"));
            //         // }
            //     }
            // }
        }

        // If retracted is not empty, it means a chain reorg occurred.
        // Reset mode of all connecting nodes to NORMAL.
        // TODO: need more thoughts if this is good idea
        if !retracted.is_empty() {
            debug!(target: "sync", "Chain reorg. Reset the syncing mode of all connecting nodes to NORMAL.");
            for (_, node_info_lock) in &*self.node_info.read() {
                let mut node_info = node_info_lock.write();
                node_info.mode = Mode::Normal;
            }
        }

        if !sealed.is_empty() {
            debug!(target: "sync", "Propagating blocks...");
            self.storage.insert_imported_block_hashes(sealed.clone());
            broadcast::propagate_new_blocks(self.p2p.clone(), &sealed[0], self.client.clone());
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
        use rlp::UntrustedRlp;
        if transactions.len() == 1 {
            let transaction_rlp = transactions[0].clone();
            if let Ok(tx) = UntrustedRlp::new(&transaction_rlp).as_val() {
                let transaction: UnverifiedTransaction = tx;
                let hash = transaction.hash();
                let sent_transaction_hashes_mutex = self.storage.get_sent_transaction_hashes();
                let mut sent_transaction_hashes = sent_transaction_hashes_mutex.lock();

                if !sent_transaction_hashes.contains_key(hash) {
                    sent_transaction_hashes.insert(hash.clone(), 0);
                    self.storage.insert_received_transaction(transaction_rlp);
                }
            }
        }
    }
}

impl Callable for Sync {
    fn handle(&self, hash: u64, cb: ChannelBuffer) {
        let p2p = self.p2p.clone();
        match ACTION::from(cb.head.action) {
            ACTION::STATUSREQ => {
                if cb.head.len != 0 {
                    // TODO: kill the node
                }
                let chain_info = &self.client.chain_info();
                status::receive_req(p2p, chain_info, hash)
            }
            ACTION::STATUSRES => {
                let genesis_hash = self.client.chain_info().genesis_hash;
                status::receive_res(
                    p2p,
                    self.node_info.clone(),
                    hash,
                    cb,
                    self.network_best_block_number.clone(),
                    genesis_hash,
                )
            }
            ACTION::HEADERSREQ => {
                let client = self.client.clone();
                headers::receive_req(p2p, hash, client, cb)
            }
            ACTION::HEADERSRES => headers::receive_res(p2p, hash, cb, self.storage.clone()),
            ACTION::BODIESREQ => {
                let client = self.client.clone();
                bodies::receive_req(p2p, hash, client, cb)
            }
            ACTION::BODIESRES => bodies::receive_res(p2p, hash, cb, self.storage.clone()),
            ACTION::BROADCASTTX => {
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
            ACTION::BROADCASTBLOCK => {
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
            ACTION::UNKNOWN => (),
        };
    }

    fn disconnect(&self, hash: u64) {
        info!(target: "sync", "stop syncing from disconnected node: {}", &hash);
        let mut node_info = self.node_info.write();
        node_info.remove(&hash);
        drop(node_info);
        trace!(target: "sync", "finish dropping node_info");

        let mut headers = self.storage.headers_with_bodies_requested().lock();
        headers.remove(&hash);
        drop(headers);
        trace!(target: "sync", "finish dropping headers_with_bodies_requested");

        let mut headers = self.storage.downloaded_headers().lock();
        headers.retain(|x| x.node_hash != hash);

        trace!(target: "sync", "finish cleaning disconnected node: {}", &hash);
    }
}
