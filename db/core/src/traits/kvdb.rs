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

use super::{DBValue, Result};
use dbtransaction::DBTransaction;
/// basic kvdb operation.
pub trait KeyValueDAO: Sync + Send {
    /// Get value by key
    fn get(&self, k: &[u8]) -> Option<DBValue>;
    /// Insert a key-value pair to db, return value when success, otherwise None
    fn put(&mut self, k: &[u8], v: &DBValue) -> Option<DBValue>;
    /// Delete from db. return the value if the db has the pair.
    fn delete(&mut self, k: &[u8]) -> Option<DBValue>;
    /// Return an iterator
    fn iter(&self) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)>>;
    /// Get value by partial key. Prefix size should match configured prefix size. Only searches flushed values.
    fn get_by_prefix(&self, prefix: &[u8]) -> Option<Box<[u8]>>;
    /// Return an iterator, from the beginning the key that prefix size matching the configured prefix size
    fn iter_from_prefix(
        &self,
        prefix: &'static [u8],
    ) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)>>;
}
/// db repository operation.
pub trait KeyValueDB: Sync + Send {
    /// Get the value by key from the specified db
    fn get(&self, db_name: &str, key: &[u8]) -> Result<Option<DBValue>>;
    fn keys(&self) -> Option<Vec<String>>;
    /// Commit transaction to database and flush to db
    fn write(&self, transaction: DBTransaction) -> Result<()> {
        self.write_buffered(transaction);
        self.flush()
    }
    /// Commit transaction to db.
    fn write_buffered(&self, transaction: DBTransaction);
    /// Flush db
    fn flush(&self) -> Result<()> { Ok(()) }
    /// Return a specified db' iterator
    fn iter(&self, db_name: &'static str) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)>>;
    /// Get value by partial key. Prefix size should match configured prefix size. Only searches flushed values.
    fn get_by_prefix(&self, db_name: &'static str, prefix: &[u8]) -> Option<Box<[u8]>>;
    /// Return an iterator, from the beginning the key that prefix size matching the configured prefix size
    fn iter_from_prefix<'a>(
        &'a self,
        db_name: &'static str,
        prefix: &'static [u8],
    ) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a>;
    /// Close all dbs
    #[cfg(test)]
    fn close_all(&mut self);
    /// Reopen all dbs
    #[cfg(test)]
    fn open_all(&mut self);
}
