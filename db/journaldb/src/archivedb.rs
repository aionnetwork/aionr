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

//! Disk-backed `HashStore` implementation.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use rlp::*;
use super::{DB_PREFIX_LEN, LATEST_ERA_KEY};
use traits::JournalDB;
use kvdb::{KeyValueDB, DBTransaction, HashStore, DBValue, MemoryDB};
use aion_types::H256;
use error::{BaseDataError, UtilError};
use bytes::Bytes;

/// Implementation of the `HashStore` trait for a disk-backed database with a memory overlay
/// and latent-removal semantics.
///
/// Like `OverlayDB`, there is a memory overlay; `commit()` must be called in order to
/// write operations out to disk. Unlike `OverlayDB`, `remove()` operations do not take effect
/// immediately. As this is an "archive" database, nothing is ever removed. This means
/// that the states of any block the node has ever processed will be accessible.
pub struct ArchiveDB {
    overlay: MemoryDB,
    backing: Arc<KeyValueDB>,
    latest_era: Option<u64>,
    db_name: &'static str,
}

impl ArchiveDB {
    /// Create a new instance from a key-value db.
    pub fn new(backing: Arc<KeyValueDB>, db_name: &'static str) -> ArchiveDB {
        let latest_era = backing
            .get(db_name, &LATEST_ERA_KEY)
            .expect("Low-level database error.")
            .map(|val| decode::<u64>(&val));
        ArchiveDB {
            overlay: MemoryDB::new(),
            backing: backing,
            latest_era: latest_era,
            db_name: db_name,
        }
    }

    fn payload(&self, key: &H256) -> Option<DBValue> {
        self.backing
            .get(self.db_name, key)
            .expect("Low-level database error. Some issue with your hard disk?")
    }
}

impl HashStore for ArchiveDB {
    fn keys(&self) -> HashMap<H256, i32> {
        let mut ret: HashMap<H256, i32> = self
            .backing
            .iter(self.db_name)
            .map(|(key, _)| (H256::from_slice(&*key), 1))
            .collect();

        for (key, refs) in self.overlay.keys() {
            match ret.entry(key) {
                Entry::Occupied(mut entry) => {
                    *entry.get_mut() += refs;
                }
                Entry::Vacant(entry) => {
                    entry.insert(refs);
                }
            }
        }
        ret
    }

    fn get(&self, key: &H256) -> Option<DBValue> {
        if let Some((d, rc)) = self.overlay.raw(key) {
            if rc > 0 {
                return Some(d);
            }
        }
        self.payload(key)
    }

    fn contains(&self, key: &H256) -> bool { self.get(key).is_some() }

    fn insert(&mut self, value: &[u8]) -> H256 { self.overlay.insert(value) }

    fn emplace(&mut self, key: H256, value: DBValue) { self.overlay.emplace(key, value); }

    fn remove(&mut self, key: &H256) { self.overlay.remove(key); }
}

impl JournalDB for ArchiveDB {
    fn boxed_clone(&self) -> Box<JournalDB> {
        Box::new(ArchiveDB {
            overlay: self.overlay.clone(),
            backing: self.backing.clone(),
            latest_era: self.latest_era,
            db_name: self.db_name.clone(),
        })
    }

    fn mem_used(&self) -> usize { self.overlay.mem_used() }

    fn is_empty(&self) -> bool { self.latest_era.is_none() }

    fn journal_under(
        &mut self,
        batch: &mut DBTransaction,
        now: u64,
        _id: &H256,
    ) -> Result<u32, UtilError>
    {
        let mut inserts = 0usize;
        let mut deletes = 0usize;

        for i in self.overlay.drain() {
            let (key, (value, rc)) = i;
            if rc > 0 {
                batch.put(self.db_name, &key, &value);
                inserts += 1;
            }
            if rc < 0 {
                assert!(rc == -1);
                deletes += 1;
            }
        }

        if self.latest_era.map_or(true, |e| now > e) {
            batch.put(self.db_name, &LATEST_ERA_KEY, &encode(&now));
            self.latest_era = Some(now);
        }
        Ok((inserts + deletes) as u32)
    }

    fn mark_canonical(
        &mut self,
        _batch: &mut DBTransaction,
        _end_era: u64,
        _canon_id: &H256,
    ) -> Result<u32, UtilError>
    {
        // keep everything! it's an archive, after all.
        Ok(0)
    }

    fn inject(&mut self, batch: &mut DBTransaction) -> Result<u32, UtilError> {
        let mut inserts = 0usize;
        let mut deletes = 0usize;

        for i in self.overlay.drain() {
            let (key, (value, rc)) = i;
            if rc > 0 {
                if self
                    .backing
                    .get(self.db_name, &key)
                    .expect("state db not found")
                    .is_some()
                {
                    return Err(BaseDataError::AlreadyExists(key).into());
                }
                batch.put(self.db_name, &key, &value);
                inserts += 1;
            }
            if rc < 0 {
                assert!(rc == -1);
                if self
                    .backing
                    .get(self.db_name, &key)
                    .expect("state db not found")
                    .is_none()
                {
                    return Err(BaseDataError::NegativelyReferencedHash(key).into());
                }
                batch.delete(self.db_name, &key);
                deletes += 1;
            }
        }

        Ok((inserts + deletes) as u32)
    }

    fn latest_era(&self) -> Option<u64> { self.latest_era }

    fn state(&self, id: &H256) -> Option<Bytes> {
        self.backing
            .get_by_prefix(self.db_name, &id[0..DB_PREFIX_LEN])
            .map(|b| b.into_vec())
    }

    fn is_pruned(&self) -> bool { false }

    fn backing(&self) -> &Arc<KeyValueDB> { &self.backing }

    fn consolidate(&mut self, with: MemoryDB) { self.overlay.consolidate(with); }
}

#[cfg(test)]
mod tests {}
