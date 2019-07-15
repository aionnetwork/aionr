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

use elastic_array::ElasticArray32;
use multimap::MultiMap;
use std::collections::hash_map::Keys;
use super::DBValue;

/// Database operation.
#[derive(Clone, Debug, PartialEq)]
pub enum DBOp {
    Insert {
        key: ElasticArray32<u8>,
        value: DBValue,
    },
    Delete {
        key: ElasticArray32<u8>,
    },
}

impl DBOp {
    /// Returns the key associated with this operation.
    pub fn key(&self) -> &[u8] {
        match *self {
            DBOp::Insert {
                ref key, ..
            } => key,
            DBOp::Delete {
                ref key, ..
            } => key,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct DBTransaction {
    /// Database operations.
    pub ops: MultiMap<&'static str, DBOp>,
}

impl DBTransaction {
    /// Create new transaction.
    pub fn new() -> DBTransaction { DBTransaction::with_capacity(256) }

    /// Create new transaction with capacity.
    pub fn with_capacity(cap: usize) -> DBTransaction {
        DBTransaction {
            ops: MultiMap::with_capacity(cap),
        }
    }

    /// Insert a key-value pair in the transaction. Any existing value will be overwritten upon write.
    pub fn put(&mut self, db_name: &'static str, key: &[u8], value: &[u8]) {
        let mut ekey = ElasticArray32::new();
        ekey.append_slice(key);
        self.ops.insert(
            db_name,
            DBOp::Insert {
                key: ekey,
                value: DBValue::from_slice(value),
            },
        );
    }

    /// Insert a key-value pair in the transaction. Any existing value will be overwritten upon write.
    pub fn put_vec(&mut self, db_name: &'static str, key: &[u8], value: Vec<u8>) {
        let mut ekey = ElasticArray32::new();
        ekey.append_slice(key);
        self.ops.insert(
            db_name,
            DBOp::Insert {
                key: ekey,
                value: DBValue::from_vec(value),
            },
        );
    }

    /// Delete value by key.
    pub fn delete(&mut self, db_name: &'static str, key: &[u8]) {
        let mut ekey = ElasticArray32::new();
        ekey.append_slice(key);
        self.ops.insert(
            db_name,
            DBOp::Delete {
                key: ekey,
            },
        );
    }

    /// get the first dbop in multimap by name.
    pub fn get(&self, db_name: &str) -> Option<DBOp> {
        let dbop = self.ops.get(db_name);
        match dbop {
            None => None,
            Some(q) => Some(q.clone()),
        }
    }

    /// get all dbop in multimap by name.
    pub fn get_vec(&self, db_name: &str) -> Option<Vec<DBOp>> {
        let dbop_vec = self.ops.get_vec(db_name);
        match dbop_vec {
            None => None,
            Some(q) => Some(q.clone()),
        }
    }

    ///return true if map contains db.
    pub fn contains_db(&self, db_name: &str) -> bool { self.ops.contains_key(db_name) }

    /// An iterator visiting all dbs in
    pub fn dbs(&self) -> Keys<&str, Vec<DBOp>> { self.ops.keys() }
}
