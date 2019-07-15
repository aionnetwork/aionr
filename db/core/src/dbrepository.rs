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

use rockskvdb;
use mockkvdb;
use std::collections::{HashMap, BTreeMap};
use parking_lot::RwLock;

use super::{Result, DBValue};
use traits::{KeyValueDAO, KeyValueDB};
use dbconfigs::RepositoryConfig;
use dbtransaction::{DBTransaction, DBOp};
use error::Error;

type DB = rockskvdb::Rockskvdb;
type DbName = String;
type MockDb = mockkvdb::Mockkvdb;

/// db repository
pub struct DbRepository {
    /// rocksdb in repository, and btreemap is faster than hashmap when searching.
    dbs: BTreeMap<DbName, RwLock<DB>>,
    /// db flush priority, cause Btreemap would sorted key by Dictionary order.
    db_priority: Vec<DbName>,
    /// dbs config, useful in reopening db.
    #[cfg(test)]
    configs: Vec<RepositoryConfig>,
}
/// db repository mock instance, useful in tests
pub struct MockDbRepository {
    ///mockdb in repository
    dbs: HashMap<DbName, RwLock<MockDb>>,
    /// dbs config
    #[cfg(test)]
    configs: Vec<String>,
}

//pub struct MemoryDBRepository {
//    dbs: HashMap<DbName, RwLock<MemoryDB>>,
//}

//impl MemoryDBRepository {
//    pub fn new() -> Self {
//        MemoryDBRepository {
//            dbs: HashMap::new(),
//        }
//    }
//    fn flush(&self) -> Result<()> { Ok(()) }
//
//    #[cfg(test)]
//    fn close_all(&mut self) {}
//
//    #[cfg(test)]
//    fn open_all(&mut self) {}
//}

impl DbRepository {
    /// insert a db to the repository
    pub fn insert_db(&mut self, _configs: Vec<RepositoryConfig>) -> Result<()> { unimplemented!() }
    /// init repository
    pub fn init(configs: Vec<RepositoryConfig>) -> Result<DbRepository> {
        #[cfg(test)]
        let dbconfigs = configs.clone();
        let mut dbs = BTreeMap::new();
        let mut db_names = vec![];
        for config in configs {
            match rockskvdb::Rockskvdb::open(&config.db_config, &config.db_path) {
                Ok(db) => {
                    dbs.insert(config.db_name.clone(), RwLock::new(db));
                    db_names.push(config.db_name.clone());
                }
                Err(e) => {
                    return Err(Error::OpenError {
                        name: config.db_name,
                        desc: e,
                    })
                }
            };
        }
        let dbrep = DbRepository {
            dbs,
            db_priority: db_names,
            #[cfg(test)]
            configs: dbconfigs,
        };
        Ok(dbrep)
    }
    /// flush overlay to disk
    fn flush(&self) -> Result<()> {
        for db_name in self.db_priority.clone() {
            match self.dbs.get(&*db_name) {
                Some(db) => {
                    let mut db = db.write();
                    db.flush().map_err(|e| {
                        Error::FlushError {
                            name: db_name,
                            desc: e,
                        }
                    })?;
                }
                _ => error!(target: "db","db:{} not found",db_name),
            }
        }
        Ok(())
    }
    /// close all dbs
    #[cfg(test)]
    fn close_all(&mut self) {
        self.db_priority.clear();
        self.dbs.clear();
    }
    /// reopen all dbs
    #[cfg(test)]
    fn open_all(&mut self) {
        self.close_all();
        let configs = self.configs.clone();
        for config in configs {
            match rockskvdb::Rockskvdb::open(&config.db_config, &config.db_path) {
                Ok(db) => {
                    self.dbs.insert(config.db_name.clone(), RwLock::new(db));
                    self.db_priority.push(config.db_name.clone());
                }
                Err(_e) => {}
            };
        }
    }
}

impl Drop for DbRepository {
    /// flush all dbs before drop.
    fn drop(&mut self) { let _ = self.flush(); }
}

