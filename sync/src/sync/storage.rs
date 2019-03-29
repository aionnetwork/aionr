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

use acore::block::Block;
use acore::client::{
    BlockChainClient, BlockChainInfo, BlockQueueInfo, Client, header_chain::HeaderChain
};
use acore::header::Header as BlockHeader;
use aion_types::{H256, U256};
use lru_cache::LruCache;
use parking_lot::{Mutex, RwLock};
use rlp::RlpStream;
use state::Storage;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::runtime::{Builder, Runtime, TaskExecutor};

lazy_static! {
    static ref BLOCK_CHAIN: Storage<RwLock<BlockChain>> = Storage::new();
    static ref BLOCK_HEADER_CHAIN: Storage<RwLock<BlockHeaderChain>> = Storage::new();
    static ref SYNC_EXECUTORS: Storage<RwLock<SyncExecutor>> = Storage::new();
    static ref LOCAL_STATUS: RwLock<LocalStatus> = RwLock::new(LocalStatus::new());
    static ref NETWORK_STATUS: RwLock<NetworkStatus> = RwLock::new(NetworkStatus::new());
    static ref HEADERS_WITH_BODIES_REQUESTED: Storage<Mutex<HashMap<u64, Vec<H256>>>> =
        Storage::new();
    static ref REQUESTED_BLOCK_HASHES: Storage<Mutex<LruCache<H256, SystemTime>>> = Storage::new();
    static ref SENT_TRANSACTION_HASHES: Storage<Mutex<LruCache<H256, u8>>> = Storage::new();
    static ref RECEIVED_TRANSACTIONS: Storage<Mutex<VecDeque<Vec<u8>>>> = Storage::new();
    static ref STAGED_BLOCKS: Storage<Mutex<LruCache<H256, Vec<RlpStream>>>> = Storage::new();
}

const MAX_CACHED_TRANSACTION_HASHES: usize = 20480;
const MAX_RECEIVED_TRANSACTIONS_COUNT: usize = 20480;

pub const MAX_CACHED_BLOCKS: usize = 1024;
pub const MAX_CACHED_BLOCK_HASHES: usize = 8192;

#[derive(Clone)]
struct BlockChain {
    inner: Option<Arc<Client>>,
}

#[derive(Clone)]
struct BlockHeaderChain {
    inner: Option<Arc<HeaderChain>>,
}

#[derive(Clone)]
struct SyncExecutor {
    inner: Option<Arc<Runtime>>,
}

pub struct SyncStorage;

impl SyncStorage {
    pub fn init(client: Arc<Client>, header_chain: Arc<HeaderChain>) {
        if let Some(_) = BLOCK_CHAIN.try_get() {
            if let Some(mut block_chain) = BLOCK_CHAIN.get().try_write() {
                if let Some(_) = block_chain.inner {
                } else {
                    block_chain.inner = Some(client);
                }
            }
        } else {
            let block_chain = BlockChain {
                inner: Some(client),
            };

            let block_header_chain = BlockHeaderChain {
                inner: Some(header_chain),
            };

            let sync_executor = SyncExecutor {
                inner: Some(Arc::new(
                    Builder::new()
                        .core_threads(10)
                        .name_prefix("SYNC-Task")
                        .build()
                        .expect("SYNC_RUNTIME error."),
                )),
            };

            let mut requested_block_hashes = LruCache::new(MAX_CACHED_BLOCK_HASHES);
            let mut sent_transaction_hases = LruCache::new(MAX_CACHED_TRANSACTION_HASHES);
            let mut received_transactions = VecDeque::new();
            let mut staged_blocks = LruCache::new(MAX_CACHED_BLOCK_HASHES);

            HEADERS_WITH_BODIES_REQUESTED.set(Mutex::new(HashMap::new()));
            REQUESTED_BLOCK_HASHES.set(Mutex::new(requested_block_hashes));
            SENT_TRANSACTION_HASHES.set(Mutex::new(sent_transaction_hases));
            RECEIVED_TRANSACTIONS.set(Mutex::new(received_transactions));
            STAGED_BLOCKS.set(Mutex::new(staged_blocks));
            BLOCK_CHAIN.set(RwLock::new(block_chain));
            BLOCK_HEADER_CHAIN.set(RwLock::new(block_header_chain));
            SYNC_EXECUTORS.set(RwLock::new(sync_executor));
        }
    }

