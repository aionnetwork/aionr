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

//! methods for FastVM implemented by Rust
extern crate bincode;
extern crate libc;
extern crate num_bigint;
extern crate aion_types;
extern crate acore_bytes as bytes;
extern crate ajson;
extern crate blake2b as hash;
extern crate common_types as types;
#[macro_use]
extern crate log;
extern crate rustc_hex;
extern crate db as kvdb;
#[cfg(test)]
mod tests;

pub mod context;
pub mod basetypes;
pub mod callback;

pub mod vm;
pub mod env_info;
mod ffi;
mod core;

pub use core::FastVM;
pub use ffi::EvmStatusCode;
