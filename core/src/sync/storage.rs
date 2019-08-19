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

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;
use block::Block;
use client::{BlockChainClient, BlockChainInfo, BlockQueueInfo};
use header::Header;
use sync::helper::Wrapper;
use aion_types::{H256, U256};
use lru_cache::LruCache;
use crate_state::Storage;
use tokio::runtime::{Runtime, TaskExecutor};

lazy_static! {
    static ref BLOCK_CHAIN: Storage<RwLock<BlockChain>> = Storage::new();
    static ref SYNC_EXECUTORS: Storage<RwLock<SyncExecutor>> = Storage::new();
    static ref LOCAL_STATUS: Storage<RwLock<LocalStatus>> = Storage::new();
    static ref NETWORK_STATUS: Storage<RwLock<NetworkStatus>> = Storage::new();
    static ref DOWNLOADED_HEADERS: Storage<Mutex<VecDeque<Wrapper>>> = Storage::new();
    static ref HEADERS_WITH_BODIES_REQUESTED: Storage<Mutex<HashMap<u64, Wrapper>>> =
        Storage::new();
    static ref DOWNLOADED_BLOCKS: Storage<Mutex<VecDeque<BlocksWrapper>>> = Storage::new();
    static ref REQUESTED_BLOCK_HASHES: Storage<Mutex<LruCache<H256, SystemTime>>> = Storage::new();
    static ref IMPORTED_BLOCK_HASHES: Storage<Mutex<LruCache<H256, u8>>> = Storage::new();
    static ref DOWNLOADED_BLOCK_HASHES: Storage<Mutex<LruCache<H256, u8>>> = Storage::new();
    static ref SENT_TRANSACTION_HASHES: Storage<Mutex<LruCache<H256, u8>>> = Storage::new();
    static ref RECEIVED_TRANSACTIONS: Storage<Mutex<VecDeque<Vec<u8>>>> = Storage::new();
    static ref STAGED_BLOCKS: Storage<Mutex<LruCache<H256, Vec<Vec<u8>>>>> = Storage::new();
    static ref STAGED_BLOCK_HASHES: Storage<Mutex<LruCache<H256, u8>>> = Storage::new();
}

pub const MAX_DOWNLOADED_HEADERS_COUNT: usize = 4096;
const MAX_CACHED_BLOCK_HASHES: usize = 32;
const MAX_CACHED_TRANSACTION_HASHES: usize = 20480;
const MAX_RECEIVED_TRANSACTIONS_COUNT: usize = 20480;

#[derive(Clone)]
struct BlockChain {
    inner: Option<Arc<BlockChainClient>>,
}

#[derive(Clone)]
struct SyncExecutor {
    inner: Option<Arc<Runtime>>,
}

pub struct SyncStorage;

