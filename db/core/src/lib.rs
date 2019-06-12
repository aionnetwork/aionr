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

#![allow(dead_code)]
#![allow(non_snake_case)]
#[macro_use]
extern crate log;
extern crate elastic_array;
extern crate acore_bytes;
extern crate aion_types;
extern crate multimap;
extern crate num_cpus;
extern crate parity_rocksdb;
extern crate regex;
extern crate blake2b;
extern crate rlp;
extern crate parking_lot;
extern crate interleaved_ordered;
extern crate heapsize;
extern crate plain_hasher;
extern crate rand;
mod dbrepository;
mod dbtransaction;
mod mockkvdb;
mod rockskvdb;
mod memorydb;
mod traits;
mod error;
mod dbconfigs;

use elastic_array::{ElasticArray32, ElasticArray128};
pub use dbrepository::{DbRepository, MockDbRepository, MemoryDBRepository};
pub use dbtransaction::{DBOp, DBTransaction};
pub use mockkvdb::Mockkvdb;
pub use rockskvdb::Rockskvdb;
pub use traits::{HashStore, AsHashStore, KeyValueDB};
pub use memorydb::MemoryDB;
pub use error::Error;
pub use dbconfigs::{DatabaseConfig, CompactionProfile, RepositoryConfig};

pub type Key = ElasticArray32<u8>;
pub type DBValue = ElasticArray128<u8>;
pub type Result<T> = ::std::result::Result<T, Error>;
pub const PREFIX_LEN: usize = 12;
