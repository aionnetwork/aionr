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

use std::cmp;
use traits::KeyValueDAO;
use parity_rocksdb::{
    DB, Options, BlockBasedOptions, Cache, ReadOptions, IteratorMode, Direction, WriteOptions, WriteBatch, DBIterator, Writable, DBCompactionStyle
};
use super::{Key, DBValue};
use std::collections::HashMap;
use interleaved_ordered::{interleave_ordered, InterleaveOrdered};
use std::marker::PhantomData;
use dbconfigs::DatabaseConfig;

enum KeyState {
    Insert(DBValue),
    Delete,
}

pub struct RockskvdbIterator<'a> {
    iter: InterleaveOrdered<::std::vec::IntoIter<(Box<[u8]>, Box<[u8]>)>, DBIterator>,
    _marker: PhantomData<&'a Rockskvdb>,
}

impl<'a> Iterator for RockskvdbIterator<'a> {
    type Item = (Box<[u8]>, Box<[u8]>);

    fn next(&mut self) -> Option<Self::Item> { self.iter.next() }
}

pub struct Rockskvdb {
    db: DB,
    write_options: WriteOptions,
    read_options: ReadOptions,
    block_cache_options: BlockBasedOptions,
    overlay: HashMap<Key, KeyState>,
}
impl Rockskvdb {
    /// Crate a new database file by default.
    pub fn new_default() -> Self {
        Rockskvdb {
            db: DB::open_default("./temp/testdb").expect("open default rocksdb failed"),
            write_options: WriteOptions::new(),
            read_options: ReadOptions::new(),
            block_cache_options: BlockBasedOptions::new(),
            overlay: HashMap::new(),
        }
    }

