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

use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use std::time::Instant;
use itertools::Itertools;
// use std::time::SystemTime;
use std::collections::{HashMap};
use std::thread;
use client::{BlockId, BlockChainClient, BlockStatus, ChainNotify};
// use transaction::UnverifiedTransaction;
use aion_types::{H256,U256};
use futures::Future;
use futures::Stream;
use lru_cache::LruCache;
use tokio::runtime::Runtime;
use tokio::timer::Interval;
use futures::sync::oneshot;
use futures::sync::oneshot::Sender;
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
// use sync::handler::broadcast;
use sync::handler::import;
use sync::node_info::NodeInfo;
use sync::storage::SyncStorage;

const _HEADERS_CAPACITY: u64 = 256;
const _STATUS_REQ_INTERVAL: u64 = 2;
const _BLOCKS_BODIES_REQ_INTERVAL: u64 = 50;
const _BLOCKS_IMPORT_INTERVAL: u64 = 50;
const _BROADCAST_TRANSACTIONS_INTERVAL: u64 = 50;
const INTERVAL_STATUS: u64 = 5000;
const INTERVAL_HEADERS: u64 = 100;
const INTERVAL_BODIES: u64 = 100;
const INTERVAL_STATISICS: u64 = 5;
const MAX_TX_CACHE: usize = 20480;
const MAX_BLOCK_CACHE: usize = 32;

pub struct Sync {
    _config: Arc<Config>,

    client: Arc<BlockChainClient>,

    shutdown_hook: Arc<RwLock<Option<Sender<()>>>>,

    p2p: Mgr,

    /// Sync local storage cache
    storage: Arc<SyncStorage>,

    /// active nodes info
    node_info: Arc<RwLock<HashMap<u64, NodeInfo>>>,

    /// local best td
    _local_best_td: Arc<RwLock<U256>>,

    /// local best block number
    _local_best_block_number: Arc<RwLock<u64>>,

    /// network best td
    _network_best_td: Arc<RwLock<U256>>,

