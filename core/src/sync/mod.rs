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
mod route;
mod wrappers;
mod storage;
mod node_info;
#[cfg(test)]
mod test;

use std::sync::RwLock;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::thread;
use std::ptr;
use rustc_hex::ToHex;
use client::BlockChainClient;
use client::BlockId;
use client::BlockStatus;
use client::ChainNotify;
use transaction::UnverifiedTransaction;
use aion_types::{H256,U256};
use futures::Future;
use futures::Stream;
use lru_cache::LruCache;
use rlp::UntrustedRlp;
use tokio::runtime::Runtime;
use tokio::timer::Interval;
//use bytes::BufMut;
//use byteorder::{BigEndian,ByteOrder};

//use p2p::Node;
use p2p::ChannelBuffer;
use p2p::Config;
use p2p::Mgr;
use p2p::Callable;
//use sync::route::VERSION;
//use sync::route::MODULE;
use sync::route::ACTION;
use sync::handler::status;
use sync::handler::bodies;
use sync::handler::headers;
// use sync::handler::broadcast;
// use sync::handler::import;
use sync::wrappers::{HeaderWrapper, BlockWrapper};
use sync::node_info::NodeInfo;
use header::Header;

use sync::storage::ActivePeerInfo;
use sync::storage::PeerInfo;
use sync::storage::SyncState;
use sync::storage::SyncStatus;
use sync::storage::SyncStorage;
use sync::storage::TransactionStats;

const HEADERS_CAPACITY: u64 = 256;
const STATUS_REQ_INTERVAL: u64 = 2;
const BLOCKS_BODIES_REQ_INTERVAL: u64 = 50;
const BLOCKS_IMPORT_INTERVAL: u64 = 50;
const BROADCAST_TRANSACTIONS_INTERVAL: u64 = 50;
const INTERVAL_STATUS: u64 = 5000;
const INTERVAL_HEADERS: u64 = 2;
const INTERVAL_BODIES: u64 = 2;
const INTERVAL_STATISICS: u64 = 5;

const MAX_TX_CACHE: usize = 20480;
const MAX_BLOCK_CACHE: usize = 32;

pub struct Sync {
    config: Arc<Config>,
    client: Arc<BlockChainClient>,
    runtime: Arc<Runtime>,
    p2p: Mgr,

    /// collection of headers wrappers
    headers: Arc<RwLock<HashMap<u64, HeaderWrapper>>>,

    /// collection of headers wrappers
    blocks: Arc<RwLock<HashMap<u64, BlockWrapper>>>,

    /// active nodes info
    node_info: Arc<RwLock<HashMap<u64, NodeInfo>>>,

    /// local best td
    local_best_td: Arc<RwLock<U256>>,

    /// local best block number
    local_best_block_number: Arc<RwLock<u64>>,

    /// network best td
    network_best_td: Arc<RwLock<U256>>,

    /// network best block number
    network_best_block_number: Arc<RwLock<u64>>,

    /// cache block hashes which have been downloaded
    cached_downloaded_block_hashes: Arc<Mutex<LruCache<H256, u8>>>,

    /// cache block hashes which have been imported
    cached_imported_block_hashes: Arc<Mutex<LruCache<H256, u8>>>,

    /// cache tx hash which has been stored and broadcasted
    cached_tx_hashes: Arc<Mutex<LruCache<H256, u8>>>,

    /// cache block hash which has been committed and broadcasted
    cached_block_hashes: Arc<Mutex<LruCache<H256, u8>>>,
}

impl Sync {
    pub fn new(config: Config, client: Arc<BlockChainClient>) -> Sync {
        let local_best_td: U256 = client.chain_info().total_difficulty;
        let local_best_block_number: u64 = client.chain_info().best_block_number;
        let config = Arc::new(config);
        Sync {
            config: config.clone(),
            client,
            p2p: Mgr::new(config),
            runtime: Arc::new(Runtime::new().expect("tokio runtime")),
            headers: Arc::new(RwLock::new(HashMap::new())),
            blocks: Arc::new(RwLock::new(HashMap::new())),
            node_info: Arc::new(RwLock::new(HashMap::new())),

            local_best_td: Arc::new(RwLock::new(local_best_td)),
            local_best_block_number: Arc::new(RwLock::new(local_best_block_number)),
            network_best_td: Arc::new(RwLock::new(local_best_td)),
            network_best_block_number: Arc::new(RwLock::new(local_best_block_number)),
            cached_downloaded_block_hashes: Arc::new(Mutex::new(LruCache::new(MAX_BLOCK_CACHE))),
            cached_imported_block_hashes: Arc::new(Mutex::new(LruCache::new(MAX_BLOCK_CACHE))),
            cached_tx_hashes: Arc::new(Mutex::new(LruCache::new(MAX_TX_CACHE))),
            cached_block_hashes: Arc::new(Mutex::new(LruCache::new(MAX_BLOCK_CACHE))),
        }
    }

