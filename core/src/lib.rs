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

// uncomment below line to test precompile contract blake2b hash
#![cfg_attr(feature = "benches", feature(test))]
extern crate bloomchain;
extern crate bn;
extern crate byteorder;
extern crate crossbeam;
extern crate common_types as types;
extern crate acore_bloom_journal as bloom_journal;
extern crate acore_io as io;
extern crate acore_bytes as bytes;
extern crate logger;
extern crate acore_stratum;
extern crate aion_types;
extern crate ethbloom;
extern crate ajson;
extern crate key;
extern crate crypto as rcrypto;
extern crate futures_cpupool;
extern crate futures;
extern crate itertools;
extern crate lru_cache;
extern crate num_cpus;
extern crate num;
extern crate aion_machine;
extern crate parking_lot;
extern crate rand;
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
extern crate dir;
extern crate transient_hashmap;
extern crate linked_hash_map;

extern crate abi;
#[macro_use]
extern crate abi_derive;
#[macro_use]
extern crate abi_contract;
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
extern crate macros;
#[macro_use]
extern crate log;
#[macro_use]
extern crate trace_time;
extern crate tempdir;
pub extern crate keychain;
extern crate equihash;
extern crate vms;
// for aion token bridge
extern crate tiny_keccak;
extern crate num_bigint;
extern crate bincode;
extern crate bytebuffer;
extern crate tokio;
extern crate avm_abi;

pub mod account_provider;
pub mod block;
pub mod client;
pub mod transaction;
pub mod db;
pub mod encoded;
pub mod engines;
pub mod error;
pub mod executed;
pub mod header;
pub mod machine;
pub mod miner;
pub mod pod_state;
pub mod service;
pub mod spec;
pub mod state;
pub mod state_db;
pub mod verification;
pub mod views;
pub mod account;

mod cache_manager;
mod blooms;
mod pod_account;
mod account_db;
mod precompiled;
mod executive;
mod externalities;
pub mod blockchain;
mod factory;

#[cfg(test)]
extern crate fastvm;
extern crate core;

pub use types::*;
pub use executive::contract_address;