    pub fn get_block_chain() -> Arc<Client> {
        BLOCK_CHAIN
            .get()
            .try_read()
            .expect("get_block_chain error")
            .clone()
            .inner
            .expect("get_client error")
    }

    pub fn get_chain_info() -> BlockChainInfo {
        let client = BLOCK_CHAIN
            .get()
            .try_read()
            .expect("get_block_chain error")
            .clone()
            .inner
            .expect("get_chain_info error");
        client.chain_info()
    }

    pub fn get_block_header_chain() -> Arc<HeaderChain> {
        BLOCK_HEADER_CHAIN
            .get()
            .try_read()
            .expect("get_block_header_chain error")
            .clone()
            .inner
            .expect("get_block_header_chain error")
    }

    pub fn get_sync_executor() -> TaskExecutor {
        let rt = SYNC_EXECUTORS
            .get()
            .try_read()
            .expect("get_executor error")
            .clone()
            .inner
            .expect("get_executor error");
        rt.executor()
    }

    pub fn set_total_difficulty(total_difficulty: U256) {
        let mut local_status = LOCAL_STATUS.write();
        local_status.total_difficulty = total_difficulty;
    }

    pub fn get_total_difficulty() -> U256 {
        let local_status = LOCAL_STATUS.read();
        return local_status.total_difficulty;
    }

    pub fn set_synced_block_number(synced_block_number: u64) {
        let mut local_status = LOCAL_STATUS.write();
        local_status.synced_block_number = synced_block_number;
    }

    pub fn get_synced_block_number() -> u64 {
        let local_status = LOCAL_STATUS.read();
        return local_status.synced_block_number;
    }

    pub fn set_synced_block_number_last_time(synced_block_number_last_time: u64) {
        let mut local_status = LOCAL_STATUS.write();
        local_status.synced_block_number_last_time = synced_block_number_last_time;
    }

    pub fn get_synced_block_number_last_time() -> u64 {
        let local_status = LOCAL_STATUS.read();
        return local_status.synced_block_number_last_time;
    }

    pub fn set_requested_block_number_last_time(requested_block_number_last_time: u64) {
        let mut local_status = LOCAL_STATUS.write();
        local_status.requested_block_number_last_time = requested_block_number_last_time;
    }

    pub fn get_requested_block_number_last_time() -> u64 {
        let local_status = LOCAL_STATUS.read();
        return local_status.requested_block_number_last_time;
    }

    pub fn set_sync_speed(sync_speed: u16) {
        let mut local_status = LOCAL_STATUS.write();
        local_status.sync_speed = sync_speed;
    }

    pub fn get_sync_speed() -> u16 {
        let local_status = LOCAL_STATUS.read();
        return local_status.sync_speed;
    }

    pub fn set_max_staged_block_number(max_staged_block_number: u64) {
        let mut local_status = LOCAL_STATUS.write();
        local_status.max_staged_block_number = max_staged_block_number;
    }

    pub fn get_max_staged_block_number() -> u64 {
        let local_status = LOCAL_STATUS.read();
        return local_status.max_staged_block_number;
    }

    pub fn set_is_syncing(is_syncing: bool) {
        let mut local_status = LOCAL_STATUS.write();
        local_status.is_syncing = is_syncing;
    }

    pub fn is_syncing() -> bool {
        let local_status = LOCAL_STATUS.read();
        return local_status.is_syncing;
    }