impl MockDbRepository {
    /// init db repository
    pub fn init(configs: Vec<String>) -> Self {
        #[cfg(test)]
        let dbconfigs = configs.clone();
        let mut dbs = HashMap::new();
        for db_name in configs {
            let mut db = mockkvdb::Mockkvdb::new_default();
            dbs.insert(db_name, RwLock::new(db));
        }
        MockDbRepository {
            dbs,
            #[cfg(test)]
            configs: dbconfigs,
        }
    }
    /// flush all db
    fn flush(&self) -> Result<()> { Ok(()) }
    /// close all dbs
    #[cfg(test)]
    fn close_all(&mut self) { self.dbs.clear(); }
    /// reopen all dbs
    #[cfg(test)]
    fn open_all(&mut self) {
        self.close_all();
        let configs = self.configs.clone();
        for db_name in configs {
            let mut db = mockkvdb::Mockkvdb::new_default();
            self.dbs.insert(db_name, RwLock::new(db));
        }
    }
    pub fn get_db(&mut self, db_name: &str) -> &mut RwLock<MockDb> {
        self.dbs.get_mut(db_name).unwrap()
    }
}

macro_rules! impl_keyvaluedb {
    ($name: ident) => {
        impl KeyValueDB for $name {
            fn get(&self, db_name: &str, key: &[u8]) -> Result<Option<DBValue>> {
                match self.dbs.get(db_name) {
                    Some(ref db) => {
                        let db = db.read();
                        let res = db.get(&key.to_vec());
                        trace!(target:"db", "db:{}, Get key = {:?}, value = {:?}", db_name, key, res);
                        Ok(res)
                    }
                    _ => Err(Error::NotFound(db_name.into())),
                }
            }

            fn keys(&self) -> Option<Vec<String>> {
                return Some(self.dbs.keys().into_iter().map(|k| k.clone()).collect::<Vec<String>>());
            }

            fn write_buffered(&self, transaction: DBTransaction) {
                for db_name in transaction.dbs() {
                    match self.dbs.get(*db_name) {
                        Some(db) => {
                            let mut db = db.write();
                            let ops = match transaction.get_vec(db_name){
                                Some(ops) => ops,
                                None => {
                                    vec![]
                                }
                            };
                            for op in ops {
                                match op {
                                    DBOp::Delete {
                                        key,
                                    } => {
                                        trace!(target:"db", "db:{}, Delete key = {:?}", db_name, key);
                                        db.delete(&key);
                                    }
                                    DBOp::Insert {
                                        key,
                                        value,
                                    } => {
                                        trace!(target:"db", "db:{}, Put key = {:?}, value = {:?}", db_name, key, value);
                                        db.put(&key, &value);
                                    }
                                }
                            }
                        }
                        None => {
                            error!(target:"db","db:{} not found",db_name);
                        }
                    }
                }
            }

            fn iter(&self, db_name: &'static str) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)>> {
                match self.dbs.get(db_name) {
                    Some(db) => {
                        let db = db.read();
                        Box::new(db.iter())
                    }
                    _ => {
                        error!(target:"db","db:{} not found",db_name);
                        Box::new(None.into_iter())
                    }
                }
            }

            fn get_by_prefix(&self, db_name: &'static str, prefix: &[u8]) -> Option<Box<[u8]>> {
                match self.dbs.get(db_name) {
                    Some(db) => {
                        let db = db.read();
                        let res = db.get_by_prefix(prefix);
                        res
                    }
                    None => {
                        error!(target:"db","db:{} not found",db_name);
                        None
                    }
                }
            }

            fn iter_from_prefix<'a>(
                &'a self,
                db_name: &str,
                prefix: &'static [u8],
            ) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a>
            {
                match self.dbs.get(db_name) {
                    Some(db) => {
                        let db = db.read();
                        let res = db.iter_from_prefix(prefix);
                        res
                    }
                    None => {
                        error!(target:"db","db:{} not found",db_name);
                        Box::new(None.into_iter())
                    }
                }
            }

            fn flush(&self) -> Result<()> { $name::flush(self) }

            #[cfg(test)]
            fn close_all(&mut self) { $name::close_all(self); }
            #[cfg(test)]
            fn open_all(&mut self) { $name::open_all(self); }
        }
    };
}
impl_keyvaluedb!(DbRepository);
impl_keyvaluedb!(MockDbRepository);
//impl_keyvaluedb!(MemoryDBRepository);
