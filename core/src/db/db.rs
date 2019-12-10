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

//! Database utilities and definitions.

use std::ops::Deref;
use std::hash::Hash;
use std::collections::HashMap;
use parking_lot::RwLock;
use kvdb::{DBTransaction, KeyValueDB};

use rlp;

// database columns
/// Column for State
pub const COL_STATE: &'static str = "state";
/// Column for Block headers
pub const COL_HEADERS: &'static str = "headers";
/// Column for Block bodies
pub const COL_BODIES: &'static str = "bodies";
/// Column for Extras
pub const COL_EXTRA: &'static str = "extra";
/// Column for the empty accounts bloom filter.
pub const COL_ACCOUNT_BLOOM: &'static str = "account_bloom";
/// Column for general information from the local node which can persist.
// pub const COL_NODE_INFO: &'static str = "node_info";
/// Column for avm object graph
pub const COL_AVM_GRAPH: &'static str = "avm_graph";

pub const DB_NAMES: [&'static str; 7] = [
    "headers",
    "bodies",
    "state",
    "extra",
    "account_bloom",
    "node_info",
    "avm_graph",
];
/// Modes for updating caches.
#[derive(Clone, Copy)]
pub enum CacheUpdatePolicy {
    /// Overwrite entries.
    Overwrite,
    /// Remove entries.
    Remove,
}

/// A cache for arbitrary key-value pairs.
pub trait Cache<K, V> {
    /// Insert an entry into the cache and get the old value.
    fn insert(&mut self, k: K, v: V) -> Option<V>;

    /// Remove an entry from the cache, getting the old value if it existed.
    fn remove(&mut self, k: &K) -> Option<V>;

    /// Query the cache for a key's associated value.
    fn get(&self, k: &K) -> Option<&V>;
}

impl<K, V> Cache<K, V> for HashMap<K, V>
where K: Hash + Eq
{
    fn insert(&mut self, k: K, v: V) -> Option<V> { HashMap::insert(self, k, v) }

    fn remove(&mut self, k: &K) -> Option<V> { HashMap::remove(self, k) }

    fn get(&self, k: &K) -> Option<&V> { HashMap::get(self, k) }
}

/// Should be used to get database key associated with given value.
pub trait Key<T> {
    /// The db key associated with this value.
    type Target: Deref<Target = [u8]>;

    /// Returns db key.
    fn key(&self) -> Self::Target;
}

