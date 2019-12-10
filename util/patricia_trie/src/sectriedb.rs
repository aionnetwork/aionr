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

use aion_types::H256;
use blake2b::blake2b;
use db::HashStore;
use super::triedb::TrieDB;
use super::{Trie, TrieItem, TrieIterator, Query};

/// A `Trie` implementation which hashes keys and uses a generic `HashStore` backing database.
///
/// Use it as a `Trie` trait object. You can use `raw()` to get the backing `TrieDB` object.
pub struct SecTrieDB<'db> {
    raw: TrieDB<'db>,
}

impl<'db> SecTrieDB<'db> {
    /// Create a new trie with the backing database `db` and empty `root`
    ///
    /// Initialise to the state entailed by the genesis block.
    /// This guarantees the trie is built correctly.
    /// Returns an error if root does not exist.
    pub fn new(db: &'db dyn HashStore, root: &'db H256) -> super::Result<Self> {
        Ok(SecTrieDB {
            raw: TrieDB::new(db, root)?,
        })
    }

    /// Get a reference to the underlying raw `TrieDB` struct.
    pub fn raw(&self) -> &TrieDB { &self.raw }

    /// Get a mutable reference to the underlying raw `TrieDB` struct.
    pub fn raw_mut(&mut self) -> &mut TrieDB<'db> { &mut self.raw }
}

impl<'db> Trie for SecTrieDB<'db> {
    fn iter<'a>(&'a self) -> super::Result<Box<dyn TrieIterator<Item = TrieItem> + 'a>> {
        TrieDB::iter(&self.raw)
    }

    fn root(&self) -> &H256 { self.raw.root() }

    fn contains(&self, key: &[u8]) -> super::Result<bool> { self.raw.contains(&blake2b(key)) }

    fn get_with<'a, 'key, Q: Query>(
        &'a self,
        key: &'key [u8],
        query: Q,
    ) -> super::Result<Option<Q::Item>>
    where
        'a: 'key,
    {
        self.raw.get_with(&blake2b(key), query)
    }
}

#[test]
fn trie_to_sectrie() {
    use db::{MemoryDB, DBValue};
    use super::triedbmut::TrieDBMut;
    use super::TrieMut;

    let mut memdb = MemoryDB::new();
    let mut root = H256::default();
    {
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&blake2b(&[0x01u8, 0x23]), &[0x01u8, 0x23])
            .unwrap();
    }
    let t = SecTrieDB::new(&memdb, &root).unwrap();
    assert_eq!(
        t.get(&[0x01u8, 0x23]).unwrap().unwrap(),
        DBValue::from_slice(&[0x01u8, 0x23])
    );
}