    pub fn get_headers_with_bodies_requested() -> &'static Mutex<HashMap<u64, Vec<H256>>> {
        HEADERS_WITH_BODIES_REQUESTED.get()
    }

    pub fn pick_headers_with_bodies_requested(node_hash: &u64) -> Option<Vec<H256>> {
        let mut headers_with_bodies_requested = HEADERS_WITH_BODIES_REQUESTED.get().lock();
        {
            headers_with_bodies_requested.remove(node_hash)
        }
    }

    pub fn clear_headers_with_bodies_requested() {
        let mut headers_with_bodies_requested = HEADERS_WITH_BODIES_REQUESTED.get().lock();
        {
            headers_with_bodies_requested.clear();
        }
    }

    pub fn get_network_best_block_number() -> u64 {
        let network_status = NETWORK_STATUS.read();
        network_status.best_block_num
    }

    pub fn get_network_best_block_hash() -> H256 {
        let network_status = NETWORK_STATUS.read();
        return network_status.best_hash;
    }

    pub fn get_network_total_diff() -> U256 {
        let network_status = NETWORK_STATUS.read();
        return network_status.total_diff;
    }

    pub fn insert_requested_time(hash: H256) {
        if let Some(ref mut requested_block_hashes) = REQUESTED_BLOCK_HASHES.get().try_lock() {
            if !requested_block_hashes.contains_key(&hash) {
                requested_block_hashes.insert(hash, SystemTime::now());
            }
        }
    }

    pub fn get_requested_time(hash: &H256) -> Option<SystemTime> {
        if let Some(ref mut requested_block_hashes) = REQUESTED_BLOCK_HASHES.get().try_lock() {
            if let Some(time) = requested_block_hashes.get_mut(hash) {
                return Some(time.clone());
            }
        }
        None
    }

    pub fn clear_requested_blocks() {
        if let Some(ref mut requested_block_hashes) = REQUESTED_BLOCK_HASHES.get().try_lock() {
            requested_block_hashes.clear();
        }
    }

    pub fn get_sent_transaction_hashes() -> &'static Mutex<LruCache<H256, u8>> {
        SENT_TRANSACTION_HASHES.get()
    }

    pub fn update_network_status(
        best_block_num: u64,
        best_hash: H256,
        target_total_difficulty: U256,
    ) {
        let mut network_status = NETWORK_STATUS.write();
        if target_total_difficulty > network_status.total_diff {
            network_status.best_block_num = best_block_num;
            network_status.best_hash = best_hash;
            network_status.total_diff = target_total_difficulty;
        }
    }

    pub fn get_received_transactions() -> &'static Mutex<VecDeque<Vec<u8>>> {
        RECEIVED_TRANSACTIONS.get()
    }

    pub fn get_received_transactions_count() -> usize {
        if let Some(received_transactions) = RECEIVED_TRANSACTIONS.get().try_lock() {
            return received_transactions.len();
        } else {
            0
        }
    }

    pub fn insert_received_transaction(transaction: Vec<u8>) {
        let mut lock = RECEIVED_TRANSACTIONS.get().try_lock();
        if let Some(ref mut received_transactions) = lock {
            if received_transactions.len() <= MAX_RECEIVED_TRANSACTIONS_COUNT {
                received_transactions.push_back(transaction);
            }
        } else {
            warn!(target: "sync", "downloaded_headers_mutex lock failed");
        }
    }

    pub fn get_staged_blocks() -> &'static Mutex<LruCache<H256, Vec<RlpStream>>> {
        STAGED_BLOCKS.get()
    }

    pub fn get_staged_blocks_with_hash(hash: H256) -> Option<Vec<RlpStream>> {
        if let Some(mut staged_block_hashes) = STAGED_BLOCKS.get().try_lock() {
            return staged_block_hashes.remove(&hash);
        }
        None
    }

    pub fn clear_staged_blocks() {
        if let Some(mut staged_blocks) = STAGED_BLOCKS.get().try_lock() {
            staged_blocks.clear();
        }
    }

    pub fn reset() {
        SYNC_EXECUTORS
            .get()
            .try_write()
            .expect("get_executor error")
            .inner = None;
        BLOCK_CHAIN
            .get()
            .try_write()
            .expect("get_block_chain error")
            .inner = None;
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
    pub total_difficulty: U256,
    pub synced_block_number: u64,
    pub synced_block_number_last_time: u64,
    pub requested_block_number_last_time: u64,
    pub sync_speed: u16,
    pub max_staged_block_number: u64,
    pub is_syncing: bool,
}

impl LocalStatus {
    pub fn new() -> Self {
        LocalStatus {
            total_difficulty: U256::default(),
            synced_block_number: 0,
            synced_block_number_last_time: 0,
            requested_block_number_last_time: 0,
            sync_speed: 48,
            max_staged_block_number: 0,
            is_syncing: false,
        }
    }
}

impl fmt::Display for LocalStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "LocalStatus: \n"));
        try!(write!(
            f,
            "    total difficulty: {}\n",
            self.total_difficulty
        ));
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
            "    requested block number last time: {}\n",
            self.requested_block_number_last_time
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
pub struct HeadersWrapper {
    pub node_hash: u64,
    pub timestamp: SystemTime,
    pub headers: Vec<BlockHeader>,
}

impl HeadersWrapper {
    pub fn new() -> Self {
        HeadersWrapper {
            node_hash: 0,
            timestamp: SystemTime::now(),
            headers: Vec::new(),
        }
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
