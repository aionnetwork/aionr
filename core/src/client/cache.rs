/*******************************************************************************
 * Copyright (c) 2015-2018 Parity Technologies (UK) Ltd.
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

//! Cache for data fetched from the network.

use std::time::Duration;

use encoded;
use types::BlockNumber;
use types::receipt::Receipt;
use aion_types::{H256, U256};
use heapsize::HeapSizeOf;
use memory_cache::MemoryLruCache;

/// Configuration for how much data to cache.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CacheSizes {
    /// Maximum size, in bytes, of cached headers.
    pub headers: usize,
    /// Maximum size, in bytes, of cached canonical hashes.
    pub canon_hashes: usize,
    /// Maximum size, in bytes, of cached block bodies.
    pub bodies: usize,
    /// Maximum size, in bytes, of cached block receipts.
    pub receipts: usize,
    /// Maximum size, in bytes, of cached chain score for the block.
    pub chain_score: usize,
}

impl Default for CacheSizes {
    fn default() -> Self {
        const MB: usize = 1024 * 1024;
        CacheSizes {
            headers: 500 * MB,
            canon_hashes: 30 * MB,
            bodies: 200 * MB,
            receipts: 100 * MB,
            chain_score: 70 * MB,
        }
    }
}

/// The light client data cache.
///
/// Note that almost all getter methods take `&mut self` due to the necessity to update
/// the underlying LRU-caches on read.
/// [LRU-cache](https://en.wikipedia.org/wiki/Cache_replacement_policies#Least_Recently_Used_.28LRU.29)
pub struct Cache {
    headers: MemoryLruCache<H256, encoded::Header>,
    canon_hashes: MemoryLruCache<BlockNumber, H256>,
    bodies: MemoryLruCache<H256, encoded::Body>,
    receipts: MemoryLruCache<H256, Vec<Receipt>>,
    chain_score: MemoryLruCache<H256, U256>,
}

impl Cache {
    /// Create a new data cache with the given sizes and gas price corpus expiration time.
    pub fn new(sizes: CacheSizes, _corpus_expiration: Duration) -> Self {
        Cache {
            headers: MemoryLruCache::new(sizes.headers),
            canon_hashes: MemoryLruCache::new(sizes.canon_hashes),
            bodies: MemoryLruCache::new(sizes.bodies),
            receipts: MemoryLruCache::new(sizes.receipts),
            chain_score: MemoryLruCache::new(sizes.chain_score),
        }
    }

    /// Query header by hash.
    pub fn block_header(&mut self, hash: &H256) -> Option<encoded::Header> {
        self.headers.get_mut(hash).cloned()
    }

    /// Cache the given header.
    pub fn insert_block_header(&mut self, hash: H256, hdr: encoded::Header) {
        self.headers.insert(hash, hdr);
    }
}

impl HeapSizeOf for Cache {
    fn heap_size_of_children(&self) -> usize {
        self.headers.current_size()
            + self.canon_hashes.current_size()
            + self.bodies.current_size()
            + self.receipts.current_size()
            + self.chain_score.current_size()
        // TODO: + corpus
    }
}

#[cfg(test)]
mod tests {
    use super::Cache;
    use std::time::Duration;

    #[test]
    fn corpus_inaccessible() {
        let duration = Duration::from_secs(20);
        let mut cache = Cache::new(Default::default(), duration.clone());

        cache.set_gas_price_corpus(vec![].into());
        assert_eq!(cache.gas_price_corpus(), Some(vec![].into()));

        {
            let corpus_time = &mut cache.corpus.as_mut().unwrap().1;
            *corpus_time = *corpus_time - duration;
        }
        assert!(cache.gas_price_corpus().is_none());
    }
}
