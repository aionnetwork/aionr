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

#![warn(unused_extern_crates)]
#![cfg_attr(feature = "benches", feature(test))]
extern crate bloomchain;
extern crate byteorder;
extern crate crossbeam;
extern crate acore_bloom_journal as bloom_journal;
extern crate acore_io as io;
extern crate acore_bytes;
extern crate bytes;
extern crate aion_types;
extern crate ethbloom;
extern crate ajson;
extern crate key;
extern crate crypto as rcrypto;
extern crate itertools;
extern crate lru_cache;
extern crate num_cpus;
extern crate num;
extern crate aion_machine;
extern crate parking_lot;
extern crate rayon;
extern crate rlp;
extern crate rlp_compress;
extern crate blake2b;
extern crate heapsize;
extern crate patricia_trie as trie;
extern crate triehash;
extern crate ansi_term;
extern crate unexpected;
extern crate util_error;
extern crate db as kvdb;
extern crate transient_hashmap;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate rlp_derive;
extern crate rustc_hex;
extern crate stats;
extern crate stop_guard;
extern crate time;
extern crate using_queue;
extern crate table;
extern crate memory_cache;
extern crate journaldb;
#[macro_use]
extern crate log;
#[macro_use]
extern crate trace_time;
extern crate keychain;
extern crate equihash;
extern crate vms;
extern crate futures;
extern crate tokio;
extern crate state as crate_state;
extern crate tiny_keccak;
extern crate num_bigint;
extern crate bytebuffer;
#[cfg(test)]
extern crate fastvm;
#[cfg(test)]
extern crate logger;
#[cfg(test)]
extern crate tempdir;
#[cfg(test)]
extern crate avm_abi;
#[cfg(test)]
#[macro_use]
extern crate macros;
extern crate p2p;

/// pub mod is used here to avoid name collision when used in other module
pub mod account_provider;
// encoded header
pub mod encoded;
pub mod blockchain;
pub mod miner;
pub mod block;
pub mod client;
// unverified transaction
pub mod transaction;
// PoW Engine
pub mod engine;
//pub mod error;
pub mod header;
pub mod views;
pub mod sync;

// boot
pub mod service;
pub mod spec;
pub mod verification;

mod machine;
mod pod_state;
mod pod_account;
mod state;
// mod state_db;
mod db;
mod factory;
mod cache_manager;
// mod account_db;
mod precompiled;
mod executive;
mod externalities;
mod types;

#[cfg(test)]
mod tests;

pub use types::{
    filter,
    state::log_entry,
    state::receipt,
    state::state_diff,
    block::status as block_status,
    error::Error,
    error::CallError,
    error::ImportError
};

pub use executive::contract_address;

#[cfg(test)]
use tests::common::helpers;
