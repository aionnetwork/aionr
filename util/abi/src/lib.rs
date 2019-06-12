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
extern crate rustc_hex as hex;
extern crate serde;
extern crate serde_json;
extern crate blake2b;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate error_chain;

extern crate aion_types;

pub mod param_type;
pub mod token;
mod constructor;
mod contract;
mod decoder;
mod encoder;
mod errors;
mod event;
mod event_param;
mod filter;
mod function;
mod log;
mod operation;
mod param;
mod signature;
mod util;

pub use param_type::ParamType;
pub use constructor::Constructor;
pub use contract::{Contract, Functions, Events};
pub use token::Token;
pub use errors::{Error, ErrorKind, Result, ResultExt};
pub use encoder::encode;
pub use decoder::decode;
pub use filter::{Topic, TopicFilter, RawTopicFilter};
pub use function::Function;
pub use param::Param;
pub use log::{Log, RawLog, LogParam};
pub use event::Event;
pub use event_param::EventParam;

/// ABI address.
pub type Address = aion_types::Address;

/// ABI fixed bytes.
pub type FixedBytes = Vec<u8>;

/// ABI bytes.
pub type Bytes = Vec<u8>;

/// ABI signed integer.
pub type Int = aion_types::U256;

/// ABI unsigned integer.
pub type Uint = aion_types::U256;

/// Commonly used FixedBytes of size 32
pub type Hash = aion_types::H256;
