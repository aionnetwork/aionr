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
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(asm_available, feature(asm))]

#[cfg(feature = "std")]
extern crate core;
#[macro_use]
extern crate crunchy;
#[macro_use]
extern crate uint as uint_crate;
#[macro_use]
extern crate fixed_hash;
extern crate num_bigint;

#[cfg(feature = "serialize")]
extern crate ethereum_types_serialize;
#[cfg(feature = "serialize")]
extern crate serde;

mod hash;
mod uint;

pub use uint::{U64, U128, U256, U512};
pub use hash::{H32, H64, H128, H160, H256, H264, H512, H520, H768, H1024};
pub use fixed_hash::clean_0x;

pub type Address = H256;
pub type Ed25519Public = H256;

pub fn to_u256(input: Vec<u8>, length: usize) -> U256 {
    if input.len() > length {
        U256::from_big_endian(&input[input.len() - length..input.len()])
    } else {
        U256::from_big_endian(&input.as_slice())
    }
}
