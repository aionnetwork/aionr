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
use acore::client::{header_chain::HeaderChain, BlockChainClient, BlockChainInfo, BlockQueueInfo};
use acore::header::Header as BlockHeader;
use acore::spec::Spec;
use aion_types::{H256, U256};
use kvdb::{DBTransaction, DatabaseConfig, DbRepository, KeyValueDB, RepositoryConfig};
use lru_cache::LruCache;
use state::Storage;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};
use tokio::runtime::{Runtime, TaskExecutor};

lazy_static! {
    static ref BLOCK_CHAIN: Storage<RwLock<BlockChain>> = Storage::new();
    static ref BLOCK_HEADER_CHAIN: Storage<RwLock<BlockHeaderChain>> = Storage::new();
    static ref SYNC_EXECUTORS: Storage<RwLock<SyncExecutor>> = Storage::new();
    static ref LOCAL_STATUS: Storage<RwLock<LocalStatus>> = Storage::new();
    static ref NETWORK_STATUS: Storage<RwLock<NetworkStatus>> = Storage::new();
    static ref DOWNLOADED_BLOCKS: Storage<Mutex<LruCache<u64, BlockWrapper>>> = Storage::new();
    static ref DOWNLOADED_BLOCK_HASHES: Storage<Mutex<LruCache<H256, u8>>> = Storage::new();
    static ref SENT_TRANSACTION_HASHES: Storage<Mutex<LruCache<H256, u8>>> = Storage::new();
    static ref HEADERS_WITH_BODIES_REQUESTED: Storage<Mutex<HashMap<u64, HeadersWrapper>>> =
        Storage::new();
    static ref LIGHT_CLIENT: Storage<RwLock<LightClient>> = Storage::new();
}

pub const MAX_CACHED_BLOCKS: usize = 1024;
pub const MAX_CACHED_BLOCK_HASHED: usize = 8192;

#[derive(Clone)]
struct BlockChain {
    inner: Option<Arc<BlockChainClient>>,
}

#[derive(Clone)]
struct SyncExecutor {
    inner: Option<Arc<Runtime>>,
}

#[derive(Clone)]
struct BlockHeaderChain {
    inner: Option<Arc<HeaderChain>>,
}

pub struct SyncStorage;

