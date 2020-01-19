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

extern crate acore_bloom_journal as bloom_journal;
extern crate acore_io as io;
extern crate crypto as rcrypto;
extern crate patricia_trie as trie;
extern crate db as kvdb;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate rlp_derive;
#[macro_use]
extern crate log;
#[macro_use]
extern crate trace_time;
// extern crate state as crate_state;

#[cfg(test)]
#[macro_use]
extern crate macros;

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

pub use crate::types::{
    filter,
    state::log_entry,
    state::receipt,
    state::state_diff,
    block::status as block_status,
    error::Error,
    error::CallError,
    error::ImportError,
    error::BlockError
};

pub use crate::executive::contract_address;

#[cfg(test)]
use crate::tests::common::helpers;