    /// Open database file. Creates if it does not exist.
    pub fn open(config: &DatabaseConfig, path: &str) -> Result<Self, String> {
        let mut block_opts = BlockBasedOptions::new();

        {
            block_opts.set_block_size(config.block_size);
            let cache_size = cmp::max(8, config.memory_budget);
            let cache = Cache::new(cache_size);
            block_opts.set_cache(cache);
        }

        let mut read_opts = ReadOptions::new();
        read_opts.set_verify_checksums(false);

        let mut write_opts = WriteOptions::new();
        if !config.wal {
            write_opts.disable_wal(true);
        }

        match Rockskvdb::parse_options(&config, &block_opts) {
            Ok(opts) => {
                match DB::open(&opts, path) {
                    Ok(t) => {
                        Ok(Rockskvdb {
                            db: t,
                            write_options: write_opts,
                            read_options: read_opts,
                            block_cache_options: block_opts,
                            overlay: HashMap::new(),
                        })
                    }
                    Err(ref s)
                        if s.starts_with("Corruption:") || s.starts_with(
                            "Invalid argument: You have to open all column families",
                        ) =>
                    {
                        warn!(target:"db","DB corrupted: {}, attempting repair", s);
                        DB::repair(&opts, path)?;

                        Ok(Rockskvdb {
                            db: DB::open(&opts, path)?,
                            write_options: write_opts,
                            read_options: read_opts,
                            block_cache_options: block_opts,
                            overlay: HashMap::new(),
                        })
                    }
                    Err(s) => return Err(s.into()),
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    pub fn flush(&mut self) -> Result<(), String> {
        let batch = WriteBatch::new();
        for (ref key, ref keystate) in self.overlay.drain() {
            match (key, keystate) {
                (key, KeyState::Delete) => {
                    batch.delete(&key)?;
                }
                (key, KeyState::Insert(ref value)) => {
                    batch.put(&key, &value)?;
                }
            }
        }
        self.db.write_opt(batch, &self.write_options)?;
        self.overlay.clear();
        Ok(())
    }
    fn parse_options(
        config: &DatabaseConfig,
        block_cache_config: &BlockBasedOptions,
    ) -> Result<Options, String>
    {
        let mut opts = Options::new();

        opts.create_if_missing(true);
        opts.set_use_fsync(config.use_fsync);
        opts.set_compaction_style(DBCompactionStyle::DBLevelCompaction);
        opts.set_bytes_per_sync(config.bytes_per_sync);
        opts.set_block_cache_size_mb(config.compact_options.block_size as u64);
        opts.set_target_file_size_base(config.compact_options.initial_file_size);
        opts.set_table_cache_num_shard_bits(config.table_cache_num_shard);
        opts.set_max_write_buffer_number(config.max_write_buffer_number);
        opts.set_write_buffer_size(config.write_buffer_size);
        opts.set_min_write_buffer_number_to_merge(config.min_write_buffer_number_to_merge);
        opts.set_level_zero_stop_writes_trigger(config.level_zero_stop_writes_trigger);
        opts.set_level_zero_slowdown_writes_trigger(config.level_zero_slowdown_writes_trigger);
        opts.set_max_background_compactions(config.max_background_compactions);
        opts.set_max_background_flushes(config.max_background_flushes);
        opts.set_disable_auto_compactions(config.disable_auto_compactions);
        opts.set_max_open_files(config.max_open_files);
        opts.increase_parallelism(cmp::max(1, ::num_cpus::get() as i32 / 2));
        opts.set_block_based_table_factory(block_cache_config);

        if let Some(rate_limit) = config.compact_options.write_rate_limit {
            opts.set_parsed_options(&format!("rate_limiter_bytes_per_sec={}", rate_limit))?;
        }

        opts.set_parsed_options(&format!(
            "block_based_table_factory={{{};{}}}",
            "cache_index_and_filter_blocks=true", "pin_l0_filter_and_index_blocks_in_cache=true"
        ))?;

        opts.optimize_level_style_compaction(config.memory_budget as i32);

        if config.disable_compress {
            opts.set_parsed_options(&format!(
                "compression_per_level={}:{}:{}:{}:{}:{}",
                "kSnappyCompression",
                "kSnappyCompression",
                "kSnappyCompression",
                "kSnappyCompression",
                "kSnappyCompression",
                "kSnappyCompression"
            ))?;
        } else {
            opts.set_parsed_options(&format!(
                "compression_per_level={}:{}:{}:{}:{}:{}",
                "kNoCompression",
                "kNoCompression",
                "kNoCompression",
                "kNoCompression",
                "kNoCompression",
                "kNoCompression"
            ))?;
        }

        Ok(opts)
    }
}

impl KeyValueDAO for Rockskvdb {
    fn get(&self, k: &[u8]) -> Option<DBValue> {
        match self.overlay.get(k) {
            Some(KeyState::Insert(ref value)) => Some(value.clone()),
            Some(KeyState::Delete) => None,
            None => {
                self.db
                    .get_opt(k, &self.read_options)
                    .unwrap_or(None)
                    .map(|r| DBValue::from_slice(&r))
            }
        }
    }

    fn put(&mut self, k: &[u8], v: &DBValue) -> Option<DBValue> {
        let mut ekey = Key::new();
        ekey.append_slice(k);
        self.overlay.insert(ekey, KeyState::Insert(v.clone()));
        if self.overlay.len() > 10000 {
            let _ = self.flush();
        }
        Some(v.clone())
    }

    fn delete(&mut self, k: &[u8]) -> Option<DBValue> {
        let mut ekey = Key::new();
        ekey.append_slice(k);
        self.overlay.insert(ekey, KeyState::Delete);
        if self.overlay.len() > 10000 {
            let _ = self.flush();
        }
        // ignore the result
        Some(DBValue::from_slice(k))
    }

    fn iter(&self) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)>> {
        let mut overlay_data = self
            .overlay
            .iter()
            .filter_map(|(k, v)| {
                match *v {
                    KeyState::Insert(ref value) => {
                        Some((
                            k.clone().to_vec().into_boxed_slice(),
                            value.clone().to_vec().into_boxed_slice(),
                        ))
                    }
                    KeyState::Delete => None,
                }
            })
            .collect::<Vec<_>>();
        overlay_data.sort();
        let db_iter = self
            .db
            .iterator_opt(IteratorMode::Start, &self.read_options);
        Box::new(RockskvdbIterator {
            iter: interleave_ordered(overlay_data, db_iter),
            _marker: PhantomData,
        })
    }

    fn get_by_prefix(&self, prefix: &[u8]) -> Option<Box<[u8]>> {
        let mut iter = self.db.iterator_opt(
            IteratorMode::From(prefix, Direction::Forward),
            &self.read_options,
        );
        match iter.next() {
            Some((k, v)) => {
                if k[0..prefix.len()] == prefix[..] {
                    Some(v)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn iter_from_prefix(&self, prefix: &[u8]) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)>> {
        Box::new(self.db.iterator_opt(
            IteratorMode::From(prefix, Direction::Forward),
            &self.read_options,
        ))
    }
}
impl Drop for Rockskvdb {
    fn drop(&mut self) { let _ = self.flush(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn crud_test() {
        {
            let mut db = Rockskvdb::new_default();

            let key1: Vec<u8> = vec![1];
            let value1: Vec<u8> = vec![1];
            let key2: Vec<u8> = vec![2];
            let value2: Vec<u8> = vec![2];
            let value3: Vec<u8> = vec![3];

            db.put(&key1, &DBValue::from_vec(value1.clone()));
            assert_eq!(db.get(&key1).unwrap(), value1);

            db.put(&key2, &DBValue::from_vec(value2.clone()));
            assert_eq!(db.get(&key2).unwrap(), value2);

            db.put(&key1, &DBValue::from_vec(value3.clone()));
            assert_eq!(db.get(&key1).unwrap(), value3);

            db.delete(&key1);
            db.delete(&key2);

            assert_eq!(db.get(&key1), None);
        }

        let _ = fs::remove_dir_all("./temp/testdb");
    }

    #[test]
    fn open_test() {
        {
            Rockskvdb::open(&DatabaseConfig::default(), "./temp/testdb_open").unwrap();
        }
        assert_eq!(
            Rockskvdb::open(&DatabaseConfig::default(), "./temp/testdb_open").is_ok(),
            true
        );
        let _ = fs::remove_dir_all("./temp/testdb_open");
    }
}
