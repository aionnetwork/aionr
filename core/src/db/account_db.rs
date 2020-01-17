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

//! DB backend wrapper for Account trie
use std::collections::HashMap;
use blake2b::{BLAKE2B_NULL_RLP, blake2b};
use aion_types::H256;
use kvdb::{DBValue, HashStore};
use rlp::NULL_RLP;

#[cfg(test)]
use aion_types::Address;

// combines a key with an address hash to ensure uniqueness.
// leaves the first 96 bits untouched in order to support partial key lookup.
#[inline]
fn combine_key<'a>(address_hash: &'a H256, key: &'a H256) -> H256 {
    let mut dst = key.clone();
    {
        let last_src: &[u8] = &*address_hash;
        let last_dst: &mut [u8] = &mut *dst;
        for (k, a) in last_dst[12..].iter_mut().zip(&last_src[12..]) {
            *k ^= *a
        }
    }

    dst
}

/// A factory for different kinds of account dbs.
#[derive(Debug, Clone)]
pub enum Factory {
    /// Mangle hashes based on address.
    Mangled,
    /// Don't mangle hashes.
    Plain,
}

impl Default for Factory {
    fn default() -> Self { Factory::Mangled }
}

impl Factory {
    /// Create a read-only accountdb.
    /// This will panic when write operations are called.
    pub fn readonly<'db>(
        &self,
        db: &'db dyn HashStore,
        address_hash: H256,
    ) -> Box<dyn HashStore + 'db>
    {
        match *self {
            Factory::Mangled => Box::new(AccountDB::from_hash(db, address_hash)),
            Factory::Plain => Box::new(Wrapping(db)),
        }
    }

    /// Create a new mutable hashdb.
    pub fn create<'db>(
        &self,
        db: &'db mut dyn HashStore,
        address_hash: H256,
    ) -> Box<dyn HashStore + 'db>
    {
        match *self {
            Factory::Mangled => Box::new(AccountDBMut::from_hash(db, address_hash)),
            Factory::Plain => Box::new(WrappingMut(db)),
        }
    }
}

// TODO: introduce HashStoreMut?
/// DB backend wrapper for Account trie
/// Transforms trie node keys for the database
pub struct AccountDB<'db> {
    db: &'db dyn HashStore,
    address_hash: H256,
}

impl<'db> AccountDB<'db> {
    /// Create a new AcountDB from an address' hash.
    pub fn from_hash(db: &'db dyn HashStore, address_hash: H256) -> Self {
        AccountDB {
            db: db,
            address_hash: address_hash,
        }
    }
}

impl<'db> HashStore for AccountDB<'db> {
    fn keys(&self) -> HashMap<H256, i32> { unimplemented!() }

    fn get(&self, key: &H256) -> Option<DBValue> {
        if key == &BLAKE2B_NULL_RLP {
            return Some(DBValue::from_slice(&NULL_RLP));
        }
        self.db.get(&combine_key(&self.address_hash, key))
    }

    fn contains(&self, key: &H256) -> bool {
        if key == &BLAKE2B_NULL_RLP {
            return true;
        }
        self.db.contains(&combine_key(&self.address_hash, key))
    }

    fn insert(&mut self, _value: &[u8]) -> H256 { unimplemented!() }

    fn emplace(&mut self, _key: H256, _value: DBValue) { unimplemented!() }

    fn remove(&mut self, _key: &H256) { unimplemented!() }
}

/// DB backend wrapper for Account trie
pub struct AccountDBMut<'db> {
    db: &'db mut dyn HashStore,
    address_hash: H256,
}

impl<'db> AccountDBMut<'db> {
    /// Create a new AccountDB from an address.
    #[cfg(test)]
    pub fn new(db: &'db mut dyn HashStore, address: &Address) -> Self {
        Self::from_hash(db, blake2b(address))
    }

    /// Create a new AcountDB from an address' hash.
    pub fn from_hash(db: &'db mut dyn HashStore, address_hash: H256) -> Self {
        AccountDBMut {
            db: db,
            address_hash: address_hash,
        }
    }

    #[cfg(test)]
    pub fn immutable(&'db self) -> AccountDB<'db> {
        AccountDB {
            db: self.db,
            address_hash: self.address_hash.clone(),
        }
    }
}

impl<'db> HashStore for AccountDBMut<'db> {
    fn keys(&self) -> HashMap<H256, i32> { unimplemented!() }

    fn get(&self, key: &H256) -> Option<DBValue> {
        if key == &BLAKE2B_NULL_RLP {
            return Some(DBValue::from_slice(&NULL_RLP));
        }
        self.db.get(&combine_key(&self.address_hash, key))
    }

    fn contains(&self, key: &H256) -> bool {
        if key == &BLAKE2B_NULL_RLP {
            return true;
        }
        self.db.contains(&combine_key(&self.address_hash, key))
    }

    fn insert(&mut self, value: &[u8]) -> H256 {
        if value == &NULL_RLP {
            return BLAKE2B_NULL_RLP.clone();
        }
        let k = blake2b(value);
        let ak = combine_key(&self.address_hash, &k);
        self.db.emplace(ak, DBValue::from_slice(value));
        k
    }

    fn emplace(&mut self, key: H256, value: DBValue) {
        if key == BLAKE2B_NULL_RLP {
            return;
        }
        let key = combine_key(&self.address_hash, &key);
        self.db.emplace(key, value)
    }

    fn remove(&mut self, key: &H256) {
        if key == &BLAKE2B_NULL_RLP {
            return;
        }
        let key = combine_key(&self.address_hash, key);
        self.db.remove(&key)
    }
}

struct Wrapping<'db>(&'db dyn HashStore);

impl<'db> HashStore for Wrapping<'db> {
    fn keys(&self) -> HashMap<H256, i32> { unimplemented!() }

    fn get(&self, key: &H256) -> Option<DBValue> {
        if key == &BLAKE2B_NULL_RLP {
            return Some(DBValue::from_slice(&NULL_RLP));
        }
        self.0.get(key)
    }

    fn contains(&self, key: &H256) -> bool {
        if key == &BLAKE2B_NULL_RLP {
            return true;
        }
        self.0.contains(key)
    }

    fn insert(&mut self, _value: &[u8]) -> H256 { unimplemented!() }

    fn emplace(&mut self, _key: H256, _value: DBValue) { unimplemented!() }

    fn remove(&mut self, _key: &H256) { unimplemented!() }
}

struct WrappingMut<'db>(&'db mut dyn HashStore);

impl<'db> HashStore for WrappingMut<'db> {
    fn keys(&self) -> HashMap<H256, i32> { unimplemented!() }

    fn get(&self, key: &H256) -> Option<DBValue> {
        if key == &BLAKE2B_NULL_RLP {
            return Some(DBValue::from_slice(&NULL_RLP));
        }
        self.0.get(key)
    }

    fn contains(&self, key: &H256) -> bool {
        if key == &BLAKE2B_NULL_RLP {
            return true;
        }
        self.0.contains(key)
    }

    fn insert(&mut self, value: &[u8]) -> H256 {
        if value == &NULL_RLP {
            return BLAKE2B_NULL_RLP.clone();
        }
        self.0.insert(value)
    }

    fn emplace(&mut self, key: H256, value: DBValue) {
        if key == BLAKE2B_NULL_RLP {
            return;
        }
        self.0.emplace(key, value)
    }

    fn remove(&mut self, key: &H256) {
        if key == &BLAKE2B_NULL_RLP {
            return;
        }
        self.0.remove(key)
    }
}