impl SyncStorage {
    pub fn init(client: Arc<BlockChainClient>) {
        if let Some(_) = BLOCK_CHAIN.try_get() {
            if let Ok(mut block_chain) = BLOCK_CHAIN.get().write() {
                if let Some(_) = block_chain.inner {
                } else {
                    block_chain.inner = Some(client);
                }
            }
            if let Ok(mut sync_executor) = SYNC_EXECUTORS.get().write() {
                if let Some(_) = sync_executor.inner {
                } else {
                    sync_executor.inner = Some(Arc::new(Runtime::new().expect("Tokio Runtime")));
                }
            }
        } else {
            let synced_block_number = client.chain_info().best_block_number;

            let block_chain = BlockChain {
                inner: Some(client),
            };

            let sync_executor = SyncExecutor {
                inner: Some(Arc::new(Runtime::new().expect("Tokio Runtime"))),
            };

            let mut local_status = LocalStatus::new();
            let mut network_status = NetworkStatus::new();
            let mut downloaded_headers = VecDeque::new();
            let mut headers_with_bodies_requested = HashMap::new();
            let mut downloaded_blocks = VecDeque::new();
            let mut requested_block_hashes = LruCache::new(MAX_CACHED_BLOCK_HASHES);
            let mut imported_block_hashes = LruCache::new(MAX_CACHED_BLOCK_HASHES);
            let mut downloaded_block_hashes = LruCache::new(MAX_CACHED_BLOCK_HASHES * 2);
            let mut sent_transaction_hases = LruCache::new(MAX_CACHED_TRANSACTION_HASHES);
            let mut received_transactions = VecDeque::new();
            let mut staged_blocks = LruCache::new(MAX_CACHED_BLOCK_HASHES);
            let mut staged_block_hashes = LruCache::new(MAX_CACHED_BLOCK_HASHES);

            local_status.synced_block_number = synced_block_number;
            local_status.synced_block_number_last_time = synced_block_number;
            LOCAL_STATUS.set(RwLock::new(local_status));
            NETWORK_STATUS.set(RwLock::new(network_status));
            DOWNLOADED_HEADERS.set(Mutex::new(downloaded_headers));
            HEADERS_WITH_BODIES_REQUESTED.set(Mutex::new(headers_with_bodies_requested));
            DOWNLOADED_BLOCKS.set(Mutex::new(downloaded_blocks));
            REQUESTED_BLOCK_HASHES.set(Mutex::new(requested_block_hashes));
            IMPORTED_BLOCK_HASHES.set(Mutex::new(imported_block_hashes));
            DOWNLOADED_BLOCK_HASHES.set(Mutex::new(downloaded_block_hashes));
            SENT_TRANSACTION_HASHES.set(Mutex::new(sent_transaction_hases));
            RECEIVED_TRANSACTIONS.set(Mutex::new(received_transactions));
            STAGED_BLOCKS.set(Mutex::new(staged_blocks));
            STAGED_BLOCK_HASHES.set(Mutex::new(staged_block_hashes));

            BLOCK_CHAIN.set(RwLock::new(block_chain));
            SYNC_EXECUTORS.set(RwLock::new(sync_executor));
        }
    }

    pub fn get_block_chain() -> Arc<BlockChainClient> {
        BLOCK_CHAIN
            .get()
            .read()
            .expect("get_block_chain")
            .clone()
            .inner
            .expect("get_client")
    }

    pub fn get_chain_info() -> BlockChainInfo {
        let client = BLOCK_CHAIN
            .get()
            .read()
            .expect("get_chain_info")
            .clone()
            .inner
            .expect("get_chain_info");
        client.chain_info()
    }

    pub fn get_executor() -> TaskExecutor {
        let rt = SYNC_EXECUTORS
            .get()
            .read()
            .expect("get_executor")
            .clone()
            .inner
            .expect("get_executor");
        rt.executor()
    }

    pub fn set_synced_block_number(synced_block_number: u64) {
        if let Ok(mut local_status) = LOCAL_STATUS.get().write() {
            local_status.synced_block_number = synced_block_number;
        }
    }

    pub fn get_synced_block_number() -> u64 {
        if let Ok(local_status) = LOCAL_STATUS.get().read() {
            return local_status.synced_block_number;
        }
        0
    }

    pub fn set_synced_block_number_last_time(synced_block_number_last_time: u64) {
        if let Ok(mut local_status) = LOCAL_STATUS.get().write() {
            local_status.synced_block_number_last_time = synced_block_number_last_time;
        }
    }

    pub fn get_synced_block_number_last_time() -> u64 {
        if let Ok(local_status) = LOCAL_STATUS.get().read() {
            return local_status.synced_block_number_last_time;
        }
        0
    }

    pub fn set_sync_speed(sync_speed: u16) {
        if let Ok(mut local_status) = LOCAL_STATUS.get().write() {
            local_status.sync_speed = sync_speed;
        }
    }

    pub fn get_sync_speed() -> u16 {
        if let Ok(local_status) = LOCAL_STATUS.get().read() {
            return local_status.sync_speed;
        }
        0
    }

    pub fn set_max_staged_block_number(max_staged_block_number: u64) {
        if let Ok(mut local_status) = LOCAL_STATUS.get().write() {
            local_status.max_staged_block_number = max_staged_block_number;
        }
    }

    pub fn get_max_staged_block_number() -> u64 {
        if let Ok(local_status) = LOCAL_STATUS.get().read() {
            return local_status.max_staged_block_number;
        }
        0
    }

