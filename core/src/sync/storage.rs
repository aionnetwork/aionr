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
use std::sync::Mutex;

use lru_cache::LruCache;

use aion_types::H256;
use sync::wrappers::{HeadersWrapper, BlocksWrapper};

// const MAX_DOWNLOADED_HEADERS_COUNT: usize = 4096;
const MAX_CACHED_BLOCK_HASHES: usize = 32;
// const MAX_CACHED_TRANSACTION_HASHES: usize = 20480;
// const MAX_RECEIVED_TRANSACTIONS_COUNT: usize = 20480;

pub struct SyncStorage {
    /// Downloaded headers wrappers
    downloaded_headers: Mutex<VecDeque<HeadersWrapper>>,

    /// Downloaded blocks wrappers
    downloaded_blocks: Mutex<VecDeque<BlocksWrapper>>,

    /// Downloaded blocks hashes
    downloaded_blocks_hashes: Mutex<LruCache<H256, u8>>,

    /// Imported blocks hashes
    imported_blocks_hashes: Mutex<LruCache<H256, u8>>,

    /// Bodies request record
    headers_with_bodies_requested: Mutex<HashMap<u64, HeadersWrapper>>,
}

impl SyncStorage {
    pub fn new() -> Self {
        SyncStorage {
            downloaded_headers: Mutex::new(VecDeque::new()),
            downloaded_blocks: Mutex::new(VecDeque::new()),
            downloaded_blocks_hashes: Mutex::new(LruCache::new(MAX_CACHED_BLOCK_HASHES * 2)),
            imported_blocks_hashes: Mutex::new(LruCache::new(MAX_CACHED_BLOCK_HASHES)),
            headers_with_bodies_requested: Mutex::new(HashMap::new()),
        }
    }

    pub fn downloaded_headers(&self) -> &Mutex<VecDeque<HeadersWrapper>> {
        &self.downloaded_headers
    }

    pub fn _downloaded_blocks(&self) -> &Mutex<VecDeque<BlocksWrapper>> { &self.downloaded_blocks }

    pub fn insert_downloaded_blocks(&self, blocks_wrapper: BlocksWrapper) {
        if let Ok(mut downloaded_blocks) = self.downloaded_blocks.lock() {
            downloaded_blocks.push_back(blocks_wrapper);
        } else {
            warn!(target: "sync", "downloaded_blocks lock failed");
        }
    }

    pub fn downloaded_blocks_hashes(&self) -> &Mutex<LruCache<H256, u8>> {
        &self.downloaded_blocks_hashes
    }

    pub fn is_block_hash_downloaded(&self, hash: &H256) -> bool {
        if let Ok(mut downloaded_blocks_hashes) = self.downloaded_blocks_hashes.lock() {
            downloaded_blocks_hashes.contains_key(hash)
        } else {
            warn!(target: "sync", "downloaded_block_hashes lock failed");
            false
        }
    }

    pub fn is_block_hash_imported(&self, hash: &H256) -> bool {
        if let Ok(mut imported_blocks_hashes) = self.imported_blocks_hashes.lock() {
            imported_blocks_hashes.contains_key(hash)
        } else {
            warn!(target: "sync", "imported_block_hashes lock failed");
            false
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
        if let Ok(mut headers_with_bodies_requested) = self.headers_with_bodies_requested.lock() {
            headers_with_bodies_requested.remove(node_hash)
        } else {
            warn!(target: "sync", "headers_with_bodies_requested mutex lock failed");
            None
        }
    }
}