    /// network best block number
    _network_best_block_number: Arc<RwLock<u64>>,

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
            shutdown_hook: Arc::new(RwLock::new(None)),
            storage: Arc::new(SyncStorage::new()),
            node_info: Arc::new(RwLock::new(HashMap::new())),
            _local_best_td: Arc::new(RwLock::new(local_best_td)),
            _local_best_block_number: Arc::new(RwLock::new(local_best_block_number)),
            _network_best_td: Arc::new(RwLock::new(local_best_td)),
            _network_best_block_number: Arc::new(RwLock::new(local_best_block_number)),
            _cached_tx_hashes: Arc::new(Mutex::new(LruCache::new(MAX_TX_CACHE))),
            _cached_block_hashes: Arc::new(Mutex::new(LruCache::new(MAX_BLOCK_CACHE))),
        }
    }

    pub fn run(&self, sync: Arc<Sync>) {
        // counters
        let mut runtime = Runtime::new().expect("new sync runtime failed!");
        let executor = runtime.executor();

        // init p2p;
        let p2p = &self.p2p.clone();
        let mut p2p_0 = p2p.clone();
        thread::spawn(move || {
            p2p_0.run(sync.clone());
        });

        // interval statics
        let node_info = self.node_info.clone();
        let executor_statics = executor.clone();
        let p2p_statics = p2p.clone();
        executor_statics.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_STATISICS)
            ).for_each(move |_| {
                let (total_len, active_nodes) = p2p_statics.get_statics_info();
                {
                    let active_len = active_nodes.len();
                    info!(target: "sync", "total/active {}/{}", total_len, active_len);

                    info!(target: "sync", "{:-^127}", "");
                    info!(target: "sync", "              td         bn          bh                    addr                 rev      conn  seed");
                    info!(target: "sync", "{:-^127}", "");

                    if active_len > 0 {
                        if let Ok(nodes) = node_info.read()
                            {
                                for (hash, info) in nodes.iter()
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
                                                  "{:>16}{:>11}{:>12}{:>24}{:>20}{:>10}{:>6}",
                                                  format!("{}",info.total_difficulty),
                                            format!("{}",info.best_block_number),
                                                  format!("{}", info.best_block_hash),
                                                  addr,
                                                  revision,
                                                  connection,
                                                  seed
                                            );
                                        }
                                    }
                            }
                    }

                    info!(target: "sync", "{:-^127}", "");
                }

                Ok(())
            }).map_err(|err| error!(target: "sync", "executor statics: {:?}", err))
        );

        // status thread
        let p2p_status = p2p.clone();
        let executor_status = executor.clone();
        let node_info_status = self.node_info.clone();
        executor_status.spawn(
            Interval::new(Instant::now(), Duration::from_millis(INTERVAL_STATUS))
                .for_each(move |_| {
                    status::send_random(p2p_status.clone(), node_info_status.clone());
                    Ok(())
                })
                .map_err(|err| error!(target: "p2p", "executor status: {:?}", err)),
        );

        // sync headers thread
        let p2p_header = p2p.clone();
        let executor_header = executor.clone();
        let node_info_header = self.node_info.clone();
        let client_header = self.client.clone();
        executor_header.spawn(
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
                .map_err(|err| error!(target: "sync", "executor header: {:?}", err)),
        );

        // sync bodies thread
        let p2p_body = p2p.clone();
        let executor_body = executor.clone();
        let storage_body = self.storage.clone();
        executor_body.spawn(
            Interval::new(Instant::now(), Duration::from_millis(INTERVAL_BODIES))
                .for_each(move |_| {
                    bodies::sync_bodies(p2p_body.clone(), storage_body.clone());
                    Ok(())
                })
                .map_err(|err| error!(target: "sync", "executor body: {:?}", err)),
        );

        // import thread
        let executor_import = executor.clone();
        let client_import = self.client.clone();
        let storage_import = self.storage.clone();
        executor_import.spawn(
            Interval::new(Instant::now(), Duration::from_millis(INTERVAL_BODIES))
                .for_each(move |_| {
                    import::import_blocks(client_import.clone(), storage_import.clone());
                    Ok(())
                })
                .map_err(|err| error!(target: "sync", "executor import: {:?}", err)),
        );

        //        let executor_headers = executor.clone();
        //        let queue1 = self.queue.clone();
        //        let client = self.client.clone();
        //        let synced_number = self.cached_synced_block_num.clone();
        //        let p2p_2 = p2p.clone();
        //        executor_headers.spawn(
        //            Interval::new(Instant::now(), Duration::from_secs(INTERVAL_HEADERS))
        //                .for_each(move |_| {
        //                    // make it constant
        //                    let chain_info = client.chain_info();
        //                    if let Ok(start) = synced_number.read() {
        //                        headers::send(p2p_2.clone(), *start, &chain_info, queue1.clone());
        //                    }
        //                    //                     p2p.get_node_by_td(10);
        //                    Ok(())
        //                })
        //                .map_err(|err| error!(target: "sync", "executor headers: {:?}", err)),
        //        );

        // let executor_bodies = executor.clone();
        // let storage = self.storage.clone();
        // let cached_downloaded = self.cached_downloaded_block_hashes.clone();
        // let cached_imported = self.cached_imported_block_hashes.clone();
        // let p2p_3 = p2p.clone();
        // executor_bodies.spawn(
        //     Interval::new(Instant::now(), Duration::from_secs(INTERVAL_BODIES))
        //         .for_each(move |_| {
        //             let mut req = ChannelBuffer::new();
        //             req.head.ver = VERSION::V0.value();
        //             req.head.ctrl = MODULE::SYNC.value();
        //             req.head.action = ACTION::BODIESREQ.value();

        //             let mut hws = Vec::new();
        //             if let Ok(mut downloaded_headers) = storage.downloaded_headers.try_lock() {
        //                 while let Some(hw) = downloaded_headers.pop_front() {
        //                     if !hw.headers.is_empty() {
        //                         hws.push(hw);
        //                     }
        //                 }
        //             }
        //             for hw in hws.iter() {
        //                 let mut req = req.clone();
        //                 req.body.clear();

        //                 let mut header_requested = Vec::new();
        //                 for header in hw.headers.iter() {
        //                     let is_downloaded = if let Ok(mut hashes) = cached_downloaded.lock() {
        //                         hashes.contains_key(&header.hash())
        //                     } else {
        //                         warn!(target: "sync", "downloaded_block_hashes lock failed");
        //                         false
        //                     };
        //                     let is_imported = if let Ok(mut hashes) = cached_imported.lock() {
        //                         hashes.contains_key(&header.hash())
        //                     } else {
        //                         warn!(target: "sync", "imported_block_hashes lock failed");
        //                         false
        //                     };
        //                     if !is_downloaded && !is_imported {
        //                         req.body.put_slice(&header.hash());
        //                         header_requested.push(header.clone());
        //                     }
        //                 }

        //                 let body_len = req.body.len();
        //                 if body_len > 0 {
        //                     if let Ok(ref mut headers_with_bodies_requested) =
        //                         storage.headers_with_bodies_request.lock()
        //                     {
        //                         if !headers_with_bodies_requested.contains_key(&hw.node_hash) {
        //                             req.head.len = body_len as u32;
        //                             println!("hash:{}", hw.node_hash);
        //                             p2p_3.send(hw.node_hash, req);

        //                             trace!(target: "sync", "Sync blocks bodies req sent...");
        //                             let mut hw = hw.clone();
        //                             hw.timestamp = SystemTime::now();
        //                             hw.headers.clear();
        //                             hw.headers.extend(header_requested);
        //                             headers_with_bodies_requested.insert(hw.node_hash, hw);
        //                         } else {
        //                             println!("node is busy")
        //                         }
        //                     }
        //                 }
        //             }
        //             Ok(())
        //         })
        //         .map_err(|err| error!(target: "sync", "executor bodies: {:?}", err)),
        // );

        //        let executor_import = executor.clone();
        //        executor_bodies.spawn(
        //            Interval::new(Instant::now(), Duration::from_secs(INTERVAL_BODIES))
        //                .for_each(move |_| {
        //                    if let Ok(mut queue) = queue2.try_write(){
        //                        if let Some((num,wrapper)) = queue.clone()
        //                            .iter()
        //                            .filter(|(_,w)| match w.with_status { WithStatus::GetHeader(_) => true, _ => false })
        //                            .next()
        //                            {
        //                                match wrapper.with_status {
        //                                    WithStatus::GetHeader(ref hw) => {
        //                                        let mut cb = ChannelBuffer::new();
        //                                        cb.head.ver = VERSION::V0.value();
        //                                        cb.head.ctrl = MODULE::SYNC.value();
        //                                        cb.head.action = ACTION::BODIESREQ.value();
        //                                        for h in hw.clone() {
        //                                            let rlp = UntrustedRlp::new(&h);
        //                                            let header:Header = rlp.as_val().expect("should not be err");
        //                                            cb.body.put_slice(&header.hash());
        //                                        }
        //                                        cb.head.len = cb.body.len() as u32;
        //                                        send(num,cb,nodes_bodies.clone());
        //                                        if let Some(w) =queue.get_mut(num){
        //                                            (*w).timestamp = SystemTime::now();
        //                                            (*w).with_status = WithStatus::WaitForBody(hw.clone());
        //                                        };
        //                                    }
        //                                    _ => ()
        //                                };
        //                            }
        //                    }
        //                    Ok(())
        //                })
        //                .map_err(|err| error!(target: "sync", "executor status: {:?}", err)),
        //        );

        // bind shutdown hook
        let (tx, rx) = oneshot::channel::<()>();
        {
            match self.shutdown_hook.write() {
                Ok(mut guard) => *guard = Some(tx),
                Err(_error) => {}
            }
        }

        // clear
        drop(executor_statics);
        drop(executor_status);
        drop(executor_header);
        drop(executor_body);
        drop(executor);
        runtime.block_on(rx.map_err(|_| ())).unwrap();
        runtime.shutdown_now().wait().unwrap();
        debug!(target:"sync", "shutdown executors");
    }

    pub fn shutdown(&self) {
        &self.p2p.shutdown();
        if let Ok(mut lock) = self.shutdown_hook.write() {
            if lock.is_some() {
                let tx = lock.take().unwrap();
                match tx.send(()) {
                    Ok(_) => {
                        debug!(target: "sync", "shutdown signal sent");
                    }
                    Err(err) => {
                        error!(target: "sync", "shutdown: {:?}", err);
                    }
                }
            }
        }
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
        // if get_all_nodes_count() == 0 {
        //     return;
        // }

        // TODO: to think whether we still need to do the following or not.
        if !imported.is_empty() {
            let client = self.client.clone();
            let chain_info = client.chain_info();
            let min_imported_block_number = chain_info.best_block_number + 1;
            let mut max_imported_block_number = 0;
            for hash in imported {
                let block_id = BlockId::Hash(hash);
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

            for block_number in min_imported_block_number..max_imported_block_number + 1 {
                let block_id = BlockId::Number(block_number);
                if let Some(blk) = client.block(block_id) {
                    let block_hash = blk.hash();
                    info!(target: "sync",
                            "New block #{} {}, with {} txs added in chain.",
                            block_number, block_hash, blk.transactions_count());
                    // import::import_staged_blocks(&block_hash);
                    // if let Some(time) = SyncStorage::get_requested_time(&block_hash) {
                    //     info!(target: "sync",
                    //         "New block #{} {}, with {} txs added in chain, time elapsed: {:?}.",
                    //         block_number, block_hash, blk.transactions_count(), SystemTime::now().duration_since(time).expect("importing duration"));
                    // }
                }
            }
        }

        if !sealed.is_empty() {
            debug!(target: "sync", "Propagating blocks...");
            self.storage.insert_imported_block_hashes(sealed.clone());
            // TODO: enable broadcast after implemention done
            // broadcast::propagate_blocks(sealed.index(0), SyncStorage::get_block_chain());
        }
    }

    fn start(&self) {
        info!(target: "sync", "starting...");
    }

    fn stop(&self) {
        info!(target: "sync", "stopping...");
    }

    fn broadcast(&self, _message: Vec<u8>) {}

    fn transactions_received(&self, _transactions: &[Vec<u8>]) {
        // if transactions.len() == 1 {
        //     let transaction_rlp = transactions[0].clone();
        //     if let Ok(tx) = UntrustedRlp::new(&transaction_rlp).as_val() {
        //         let transaction: UnverifiedTransaction = tx;
        //         let hash = transaction.hash();
        //         let sent_transaction_hashes_mutex = SyncStorage::get_sent_transaction_hashes();
        //         let mut lock = sent_transaction_hashes_mutex.lock();

        //         if let Ok(ref mut sent_transaction_hashes) = lock {
        //             if !sent_transaction_hashes.contains_key(hash) {
        //                 sent_transaction_hashes.insert(hash.clone(), 0);
        //                 SyncStorage::insert_received_transaction(transaction_rlp);
        //             }
        //         }
        //     }
        // }
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
            ACTION::STATUSRES => status::receive_res(p2p, self.node_info.clone(), hash, cb),
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
            ACTION::BROADCASTTX => (),
            ACTION::BROADCASTBLOCK => (),
            // TODO: kill the node
            ACTION::UNKNOWN => (),
        };
    }

    fn disconnect(&self, hash: u64) {
        if let Ok(mut node_info) = self.node_info.write() {
            node_info.remove(&hash);
        }

        if let Ok(mut headers) = self.storage.headers_with_bodies_requested().lock() {
            headers.remove(&hash);
        }
        // TODO-SYNC: remove downloaded headers with given node hash
    }
}
