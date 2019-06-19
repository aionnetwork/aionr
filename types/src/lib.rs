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

extern crate aion_types;
extern crate ethbloom;
extern crate acore_bytes as bytes;
extern crate ajson;
extern crate rlp;
#[macro_use]
extern crate rlp_derive;
extern crate heapsize;
extern crate blake2b;

pub mod account_diff;
pub mod basic_account;
pub mod block_status;
pub mod blockchain_info;
pub mod call_analytics;
pub mod filter;
pub mod ids;
pub mod log_entry;
pub mod pruning_info;
pub mod receipt;
pub mod restoration_status;
pub mod security_level;
pub mod state_diff;
pub mod trace_filter;
pub mod tree_route;
pub mod verification_queue_info;
pub mod vms;

/// Type for block number.
pub type BlockNumber = u64;
/// Type for header version.
pub type HeaderVersion = u8;
