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

use std::cmp::max;

const MIN_BC_CACHE_MB: u32 = 4;
const MIN_DB_CACHE_MB: u32 = 8;
const MIN_BLOCK_QUEUE_SIZE_LIMIT_MB: u32 = 16;
//const DEFAULT_DB_CACHE_SIZE: u32 = 128;
const DEFAULT_BC_CACHE_SIZE: u32 = 8;
const DEFAULT_BLOCK_QUEUE_SIZE_LIMIT_MB: u32 = 40;
const DEFAULT_STATE_CACHE_SIZE: u32 = 25;

/// Configuration for application cache sizes.
/// All    values are represented in MB.
#[derive(Debug, PartialEq)]
pub struct CacheConfig {
    //    /// Size of rocksDB cache. Almost all goes to the state column.
    //    db: u32,
    /// Size of blockchain cache.
    blockchain: u32,
    /// Size of transaction queue cache.
    queue: u32,
    /// Size of the state cache.
    state: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        CacheConfig::new(
            //            DEFAULT_DB_CACHE_SIZE,
            DEFAULT_BC_CACHE_SIZE,
            DEFAULT_BLOCK_QUEUE_SIZE_LIMIT_MB,
            DEFAULT_STATE_CACHE_SIZE,
        )
    }
}

impl CacheConfig {
    /// Creates new cache config with cumulative size equal `total`.
    pub fn new_with_total_cache_size(total: u32) -> Self {
        CacheConfig {
            //            db: total * 7 / 10,
            blockchain: total / 3,
            queue: DEFAULT_BLOCK_QUEUE_SIZE_LIMIT_MB,
            state: total * 2 / 3,
        }
    }

    /// Creates new cache config with gitven details.
    pub fn new(/*db: u32, */ blockchain: u32, queue: u32, state: u32) -> Self {
        CacheConfig {
            //            db: db,
            blockchain: blockchain,
            queue: queue,
            state: state,
        }
    }

    /// Size of db cache.
    pub fn db_cache_size(&self) -> u32 {
        /*max(*/
        MIN_DB_CACHE_MB /*, self.db) */
    }

    /// Size of block queue size limit
    pub fn queue(&self) -> u32 { max(self.queue, MIN_BLOCK_QUEUE_SIZE_LIMIT_MB) }

    /// Size of the blockchain cache.
    pub fn blockchain(&self) -> u32 { max(self.blockchain, MIN_BC_CACHE_MB) }

    /// Size of the state cache.
    pub fn state(&self) -> u32 { self.state * 3 / 4 }

    /// Size of the jump-tables cache.
    pub fn jump_tables(&self) -> u32 { self.state / 4 }
}

#[cfg(test)]
mod tests {
    use super::CacheConfig;

    #[test]
    fn test_cache_config_constructor() {
        let config = CacheConfig::new_with_total_cache_size(60 /*200*/);
        //        assert_eq!(config.db, 140);
        assert_eq!(config.blockchain(), 20);
        assert_eq!(config.queue(), 40);
        assert_eq!(config.state(), 30);
        assert_eq!(config.jump_tables(), 10);
    }
    /*

        #[test]
        fn test_cache_config_db_cache_sizes() {
            let config = CacheConfig::new_with_total_cache_size(400);
            assert_eq!(config.db, 280);
            assert_eq!(config.db_cache_size(), 280);
        }
    */

    #[test]
    fn test_cache_config_default() {
        assert_eq!(
            CacheConfig::default(),
            CacheConfig::new(
                //                super::DEFAULT_DB_CACHE_SIZE,
                super::DEFAULT_BC_CACHE_SIZE,
                super::DEFAULT_BLOCK_QUEUE_SIZE_LIMIT_MB,
                super::DEFAULT_STATE_CACHE_SIZE
            )
        );
    }
}
