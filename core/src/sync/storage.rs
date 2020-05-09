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
use std::collections::{VecDeque, HashMap};

use lru_cache::LruCache;
use parking_lot::{Mutex, RwLock};

use aion_types::H256;
use sync::wrappers::{HeadersWrapper, BlocksWrapper};

// const MAX_DOWNLOADED_HEADERS_COUNT: usize = 4096;
const MAX_CACHED_BLOCK_HASHES: usize = 32;
const MAX_CACHED_TRANSACTION_HASHES: usize = 20480;
const MAX_RECEIVED_TRANSACTIONS_COUNT: usize = 20480;

pub struct SyncStorage {
    /// Downloaded headers wrappers
    downloaded_headers: Mutex<VecDeque<HeadersWrapper>>,

    /// Downloaded blocks wrappers
    downloaded_blocks: Mutex<VecDeque<BlocksWrapper>>,

    /// Recorded blocks hashes, including downloaded blocks hashes from syncing and broadcasting
    /// and sealed blocks hashes
    recorded_blocks_hashes: Mutex<LruCache<H256, u8>>,

    /// Bodies request record
    headers_with_bodies_requested: Mutex<HashMap<u64, HeadersWrapper>>,

    /// Staged blocks to be imported later
    staged_blocks: Mutex<HashMap<(H256, u64), Vec<Vec<u8>>>>,

    /// Recorded tx hashes
    recorded_transaction_hashes: Mutex<LruCache<H256, u8>>,

    /// Received txs
    received_transactions: Mutex<VecDeque<Vec<u8>>>,

    // Lightning sync block height
    lightning_base: RwLock<u64>,
}

impl SyncStorage {
    pub fn new() -> Self {
        SyncStorage {
            downloaded_headers: Mutex::new(VecDeque::new()),
            downloaded_blocks: Mutex::new(VecDeque::new()),
            recorded_blocks_hashes: Mutex::new(LruCache::new(MAX_CACHED_BLOCK_HASHES * 40)),
            headers_with_bodies_requested: Mutex::new(HashMap::new()),
            staged_blocks: Mutex::new(HashMap::new()),
            recorded_transaction_hashes: Mutex::new(LruCache::new(MAX_CACHED_TRANSACTION_HASHES)),
            received_transactions: Mutex::new(VecDeque::new()),
            lightning_base: RwLock::new(0u64),
        }
    }

    pub fn downloaded_headers(&self) -> &Mutex<VecDeque<HeadersWrapper>> {
        &self.downloaded_headers
    }

    pub fn downloaded_blocks(&self) -> &Mutex<VecDeque<BlocksWrapper>> { &self.downloaded_blocks }

    pub fn insert_downloaded_blocks(&self, blocks_wrapper: BlocksWrapper) {
        let mut downloaded_blocks = self.downloaded_blocks.lock();
        downloaded_blocks.push_back(blocks_wrapper);
    }

    pub fn recorded_blocks_hashes(&self) -> &Mutex<LruCache<H256, u8>> {
        &self.recorded_blocks_hashes
    }

    pub fn is_block_hash_recorded(&self, hash: &H256) -> bool {
        let mut recorded_blocks_hashes = self.recorded_blocks_hashes.lock();
        recorded_blocks_hashes.contains_key(hash)
    }

    pub fn remove_recorded_blocks_hashes(&self, hashes: &Vec<H256>) {
        let mut recorded_blocks_hashes = self.recorded_blocks_hashes.lock();
        for hash in hashes {
            recorded_blocks_hashes.remove(hash);
        }
    }

    pub fn recorded_blocks_hashes_statics(&self) -> (usize, usize) {
        let recorded_blocks_hashes = self.recorded_blocks_hashes.lock();
        (
            recorded_blocks_hashes.len(),
            recorded_blocks_hashes.capacity(),
        )
    }

    pub fn insert_recorded_blocks_hashes(&self, hashes: Vec<H256>) {
        let mut recorded_blocks_hashes = self.recorded_blocks_hashes.lock();
        for hash in hashes {
            if !recorded_blocks_hashes.contains_key(&hash) {
                recorded_blocks_hashes.insert(hash, 0);
            }
        }
    }

    pub fn headers_with_bodies_requested(&self) -> &Mutex<HashMap<u64, HeadersWrapper>> {
        &self.headers_with_bodies_requested
    }

    pub fn headers_with_bodies_requested_for_node(
        &self,
        node_hash: &u64,
    ) -> Option<HeadersWrapper>
    {
        let mut headers_with_bodies_requested = self.headers_with_bodies_requested.lock();
        headers_with_bodies_requested.remove(node_hash)
    }

    pub fn staged_blocks(&self) -> &Mutex<HashMap<(H256, u64), Vec<Vec<u8>>>> {
        &self.staged_blocks
    }

    pub fn stage_blocks(&self, parent_hash: H256, parent_num: u64, blocks: Vec<Vec<u8>>) -> bool {
        let mut staged_blocks = self.staged_blocks.lock();
        staged_blocks.insert((parent_hash, parent_num), blocks);
        staged_blocks.len() <= MAX_CACHED_BLOCK_HASHES * 4
    }

    pub fn staged_blocks_statics(&self) -> (usize, usize) {
        let staged_blocks = self.staged_blocks.lock();
        (staged_blocks.len(), staged_blocks.capacity())
    }

    pub fn staged_is_full(&self) -> bool {
        let staged_blocks = self.staged_blocks.lock();
        staged_blocks.len() > MAX_CACHED_BLOCK_HASHES * 4
    }

    pub fn recorded_transaction_hashes(&self) -> &Mutex<LruCache<H256, u8>> {
        &self.recorded_transaction_hashes
    }

    pub fn received_transactions(&self) -> &Mutex<VecDeque<Vec<u8>>> { &self.received_transactions }

    pub fn insert_received_transaction(&self, transaction: Vec<u8>) {
        let mut received_transactions = self.received_transactions.lock();
        if received_transactions.len() <= MAX_RECEIVED_TRANSACTIONS_COUNT {
            received_transactions.push_back(transaction);
        }
    }

    pub fn get_lightning_base_lock(&self) -> &RwLock<u64> { &self.lightning_base }

    pub fn lightning_base(&self) -> u64 { *self.lightning_base.read() }

    // pub fn set_lightning_base(&self, base: u64) { *self.lightning_base.write() = base; }
}