    pub fn run(&self, sync: Arc<Sync>) {
        // counters
        let runtime = self.runtime.clone();
        let executor = Arc::new(runtime.executor());

        // init p2p;
        let mut p2p = &self.p2p.clone();
        let mut p2p_0 = p2p.clone();
        thread::spawn(move || {
            p2p_0.run(sync.clone());
        });

        // interval statisics
        //         let executor_statisics = executor.clone();
        //         let p2p_statisics = p2p.clone();
        //         executor_statisics.spawn(
        //             Interval::new(
        //                 Instant::now(),
        //                 Duration::from_secs(INTERVAL_STATISICS)
        //             ).for_each(move |_| {
        //                 match p2p_statisics.nodes.try_read() {
        //                     Ok(nodes) => {
        //                         let mut total: usize = 0;
        //                         let mut active: usize = 0;
        //                         if nodes.len() > 0 {
        //                             let mut active_nodes = vec![];
        //                             info!(target: "p2p", "{:-^127}","");
        //                             info!(target: "p2p","              td         bn          bh                    addr                 rev      conn  seed");
        //                             info!(target: "p2p", "{:-^127}","");
        //
        //                             for (_hash, node) in nodes.iter(){
        //                                 total += 1;
        //                                 if node.state == STATE::ACTIVE {
        //                                     active += 1;
        //                                     active_nodes.push(node.clone());
        //                                 }
        //                             }
        //
        //                             if active_nodes.len() > 0 {
        //                                 active_nodes.sort_by(|a, b| {
        //                                     if a.total_difficulty != b.total_difficulty {
        //                                         b.total_difficulty.cmp(&a.total_difficulty)
        //                                     } else {
        //                                         b.block_num.cmp(&a.block_num)
        //                                     }
        //                                 });
        //                                 for node in active_nodes.iter() {
        //                                     info!(target: "p2p",
        //                                         "{:>16}{:>11}{:>12}{:>24}{:>20}{:>10}{:>6}",
        //                                         format!("{}",node.total_difficulty),
        //                                         node.block_num,
        //                                         format!("{}",node.block_hash),
        //                                         node.addr.to_formatted_string(),
        //                                         String::from_utf8_lossy(&node.revision).trim(),
        //                                         format!("{}",node.connection),
        //                                         match node.if_seed{
        //                                             true => "y",
        //                                             _ => " "
        //                                         }
        //                                     );
        //                                 }
        //
        //                             }
        //
        //                             info!(target: "p2p", "{:-^127}","");
        //                         }
        //                         info!(target: "p2p", "total/active {}/{}", total, active);
        //                     },
        //                     Err(err) => {
        //                         warn!(target:"p2p", "executor statisics: try read {:?}", err);
        //                     }
        //                 }
        //                 Ok(())
        //             }).map_err(|err| error!(target: "p2p", "executor statisics: {:?}", err))
        //         );

        // interval status
        let p2p_1 = p2p.clone();
        let executor_status = executor.clone();
        executor_status.spawn(
            Interval::new(Instant::now(), Duration::from_millis(INTERVAL_STATUS))
                .for_each(move |_| {
                    status::send_random(p2p_1.clone());
                    Ok(())
                })
                .map_err(|err| error!(target: "p2p", "executor status: {:?}", err)),
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
        //                .map_err(|err| error!(target: "p2p", "executor headers: {:?}", err)),
        //        );

        //        let executor_bodies = executor.clone();
        //        let queue2 = self.queue.clone();
        //        let p2p_3 = p2p.clone();
        //        executor_bodies.spawn(
        //            Interval::new(Instant::now(), Duration::from_secs(INTERVAL_BODIES))
        //                .for_each(move |_| {
        //                    if let Ok(mut queue) = queue2.try_write() {
        //                        if let Some((num, wrapper)) = queue
        //                            .clone()
        //                            .iter()
        //                            .filter(|(_, w)| {
        //                                w.with_status.value() == 0
        //                            })
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
        //                                            let header: Header =
        //                                                rlp.as_val().expect("should not be err");
        //                                            cb.body.put_slice(&header.hash());
        //                                        }
        //                                        cb.head.len = cb.body.len() as u32;
        //                                        p2p_3.send(p2p_3.clone(), num.clone(), cb);
        //                                        if let Some(w) = queue.get_mut(num) {
        //                                            (*w).timestamp = SystemTime::now();
        //                                            (*w).with_status = WithStatus::WaitForBody(hw.clone());
        //                                        };
        //                                    }
        //                                    _ => (),
        //                                };
        //                            }
        //                    }
        //                    Ok(())
        //                })
        //                .map_err(|err| error!(target: "p2p", "executor bodies: {:?}", err)),
        //        );

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
        //                .map_err(|err| error!(target: "p2p", "executor status: {:?}", err)),
        //        );
    }

    pub fn shutdown(&self) { &self.p2p.shutdown(); }
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
            start_block_number: self.client.chain_info().best_block_number,
            last_imported_block_number: None,
            highest_block_number: match self.network_best_block_number.read() {
                Ok(number) => Some(*number),
                Err(_) => None,
            },
            blocks_received: 0,
            blocks_total: 0,
            //num_peers: { get_nodes_count(ALIVE.value()) },
            num_peers: 0,
            num_active_peers: 0,
        }
    }

    /// Get sync peers
    fn peers(&self) -> Vec<PeerInfo> {
        // let mut peer_info_list = Vec::new();
        // let peer_nodes = get_all_nodes();
        // for peer in peer_nodes.iter() {
        //     let peer_info = PeerInfo {
        //         id: Some(peer.get_node_id()),
        //     };
        //     peer_info_list.push(peer_info);
        // }
        // peer_info_list
        Vec::new()
    }

    fn enode(&self) -> Option<String> {
        // Some(get_local_node().get_node_id())
        None
    }

    fn transactions_stats(&self) -> BTreeMap<H256, TransactionStats> { BTreeMap::new() }

    fn active(&self) -> Vec<ActivePeerInfo> {
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
        vec![]
    }
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
        // if get_all_nodes_count() == 0 {
        //     return;
        // }

        if !imported.is_empty() {
            let client = self.client.clone();
            let chain_info = client.chain_info();
            let min_imported_block_number = chain_info.best_block_number + 1;
            let mut max_imported_block_number = 0;
            for hash in imported.iter() {
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

            let synced_block_number = chain_info.best_block_number;
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
                    // import::import_staged_blocks(&block_hash);
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
                // import::import_staged_blocks(&hash);
            }
        }

        if !sealed.is_empty() {
            debug!(target: "sync", "Propagating blocks...");
            SyncStorage::insert_imported_block_hashes(sealed.clone());
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
                let chain_info = &self.client.chain_info();
                let node_info = self.node_info.clone();
                let headers = self.headers.clone();
                status::receive_res(p2p, chain_info, node_info, headers, hash, cb)
            }
            ACTION::HEADERSREQ => {
                let client = self.client.clone();
                headers::receive_req(p2p, hash, client, cb)
            }
            ACTION::HEADERSRES => {
                let headers = self.headers.clone();
                let downloaded_hashes = self.cached_downloaded_block_hashes.clone();
                let imported_hashes = self.cached_imported_block_hashes.clone();
                headers::receive_res(p2p, hash, cb, headers, downloaded_hashes, imported_hashes)
            }
            ACTION::BODIESREQ => {
                let client = self.client.clone();
                bodies::receive_req(p2p, hash, client, cb)
            }
            ACTION::BODIESRES => {
                let headers = self.headers.clone();
                //                let blocks = self.blocks.clone();
                let chain_info = self.client.chain_info();
                let downloaded_hashes = self.cached_downloaded_block_hashes.clone();
                bodies::receive_res(p2p, hash, cb, headers, chain_info, downloaded_hashes)
            }
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
        if let Ok(mut headers) = self.headers.write() {
            headers.remove(&hash);
        }
    }
}