    pub fn get_downloaded_headers() -> &'static Mutex<VecDeque<Wrapper>> {
        DOWNLOADED_HEADERS.get()
    }

    pub fn clear_downloaded_headers() {
        if let Ok(ref mut downloaded_headers) = DOWNLOADED_HEADERS.get().lock() {
            downloaded_headers.clear();
        }
    }

    pub fn get_headers_with_bodies_requested() -> &'static Mutex<HashMap<u64, Wrapper>> {
        HEADERS_WITH_BODIES_REQUESTED.get()
    }

    pub fn pick_headers_with_bodies_requested(node_hash: &u64) -> Option<Wrapper> {
        if let Ok(ref mut headers_with_bodies_requested) =
            HEADERS_WITH_BODIES_REQUESTED.get().lock()
        {
            headers_with_bodies_requested.remove(node_hash)
        } else {
            warn!(target: "sync", "headers_with_bodies_requested_mutex lock failed");
            None
        }
    }

    pub fn clear_headers_with_bodies_requested() {
        if let Ok(ref mut headers_with_bodies_requested) =
            HEADERS_WITH_BODIES_REQUESTED.get().lock()
        {
            headers_with_bodies_requested.clear();
        }
    }

    pub fn get_downloaded_blocks() -> &'static Mutex<VecDeque<BlocksWrapper>> {
        DOWNLOADED_BLOCKS.get()
    }

    pub fn insert_downloaded_headers(hw: Wrapper) {
        let downloaded_headers_mutex = DOWNLOADED_HEADERS.get();
        {
            let mut lock = downloaded_headers_mutex.lock();
            if let Ok(ref mut downloaded_headers) = lock {
                if downloaded_headers.len() <= MAX_DOWNLOADED_HEADERS_COUNT {
                    downloaded_headers.push_back(hw);
                } else {
                    warn!(target: "sync", "too many downloaded_headers...");
                }
            } else {
                warn!(target: "sync", "downloaded_headers_mutex lock failed");
            }
        }
    }

    pub fn insert_downloaded_blocks(bw: BlocksWrapper) {
        let downloaded_blocks_mutex = DOWNLOADED_BLOCKS.get();
        {
            let mut lock = downloaded_blocks_mutex.lock();
            if let Ok(ref mut downloaded_blocks) = lock {
                downloaded_blocks.push_back(bw);
            } else {
                warn!(target: "sync", "downloaded_blocks_mutex lock failed");
            }
        }
    }

    pub fn clear_downloaded_blocks() {
        if let Ok(ref mut downloaded_blocks) = DOWNLOADED_BLOCKS.get().lock() {
            downloaded_blocks.clear();
        }
    }

    pub fn get_network_best_block_number() -> u64 {
        if let Ok(network_status) = NETWORK_STATUS.get().read() {
            return network_status.best_block_num;
        }
        0
    }

    pub fn get_network_best_block_hash() -> H256 {
        if let Ok(network_status) = NETWORK_STATUS.get().read() {
            return network_status.best_hash;
        }
        H256::from(0)
    }

    pub fn get_network_total_diff() -> U256 {
        if let Ok(network_status) = NETWORK_STATUS.get().read() {
            return network_status.total_diff;
        }
        U256::from(0)
    }

    pub fn insert_requested_time(hash: H256) {
        if let Ok(ref mut requested_block_hashes) = REQUESTED_BLOCK_HASHES.get().lock() {
            if !requested_block_hashes.contains_key(&hash) {
                requested_block_hashes.insert(hash, SystemTime::now());
            }
        }
    }

    pub fn get_requested_time(hash: &H256) -> Option<SystemTime> {
        if let Ok(ref mut requested_block_hashes) = REQUESTED_BLOCK_HASHES.get().lock() {
            if let Some(time) = requested_block_hashes.get_mut(hash) {
                return Some(time.clone());
            }
        }
        None
    }

    pub fn clear_requested_blocks() {
        if let Ok(ref mut requested_block_hashes) = REQUESTED_BLOCK_HASHES.get().lock() {
            requested_block_hashes.clear();
        }
    }

    pub fn get_imported_block_hashes() -> &'static Mutex<LruCache<H256, u8>> {
        return IMPORTED_BLOCK_HASHES.get();
    }

    pub fn remove_imported_block_hashes(hashes: Vec<H256>) {
        if let Ok(ref mut imported_block_hashes) = IMPORTED_BLOCK_HASHES.get().lock() {
            for hash in hashes.iter() {
                imported_block_hashes.remove(&hash);
            }
        } else {
            warn!(target: "sync", "imported_block_hashes_mutex lock failed");
        }
    }

    // pub fn clear_imported_block_hashes() {
    //     if let Ok(ref mut imported_block_hashes) = IMPORTED_BLOCK_HASHES.get().lock() {
    //         imported_block_hashes.clear();
    //     }
    // }

    pub fn insert_imported_block_hashes(imported: Vec<H256>) {
        if let Ok(ref mut imported_block_hashes) = IMPORTED_BLOCK_HASHES.get().lock() {
            for hash in imported.iter() {
                imported_block_hashes.insert(*hash, 0);
            }
        } else {
            warn!(target: "sync", "imported_block_hashes_mutex lock failed");
        }
    }

    pub fn is_imported_block_hash(hash: &H256) -> bool {
        if let Ok(ref mut imported_block_hashes) = IMPORTED_BLOCK_HASHES.get().lock() {
            imported_block_hashes.contains_key(hash)
        } else {
            warn!(target: "sync", "imported_block_hashes_mutex lock failed");
            false
        }
    }

    pub fn get_downloaded_block_hashes() -> &'static Mutex<LruCache<H256, u8>> {
        return DOWNLOADED_BLOCK_HASHES.get();
    }

    pub fn clear_downloaded_block_hashes() {
        if let Ok(ref mut downloaded_block_hashes) = DOWNLOADED_BLOCK_HASHES.get().lock() {
            downloaded_block_hashes.clear();
        }
    }

    pub fn is_downloaded_block_hashes(hash: &H256) -> bool {
        if let Ok(ref mut downloaded_block_hashes) = DOWNLOADED_BLOCK_HASHES.get().lock() {
            downloaded_block_hashes.contains_key(hash)
        } else {
            warn!(target: "sync", "downloaded_block_hashes lock failed");
            false
        }
    }

    pub fn get_sent_transaction_hashes() -> &'static Mutex<LruCache<H256, u8>> {
        SENT_TRANSACTION_HASHES.get()
    }

    pub fn update_network_status(
        best_block_num: u64,
        best_hash: H256,
        target_total_difficulty: U256,
    )
    {
        if let Ok(mut network_status) = NETWORK_STATUS.get().write() {
            if target_total_difficulty > network_status.total_diff {
                network_status.best_block_num = best_block_num;
                network_status.best_hash = best_hash;
                network_status.total_diff = target_total_difficulty;
            }
        }
    }

    pub fn get_received_transactions() -> &'static Mutex<VecDeque<Vec<u8>>> {
        RECEIVED_TRANSACTIONS.get()
    }

    pub fn insert_received_transaction(transaction: Vec<u8>) {
        let mut lock = RECEIVED_TRANSACTIONS.get().lock();
        if let Ok(ref mut received_transactions) = lock {
            if received_transactions.len() <= MAX_RECEIVED_TRANSACTIONS_COUNT {
                received_transactions.push_back(transaction);
            }
        } else {
            warn!(target: "sync", "downloaded_headers_mutex lock failed");
        }
    }

    pub fn get_staged_blocks() -> &'static Mutex<LruCache<H256, Vec<Vec<u8>>>> {
        STAGED_BLOCKS.get()
    }

    pub fn insert_staged_block_hashes(hashes: Vec<H256>) {
        if let Ok(mut staged_block_hashes) = STAGED_BLOCK_HASHES.get().lock() {
            for hash in hashes.iter() {
                staged_block_hashes.insert(*hash, 0);
            }
        }
    }

    pub fn is_staged_block_hash(hash: H256) -> bool {
        if let Ok(mut staged_block_hashes) = STAGED_BLOCK_HASHES.get().lock() {
            return staged_block_hashes.contains_key(&hash);
        }
        return false;
    }

    //    pub fn remove_staged_block_hash(hash: H256) {
    //        if let Ok(mut staged_block_hashes) = STAGED_BLOCK_HASHES.get().lock() {
    //            staged_block_hashes.remove(&hash);
    //        }
    //    }

    pub fn clear_staged_blocks() {
        if let Ok(mut staged_blocks) = STAGED_BLOCKS.get().lock() {
            staged_blocks.clear();
        }

        if let Ok(mut staged_block_hashes) = STAGED_BLOCK_HASHES.get().lock() {
            staged_block_hashes.clear();
        }
    }

    pub fn reset() {
        SYNC_EXECUTORS.get().write().expect("get_executor").inner = None;
        BLOCK_CHAIN.get().write().expect("get_block_chain").inner = None;
    }
}

