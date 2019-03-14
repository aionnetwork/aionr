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
use aion_types::H256;
use heapsize::HeapSizeOf;
use memory_cache::MemoryLruCache;

/// Configuration for how much data to cache.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CacheSizes {
    /// Maximum size, in bytes, of cached headers.
    pub headers: usize,
}

impl Default for CacheSizes {
    fn default() -> Self {
        const MB: usize = 1024 * 1024;
        CacheSizes {
            headers: 100 * MB,
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
}

impl Cache {
    /// Create a new data cache with the given sizes and gas price corpus expiration time.
    pub fn new(sizes: CacheSizes, _corpus_expiration: Duration) -> Self {
        Cache {
            headers: MemoryLruCache::new(sizes.headers),
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
        // TODO: + corpus
    }
}
