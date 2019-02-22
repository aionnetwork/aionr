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

extern crate parking_lot;
extern crate bincode;
extern crate byteorder;
extern crate bytes;
extern crate futures;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate lru_cache;
extern crate rand;
extern crate rustc_hex;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate socket2;
extern crate state;
extern crate tokio;
extern crate tokio_codec;
extern crate tokio_reactor;
extern crate tokio_threadpool;
extern crate tokio_current_thread;

extern crate blake2b;
extern crate acore;
extern crate acore_bytes;
extern crate acore_io;
extern crate aion_types;
extern crate rlp;
extern crate uuid;
extern crate aion_version as version;

pub mod net;
pub mod p2p;
pub mod sync;
pub mod light;

extern crate db as kvdb;

#[cfg(test)]
mod tests;