#[derive(Clone, Copy)]
pub struct SyncStatus {
    /// State
    pub state: SyncState,
    /// Syncing protocol version. That's the maximum protocol version we connect to.
    pub protocol_version: u8,
    /// The underlying p2p network version.
    pub network_id: u32,
    /// `BlockChain` height for the moment the sync started.
    pub start_block_number: u64,
    /// Last fully downloaded and imported block number (if any).
    pub last_imported_block_number: Option<u64>,
    /// Highest block number in the download queue (if any).
    pub highest_block_number: Option<u64>,
    /// Total number of blocks for the sync process.
    pub blocks_total: u64,
    /// Number of blocks downloaded so far.
    pub blocks_received: u64,
    /// Total number of connected peers
    pub num_peers: usize,
    /// Total number of active peers.
    pub num_active_peers: usize,
}

impl SyncStatus {
    pub fn is_syncing(&self, queue_info: BlockQueueInfo) -> bool {
        let is_syncing_state = match self.state {
            SyncState::Idle | SyncState::NewBlocks => false,
            _ => true,
        };
        let is_verifying = queue_info.unverified_queue_size + queue_info.verified_queue_size > 3;
        is_verifying || is_syncing_state
    }
}

pub struct PeerInfo {
    pub id: Option<String>,
}
pub struct TransactionStats {
    pub first_seen: u64,
}

