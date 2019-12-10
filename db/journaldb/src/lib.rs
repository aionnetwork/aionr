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

//! `JournalDB` interface and implementation.

extern crate heapsize;
#[macro_use]
extern crate log;

extern crate aion_types;
extern crate acore_bytes as bytes;
extern crate parking_lot;
extern crate plain_hasher;
extern crate rlp;
extern crate util_error as error;
extern crate db;
#[cfg(test)]
extern crate logger;
#[cfg(test)]
extern crate blake2b;

use std::{fmt, str};
use std::sync::Arc;
use db as kvdb;
/// Export the journaldb module.
mod traits;
mod archivedb;
mod overlayrecentdb;
#[cfg(test)]
mod tests;

/// Export the `JournalDB` trait.
pub use self::traits::JournalDB;

/// A journal database algorithm.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Algorithm {
    /// Keep all keys forever.
    Archive,
    /// Ancient and recent history maintained separately; recent history lasts for particular
    /// number of blocks.
    ///
    /// Inserts go into memory overlay, which is tried for key fetches. Memory overlay gets
    /// flushed in backing only at end of recent history.
    OverlayRecent,
}

impl Default for Algorithm {
    fn default() -> Algorithm { Algorithm::OverlayRecent }
}

impl str::FromStr for Algorithm {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "archive" => Ok(Algorithm::Archive),
            "fast" => Ok(Algorithm::OverlayRecent),
            e => Err(format!("Invalid algorithm: {}", e)),
        }
    }
}

impl Algorithm {
    /// Returns static str describing journal database algorithm.
    pub fn as_str(&self) -> &'static str {
        match *self {
            Algorithm::Archive => "archive",
            Algorithm::OverlayRecent => "fast",
        }
    }

    /// Returns static str describing journal database algorithm.
    pub fn as_internal_name_str(&self) -> &'static str {
        match *self {
            Algorithm::Archive => "archive",
            Algorithm::OverlayRecent => "overlayrecent",
        }
    }

    /// Returns true if pruning strategy is stable
    pub fn is_stable(&self) -> bool { true }

    /// Returns all algorithm types.
    pub fn all_types() -> Vec<Algorithm> { vec![Algorithm::Archive, Algorithm::OverlayRecent] }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.as_str()) }
}

/// Create a new `JournalDB` trait object over a generic key-value database.
pub fn new(
    backing: Arc<dyn (::kvdb::KeyValueDB)>,
    algorithm: Algorithm,
    db_name: &'static str,
) -> Box<dyn JournalDB>
{
    match algorithm {
        Algorithm::Archive => Box::new(archivedb::ArchiveDB::new(backing, db_name)),
        Algorithm::OverlayRecent => {
            Box::new(overlayrecentdb::OverlayRecentDB::new(backing, db_name))
        }
    }
}

// all keys must be at least 12 bytes
const DB_PREFIX_LEN: usize = ::kvdb::PREFIX_LEN;
const LATEST_ERA_KEY: [u8; ::kvdb::PREFIX_LEN] = [b'l', b'a', b's', b't', 0, 0, 0, 0, 0, 0, 0, 0];
