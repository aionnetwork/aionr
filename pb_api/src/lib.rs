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

#![warn(unused_extern_crates)]
extern crate zmq;
extern crate protobuf;
extern crate aion_rpc;
extern crate aion_types;
extern crate rustc_hex;
extern crate acore_bytes as bytes;
extern crate parking_lot;
extern crate acore_io as io;
extern crate acore;
extern crate crossbeam;
extern crate dir;
#[macro_use]
extern crate log;
#[cfg(test)]
extern crate db as kvdb;
#[cfg(test)]
extern crate sync;
#[cfg(test)]
#[macro_use]
extern crate lazy_static;
#[cfg(test)]
extern crate rand;

pub mod message;
mod protobuf_engine;
pub mod pb_api_util;
pub mod api_process;
mod tx_pending_status;

pub use protobuf_engine::PBEngine;
pub use protobuf_engine::WalletApiConfiguration;
pub use protobuf_engine::new_pb;

static LOG_TARGET: &str = "pb_api";