/// Should be used to write value into database.
pub trait Writable {
    /// Writes the value into the database.
    fn write<T, R>(&mut self, db_name: &'static str, key: &dyn Key<T, Target = R>, value: &T)
    where
        T: rlp::Encodable,
        R: Deref<Target = [u8]>;

    /// Deletes key from the databse.
    fn delete<T, R>(&mut self, db_name: &'static str, key: &dyn Key<T, Target = R>)
    where
        T: rlp::Encodable,
        R: Deref<Target = [u8]>;

    /// Writes the value into the database and updates the cache.
    fn write_with_cache<K, T, R>(
        &mut self,
        db_name: &'static str,
        cache: &mut dyn Cache<K, T>,
        key: K,
        value: T,
        policy: CacheUpdatePolicy,
    ) where
        K: Key<T, Target = R> + Hash + Eq,
        T: rlp::Encodable,
        R: Deref<Target = [u8]>,
    {
        self.write(db_name, &key, &value);
        match policy {
            CacheUpdatePolicy::Overwrite => {
                cache.insert(key, value);
            }
            CacheUpdatePolicy::Remove => {
                cache.remove(&key);
            }
        }
    }

    /// Writes the values into the database and updates the cache.
    fn extend_with_cache<K, T, R>(
        &mut self,
        db_name: &'static str,
        cache: &mut dyn Cache<K, T>,
        values: HashMap<K, T>,
        policy: CacheUpdatePolicy,
    ) where
        K: Key<T, Target = R> + Hash + Eq,
        T: rlp::Encodable,
        R: Deref<Target = [u8]>,
    {
        match policy {
            CacheUpdatePolicy::Overwrite => {
                for (key, value) in values {
                    self.write(db_name, &key, &value);
                    cache.insert(key, value);
                }
            }
            CacheUpdatePolicy::Remove => {
                for (key, value) in &values {
                    self.write(db_name, key, value);
                    cache.remove(key);
                }
            }
        }
    }

    /// Writes and removes the values into the database and updates the cache.
    fn extend_with_option_cache<K, T, R>(
        &mut self,
        db_name: &'static str,
        cache: &mut dyn Cache<K, Option<T>>,
        values: HashMap<K, Option<T>>,
        policy: CacheUpdatePolicy,
    ) where
        K: Key<T, Target = R> + Hash + Eq,
        T: rlp::Encodable,
        R: Deref<Target = [u8]>,
    {
        match policy {
            CacheUpdatePolicy::Overwrite => {
                for (key, value) in values {
                    match value {
                        Some(ref v) => self.write(db_name, &key, v),
                        None => self.delete(db_name, &key),
                    }
                    cache.insert(key, value);
                }
            }
            CacheUpdatePolicy::Remove => {
                for (key, value) in values {
                    match value {
                        Some(v) => self.write(db_name, &key, &v),
                        None => self.delete(db_name, &key),
                    }
                    cache.remove(&key);
                }
            }
        }
    }
}

/// Should be used to read values from database.
pub trait Readable {
    /// Returns value for given key.
    fn read<T, R>(&self, db_name: &'static str, key: &dyn Key<T, Target = R>) -> Option<T>
    where
        T: rlp::Decodable,
        R: Deref<Target = [u8]>;

    /// Returns value for given key either in cache or in database.
    fn read_with_cache<K, T, C>(
        &self,
        db_name: &'static str,
        cache: &RwLock<C>,
        key: &K,
    ) -> Option<T>
    where
        K: Key<T> + Eq + Hash + Clone,
        T: Clone + rlp::Decodable,
        C: Cache<K, T>,
    {
        {
            let read = cache.read();
            if let Some(v) = read.get(key) {
                return Some(v.clone());
            }
        }

        self.read(db_name, key).map(|value: T| {
            let mut write = cache.write();
            write.insert(key.clone(), value.clone());
            value
        })
    }

    /// Returns true if given value exists.
    fn exists<T, R>(&self, db_name: &'static str, key: &dyn Key<T, Target = R>) -> bool
    where R: Deref<Target = [u8]>;

    /// Returns true if given value exists either in cache or in database.
    fn exists_with_cache<K, T, R, C>(
        &self,
        db_name: &'static str,
        cache: &RwLock<C>,
        key: &K,
    ) -> bool
    where
        K: Eq + Hash + Key<T, Target = R>,
        R: Deref<Target = [u8]>,
        C: Cache<K, T>,
    {
        {
            let read = cache.read();
            if read.get(key).is_some() {
                return true;
            }
        }

        self.exists::<T, R>(db_name, key)
    }
}

impl Writable for DBTransaction {
    fn write<T, R>(&mut self, db_name: &'static str, key: &dyn Key<T, Target = R>, value: &T)
    where
        T: rlp::Encodable,
        R: Deref<Target = [u8]>,
    {
        self.put(db_name, &key.key(), &rlp::encode(value));
    }

    fn delete<T, R>(&mut self, db_name: &'static str, key: &dyn Key<T, Target = R>)
    where
        T: rlp::Encodable,
        R: Deref<Target = [u8]>,
    {
        self.delete(db_name, &key.key());
    }
}

impl<KVDB: KeyValueDB + ?Sized> Readable for KVDB {
    fn read<T, R>(&self, db_name: &'static str, key: &dyn Key<T, Target = R>) -> Option<T>
    where
        T: rlp::Decodable,
        R: Deref<Target = [u8]>,
    {
        let result = self.get(db_name, &key.key());

        match result {
            Ok(option) => option.map(|v| rlp::decode(&v)),
            Err(err) => {
                panic!(
                    "db get failed, key: {:?}, err: {:?}",
                    &key.key() as &[u8],
                    err
                );
            }
        }
    }

    fn exists<T, R>(&self, db_name: &'static str, key: &dyn Key<T, Target = R>) -> bool
    where R: Deref<Target = [u8]> {
        let result = self.get(db_name, &key.key());

        match result {
            Ok(v) => v.is_some(),
            Err(err) => {
                panic!(
                    "db get failed, key: {:?}, err: {:?}",
                    &key.key() as &[u8],
                    err
                );
            }
        }
    }
}