impl SyncStorage {
    pub fn init(client: Arc<BlockChainClient>, spec: Spec, db: Arc<KeyValueDB>) {
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
            let block_chain = BlockChain {
                inner: Some(client),
            };

            let sync_executor = SyncExecutor {
                inner: Some(Arc::new(Runtime::new().expect("Tokio Runtime"))),
            };

            let header_chain = HeaderChain::new(db, &spec).unwrap();

            let block_header_chain = BlockHeaderChain {
                inner: Some(Arc::new(header_chain)),
            };

            let mut local_status = LocalStatus::new();
            let network_status = NetworkStatus::new();
            let downloaded_blocks = LruCache::new(MAX_CACHED_BLOCKS);
            let downloaded_block_hashes = LruCache::new(MAX_CACHED_BLOCK_HASHED);

            local_status.synced_block_number = 0;
            local_status.synced_block_number_last_time = 0;
            LOCAL_STATUS.set(RwLock::new(local_status));
            NETWORK_STATUS.set(RwLock::new(network_status));
            DOWNLOADED_BLOCKS.set(Mutex::new(downloaded_blocks));
            DOWNLOADED_BLOCK_HASHES.set(Mutex::new(downloaded_block_hashes));
            HEADERS_WITH_BODIES_REQUESTED.set(Mutex::new(HashMap::new()));

            BLOCK_CHAIN.set(RwLock::new(block_chain));
            SYNC_EXECUTORS.set(RwLock::new(sync_executor));
            BLOCK_HEADER_CHAIN.set(RwLock::new(block_header_chain));
            LIGHT_CLIENT.set(RwLock::new(LightClient::new("./data")));
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

    pub fn get_block_header_chain() -> Arc<HeaderChain> {
        BLOCK_HEADER_CHAIN
            .get()
            .read()
            .expect("get_block_header_chain")
            .clone()
            .inner
            .expect("get_block_header_chain")
    }

    pub fn set_starting_block_number(starting_block_number: u64) {
        if let Ok(mut local_status) = LOCAL_STATUS.get().write() {
            local_status.starting_block_number = starting_block_number;
        }
    }

    pub fn get_starting_block_number() -> u64 {
        if let Ok(local_status) = LOCAL_STATUS.get().read() {
            return local_status.starting_block_number;
        }
        0
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

    pub fn set_local_status(status: LocalStatus) {
        if let Ok(mut local_status) = LOCAL_STATUS.get().write() {
            local_status.genesis_hash = status.genesis_hash;
            local_status.synced_block_hash = status.synced_block_hash;
            local_status.synced_block_number = status.synced_block_number;
            local_status.total_difficulty = status.total_difficulty;
        }
    }

    pub fn get_local_status() -> LocalStatus {
        if let Ok(local_status) = LOCAL_STATUS.get().read() {
            return local_status.clone();
        }
        LocalStatus::new()
    }

    pub fn get_downloaded_blocks() -> &'static Mutex<LruCache<u64, BlockWrapper>> {
        DOWNLOADED_BLOCKS.get()
    }

    pub fn is_downloaded_block(block_number: u64, hash: H256) -> bool {
        if let Ok(ref mut downloaded_blocks) = DOWNLOADED_BLOCKS.get().lock() {
            if let Some(bw) = downloaded_blocks.get_mut(&block_number) {
                for h in bw.block_hashes.iter() {
                    if h == &hash {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn get_downloaded_blocks_count() -> usize {
        if let Ok(downloaded_blocks) = DOWNLOADED_BLOCKS.get().lock() {
            return downloaded_blocks.len();
        } else {
            0
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

    pub fn get_downloaded_block_hashes() -> &'static Mutex<LruCache<H256, u8>> {
        return DOWNLOADED_BLOCK_HASHES.get();
    }

    pub fn clear_downloaded_block_hashes() {
        if let Ok(ref mut downloaded_block_hashes) = DOWNLOADED_BLOCK_HASHES.get().lock() {
            downloaded_block_hashes.clear();
        }
    }

    pub fn is_block_hash_confirmed(hash: H256, is_increase: bool) -> bool {
        if let Ok(mut downloaded_block_hashes) = DOWNLOADED_BLOCK_HASHES.get().lock() {
            if downloaded_block_hashes.contains_key(&hash) {
                let increase = if is_increase { 1 } else { 0 };
                if let Some(count) = downloaded_block_hashes.get_mut(&hash) {
                    if *count >= 2 {
                        return true;
                    } else {
                        *count += increase;
                    }
                }
            } else {
                downloaded_block_hashes.insert(hash, 1);
            }
        }

        false
    }

    pub fn get_sent_transaction_hashes() -> &'static Mutex<LruCache<H256, u8>> {
        SENT_TRANSACTION_HASHES.get()
    }

    pub fn update_network_status(
        best_block_num: u64,
        best_hash: H256,
        target_total_difficulty: U256,
    ) {
        if let Ok(mut network_status) = NETWORK_STATUS.get().write() {
            if target_total_difficulty > network_status.total_diff {
                network_status.best_block_num = best_block_num;
                network_status.best_hash = best_hash;
                network_status.total_diff = target_total_difficulty;
            }
        }
    }

    pub fn get_light_client() -> &'static RwLock<LightClient> {
        LIGHT_CLIENT.get()
    }

    pub fn get_best_block_header(genesis_hash: &H256) -> Option<Vec<u8>> {
        if let Ok(light_client) = LIGHT_CLIENT.get().read() {
            if let Some(ref best_block_hash) = light_client.get_best_block_hash(genesis_hash) {
                error!(target: "sync", "get_best_block_header: {}", genesis_hash);
                return light_client.find_header(best_block_hash);
            } else {
                error!(target: "sync", "No best block header found, genesis hash: {}", genesis_hash);
            }
        }
        None
    }

    pub fn get_chain_info() -> BlockChainInfo {
        let local_status = SyncStorage::get_local_status();
        BlockChainInfo {
            total_difficulty: local_status.total_difficulty,
            pending_total_difficulty: local_status.total_difficulty,
            //genesis_hash: H256::from("0x30793b4ea012c6d3a58c85c5b049962669369807a98e36807c1b02116417f823"),
            genesis_hash: local_status.genesis_hash,
            best_block_hash: local_status.synced_block_hash,
            best_block_number: local_status.synced_block_number,
            best_block_timestamp: 0,
            ancient_block_hash: None,
            ancient_block_number: None,
            first_block_hash: None,
            first_block_number: None,
        }
    }

    pub fn insert_headers_with_bodies_requested(hw: HeadersWrapper) -> bool {
        if let Ok(ref mut headers_with_bodies_requested) =
            HEADERS_WITH_BODIES_REQUESTED.get().lock()
        {
            if !headers_with_bodies_requested.contains_key(&hw.node_hash) {
                headers_with_bodies_requested.insert(hw.node_hash, hw);
                return true;
            }
        }

        false
    }

    pub fn pick_headers_with_bodies_requested(node_hash: &u64) -> Option<HeadersWrapper> {
        if let Ok(ref mut headers_with_bodies_requested) =
            HEADERS_WITH_BODIES_REQUESTED.get().lock()
        {
            headers_with_bodies_requested.remove(node_hash)
        } else {
            warn!(target: "sync", "headers_with_bodies_requested_mutex lock failed");
            None
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
    pub synced_block_hash: H256,
    pub total_difficulty: U256,
    pub genesis_hash: H256,
    pub starting_block_number: u64,
}

impl LocalStatus {
    pub fn new() -> Self {
        LocalStatus {
            synced_block_number: 0,
            synced_block_number_last_time: 0,
            sync_speed: 48,
            synced_block_hash: H256::from(0),
            total_difficulty: U256::from(0),
            genesis_hash: H256::from(0),
            starting_block_number: 1,
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
            "    synced block hash: {:?}\n",
            self.synced_block_hash
        ));
        try!(write!(f, "    genesis hash: {:?}\n", self.genesis_hash));
        try!(write!(
            f,
            "    starting block number: {}\n",
            self.starting_block_number
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
    pub hashes: Vec<H256>,
    pub headers: Vec<BlockHeader>,
}

impl HeadersWrapper {
    pub fn new() -> Self {
        HeadersWrapper {
            node_hash: 0,
            hashes: Vec::new(),
            headers: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct BlockWrapper {
    pub block_number: u64,
    pub parent_hash: H256,
    pub block_hashes: Vec<H256>,
    pub block_headers: Option<Vec<BlockHeader>>,
}

impl BlockWrapper {
    pub fn new() -> Self {
        BlockWrapper {
            block_number: 0,
            parent_hash: H256::new(),
            block_hashes: Vec::new(),
            block_headers: None,
        }
    }
}

const HEADERS_DB: &'static str = "headers";
const BLOCKS_DB: &'static str = "blocks";
const BEST_BLOCK_DB: &'static str = "best_block";

pub struct LightClient {
    db: Arc<KeyValueDB>,
}

impl LightClient {
    pub fn new(path: &str) -> LightClient {
        let db_configs = vec![
            Self::generate_db_configs(HEADERS_DB, path),
            Self::generate_db_configs(BLOCKS_DB, path),
            Self::generate_db_configs(BEST_BLOCK_DB, path),
        ];
        let db = DbRepository::init(db_configs).unwrap();
        LightClient { db: Arc::new(db) }
    }

    pub fn get_db(&self) -> Arc<KeyValueDB> {
        self.db.clone()
    }

    fn generate_db_configs(name: &str, path: &str) -> RepositoryConfig {
        RepositoryConfig {
            db_name: name.into(),
            db_config: DatabaseConfig::default(),
            db_path: format!("{}/{}", path, name),
        }
    }

    fn find(&self, db_name: &'static str, key: &H256) -> Option<Vec<u8>> {
        let reader = self.db.clone();

        match reader.get(db_name.into(), key) {
            Ok(data) => {
                if let Some(data_bytes) = data {
                    return Some(data_bytes.into_vec());
                }
            }
            Err(e) => {
                error!(target: "sync", "Error: {}", e);
            }
        }

        None
    }

    pub fn save(&mut self, db_name: &'static str, keyes: Vec<H256>, data: Vec<&[u8]>) {
        let writer = self.db.clone();

        let mut db_tx = DBTransaction::new();
        let keyes_size = keyes.len();
        if keyes_size > 0 && keyes_size == data.len() {
            let mut keyes_iterator = keyes.iter();
            let mut data_iterator = data.iter();
            for _ in 0..keyes_size {
                let k = keyes_iterator.next().expect("Invalid inuut key");
                let d = data_iterator.next().expect("Invalid inuut data");
                db_tx.put(db_name.into(), k, d);
            }
            if let Err(e) = writer.write(db_tx) {
                error!(target: "sync", "failed to save into {}, {}", db_name, e);
            }
        }
    }

    pub fn find_header(&self, hash: &H256) -> Option<Vec<u8>> {
        self.find(HEADERS_DB, hash)
    }

    pub fn save_headers(&mut self, hashes: Vec<H256>, headers: Vec<&[u8]>) {
        self.save(HEADERS_DB, hashes, headers);
    }

    pub fn find_block(&self, hash: &H256) -> Option<Vec<u8>> {
        self.find(BLOCKS_DB, hash)
    }

    pub fn save_blocks(&mut self, hashes: Vec<H256>, blocks: Vec<&[u8]>) {
        self.save(BLOCKS_DB, hashes, blocks);
    }

    pub fn get_best_block_hash(&self, genesis_hash: &H256) -> Option<H256> {
        if let Some(best_block_hash) = self.find(BEST_BLOCK_DB, genesis_hash) {
            return Some(H256::from(best_block_hash.as_slice()));
        }
        None
    }

    pub fn set_best_block_hash(&mut self, genesis_hash: H256, best_block_hash: H256) {
        self.save(BEST_BLOCK_DB, vec![genesis_hash], vec![&best_block_hash]);
    }
}

#[test]
fn test_light_client() {
    let mut light_client = LightClient::new("./data");

    let hash1 = H256::from("0x30793b4ea012c6d3a58c85c5b049962669369807a98e36807c1b02116417f823");
    let hash2 = H256::from("0x60793b4ea012c6d3a58c85c5b049962669369807a98e36807c1b02116417f826");
    let hashes = vec![hash1, hash2];

    println!("1 header 1: {:?}", light_client.find_header(&hash1));
    println!("1 header 2: {:?}", light_client.find_header(&hash2));
    println!("1 block 1: {:?}", light_client.find_block(&hash1));
    println!("1 block 2: {:?}", light_client.find_block(&hash2));
    println!(
        "1 best_block_hash: {:?}",
        light_client.get_best_block_hash(&hash1)
    );

    let header1 = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
    let header2 = vec![0, 9, 8, 7, 6, 5, 4, 3, 2, 1];
    let headers = vec![header1.as_slice(), header2.as_slice()];

    light_client.save_headers(hashes.clone(), headers);
    let found_header1 = light_client.find_header(&hash1).unwrap();
    let found_header2 = light_client.find_header(&hash2).unwrap();
    assert_eq!(found_header1, header1);
    assert_eq!(found_header2, header2);

    let mut block1 = Vec::new();
    let mut block2 = Vec::new();
    let mut body;
    body = format!("{:?}", ::std::time::SystemTime::now()).into_bytes();
    block1.extend(header1.clone());
    block1.extend(body);
    body = format!("{:?}", ::std::time::SystemTime::now()).into_bytes();
    block2.extend(header2.clone());
    block2.extend(body);
    let blocks = vec![block1.as_slice(), block2.as_slice()];

    light_client.save_blocks(hashes.clone(), blocks);
    let found_block1 = light_client.find_block(&hash1).unwrap();
    let found_block2 = light_client.find_block(&hash2).unwrap();

    assert_eq!(found_block1, block1);
    assert_eq!(found_block2, block2);

    light_client.set_best_block_hash(hash1, hash2);
    let best_block_hash = light_client.get_best_block_hash(&hash1).unwrap();
    assert_eq!(best_block_hash, hash2);

    println!("2 header 1: {:?}", light_client.find_header(&hash1));
    println!("2 header 2: {:?}", light_client.find_header(&hash2));
    println!("3 block 1: {:?}", light_client.find_block(&hash1));
    println!("3 block 2: {:?}", light_client.find_block(&hash2));
    println!(
        "2 best_block_hash: {:?}",
        light_client.get_best_block_hash(&hash1)
    );
}
