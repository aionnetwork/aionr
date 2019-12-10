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

//! Reference-counted memory-based `HashStore` implementation.
use std::mem;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use heapsize::HeapSizeOf;
use aion_types::H256;
use super::{HashStore, DBValue};
use blake2b::{BLAKE2B_NULL_RLP, blake2b};
use plain_hasher::H256FastMap;
use rlp::NULL_RLP;

#[derive(Default, Clone, PartialEq)]
pub struct MemoryDB {
    data: H256FastMap<(DBValue, i32)>,
}

impl MemoryDB {
    /// Create a new instance of the memory DB.
    pub fn new() -> MemoryDB {
        MemoryDB {
            data: H256FastMap::default(),
        }
    }

    pub fn get(&self, k: &[u8]) -> Option<DBValue> {
        match self.data.get(&H256::from(k)).cloned() {
            Some(value) => Some(value.0),
            _ => None,
        }
    }

    pub fn delete(&mut self, _k: &[u8]) -> Option<DBValue> { unimplemented!() }

    pub fn put(&mut self, _k: &[u8], _v: &DBValue) -> Option<DBValue> { unimplemented!() }

    pub fn iter(&self) -> Box<dyn Iterator<Item = (Box<[u8]>, Box<[u8]>)>> { unimplemented!() }

    pub fn get_by_prefix(&self, _prefix: &[u8]) -> Option<Box<[u8]>> { unimplemented!() }

    pub fn iter_from_prefix(&self, _prefix: &[u8]) -> Box<dyn Iterator<Item = (Box<[u8]>, Box<[u8]>)>> {
        unimplemented!()
    }

    pub fn clear(&mut self) { self.data.clear(); }

    /// Purge all zero-referenced data from the database.
    pub fn purge(&mut self) { self.data.retain(|_, &mut (_, rc)| rc != 0); }

    /// Return the internal map of hashes to data, clearing the current state.
    pub fn drain(&mut self) -> H256FastMap<(DBValue, i32)> {
        mem::replace(&mut self.data, H256FastMap::default())
    }

    /// Grab the raw information associated with a key. Returns None if the key
    /// doesn't exist.
    ///
    /// Even when Some is returned, the data is only guaranteed to be useful
    /// when the refs > 0.
    pub fn raw(&self, key: &H256) -> Option<(DBValue, i32)> {
        if key == &BLAKE2B_NULL_RLP {
            return Some((DBValue::from_slice(&NULL_RLP), 1));
        }
        self.data.get(key).cloned()
    }

    /// Returns the size of allocated heap memory
    pub fn mem_used(&self) -> usize { self.data.heap_size_of_children() }

    /// Remove an element and delete it from storage if reference count reaches zero.
    /// If the value was purged, return the old value.
    pub fn remove_and_purge(&mut self, key: &H256) -> Option<DBValue> {
        if key == &BLAKE2B_NULL_RLP {
            return None;
        }
        match self.data.entry(key.clone()) {
            Entry::Occupied(mut entry) => {
                if entry.get().1 == 1 {
                    Some(entry.remove().0)
                } else {
                    entry.get_mut().1 -= 1;
                    None
                }
            }
            Entry::Vacant(entry) => {
                entry.insert((DBValue::new(), -1));
                None
            }
        }
    }

    /// Consolidate all the entries of `other` into `self`.
    pub fn consolidate(&mut self, mut other: Self) {
        for (key, (value, rc)) in other.drain() {
            match self.data.entry(key) {
                Entry::Occupied(mut entry) => {
                    if entry.get().1 < 0 {
                        entry.get_mut().0 = value;
                    }

                    entry.get_mut().1 += rc;
                }
                Entry::Vacant(entry) => {
                    entry.insert((value, rc));
                }
            }
        }
    }
}

impl HashStore for MemoryDB {
    fn get(&self, key: &H256) -> Option<DBValue> {
        if key == &BLAKE2B_NULL_RLP {
            return Some(DBValue::from_slice(&NULL_RLP));
        }

        match self.data.get(key) {
            Some(&(ref d, rc)) if rc > 0 => Some(d.clone()),
            _ => None,
        }
    }

    fn keys(&self) -> HashMap<H256, i32> {
        self.data
            .iter()
            .filter_map(|(k, v)| if v.1 != 0 { Some((*k, v.1)) } else { None })
            .collect()
    }

    fn contains(&self, key: &H256) -> bool {
        if key == &BLAKE2B_NULL_RLP {
            return true;
        }

        match self.data.get(key) {
            Some(&(_, x)) if x > 0 => true,
            _ => false,
        }
    }

    fn insert(&mut self, value: &[u8]) -> H256 {
        if value == &NULL_RLP {
            return BLAKE2B_NULL_RLP.clone();
        }
        let key = blake2b(value);
        match self.data.entry(key) {
            Entry::Occupied(mut entry) => {
                let &mut (ref mut old_value, ref mut rc) = entry.get_mut();
                if *rc >= -0x80000000i32 && *rc <= 0 {
                    *old_value = DBValue::from_slice(value);
                }
                *rc += 1;
            }
            Entry::Vacant(entry) => {
                entry.insert((DBValue::from_slice(value), 1));
            }
        }
        key
    }

    fn emplace(&mut self, key: H256, value: DBValue) {
        if &*value == &NULL_RLP {
            return;
        }

        match self.data.entry(key) {
            Entry::Occupied(mut entry) => {
                let &mut (ref mut old_value, ref mut rc) = entry.get_mut();
                if *rc >= -0x80000000i32 && *rc <= 0 {
                    *old_value = value;
                }
                *rc += 1;
            }
            Entry::Vacant(entry) => {
                entry.insert((value, 1));
            }
        }
    }

    fn remove(&mut self, key: &H256) {
        if key == &BLAKE2B_NULL_RLP {
            return;
        }

        match self.data.entry(*key) {
            Entry::Occupied(mut entry) => {
                let &mut (_, ref mut rc) = entry.get_mut();
                *rc -= 1;
            }
            Entry::Vacant(entry) => {
                entry.insert((DBValue::new(), -1));
            }
        }
    }
}
