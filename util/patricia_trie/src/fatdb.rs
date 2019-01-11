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
use super::{TrieDB, Trie, TrieDBIterator, TrieItem, TrieIterator, Query};

/// A `Trie` implementation which hashes keys and uses a generic `HashStore` backing database.
/// Additionaly it stores inserted hash-key mappings for later retrieval.
///
/// Use it as a `Trie` or `TrieMut` trait object.
pub struct FatDB<'db> {
    raw: TrieDB<'db>,
}

impl<'db> FatDB<'db> {
    /// Create a new trie with the backing database `db` and empty `root`
    /// Initialise to the state entailed by the genesis block.
    /// This guarantees the trie is built correctly.
    pub fn new(db: &'db HashStore, root: &'db H256) -> super::Result<Self> {
        let fatdb = FatDB {
            raw: TrieDB::new(db, root)?,
        };

        Ok(fatdb)
    }

    /// Get the backing database.
    pub fn db(&self) -> &HashStore { self.raw.db() }
}

impl<'db> Trie for FatDB<'db> {
    fn iter<'a>(&'a self) -> super::Result<Box<TrieIterator<Item = TrieItem> + 'a>> {
        FatDBIterator::new(&self.raw).map(|iter| Box::new(iter) as Box<_>)
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

/// Itarator over inserted pairs of key values.
pub struct FatDBIterator<'db> {
    trie_iterator: TrieDBIterator<'db>,
    trie: &'db TrieDB<'db>,
}

impl<'db> FatDBIterator<'db> {
    /// Creates new iterator.
    pub fn new(trie: &'db TrieDB) -> super::Result<Self> {
        Ok(FatDBIterator {
            trie_iterator: TrieDBIterator::new(trie)?,
            trie: trie,
        })
    }
}

impl<'db> TrieIterator for FatDBIterator<'db> {
    fn seek(&mut self, key: &[u8]) -> super::Result<()> { self.trie_iterator.seek(&blake2b(key)) }
}

impl<'db> Iterator for FatDBIterator<'db> {
    type Item = TrieItem<'db>;

    fn next(&mut self) -> Option<Self::Item> {
        self.trie_iterator.next().map(|res| {
            res.map(|(hash, value)| {
                let aux_hash = blake2b(hash);
                (
                    self.trie
                        .db()
                        .get(&aux_hash)
                        .expect("Missing fatdb hash")
                        .into_vec(),
                    value,
                )
            })
        })
    }
}

#[test]
fn fatdb_to_trie() {
    use db::{MemoryDB, DBValue};
    use super::fatdbmut::FatDBMut;
    use super::TrieMut;

    let mut memdb = MemoryDB::new();
    let mut root = H256::default();
    {
        let mut t = FatDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
    }
    let t = FatDB::new(&memdb, &root).unwrap();
    assert_eq!(
        t.get(&[0x01u8, 0x23]).unwrap().unwrap(),
        DBValue::from_slice(&[0x01u8, 0x23])
    );
    assert_eq!(
        t.iter().unwrap().map(Result::unwrap).collect::<Vec<_>>(),
        vec![(
            vec![0x01u8, 0x23],
            DBValue::from_slice(&[0x01u8, 0x23] as &[u8])
        )]
    );
}