pub struct ActivePeerInfo {
    /// Best block number
    pub highest_block_number: u64,
    /// node id
    pub id: String,
    /// remote p2p ip
    pub ip: String,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SyncState {
    WaitingPeers,
    Blocks,
    Idle,
    Waiting,
    NewBlocks,
}

#[derive(Clone)]
pub struct LocalStatus {
    pub synced_block_number: u64,
    pub synced_block_number_last_time: u64,
    pub sync_speed: u16,
    pub max_staged_block_number: u64,
}

impl LocalStatus {
    pub fn new() -> Self {
        LocalStatus {
            synced_block_number: 0,
            synced_block_number_last_time: 0,
            sync_speed: 48,
            max_staged_block_number: 0,
        }
    }
}

impl fmt::Display for LocalStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "LocalStatus: \n"));
        try!(write!(
            f,
            "    synced block number: {}\n",
            self.synced_block_number
        ));
        try!(write!(
            f,
            "    synced block number last time: {}\n",
            self.synced_block_number_last_time
        ));
        try!(write!(
            f,
            "    max staged block number: {}\n",
            self.max_staged_block_number
        ));
        write!(f, "\n")
    }
}

#[derive(Clone)]
pub struct NetworkStatus {
    pub total_diff: U256,
    pub best_block_num: u64,
    pub best_hash: H256,
}

impl NetworkStatus {
    pub fn new() -> Self {
        NetworkStatus {
            total_diff: U256::default(),
            best_block_num: 0,
            best_hash: H256::default(),
        }
    }
}

impl fmt::Display for NetworkStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "NetworkStatus: \n"));
        try!(write!(f, "    total difficuty: {}\n", self.total_diff));
        try!(write!(
            f,
            "    best block number: {}\n",
            self.best_block_num
        ));
        try!(write!(f, "    best hash: "));
        for item in self.best_hash.iter() {
            try!(write!(f, "{:02X}", item));
        }
        write!(f, "\n")
    }
}

#[derive(Clone, PartialEq)]
pub struct BlocksWrapper {
    pub node_id_hash: u64,
    pub blocks: Vec<Block>,
}

impl BlocksWrapper {
    pub fn new() -> Self {
        BlocksWrapper {
            node_id_hash: 0,
            blocks: Vec::new(),
        }
    }
}
