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
#![cfg_attr(test, feature(test))]

#[cfg(test)]
extern crate test;

extern crate aion_types;

pub use aion_types::H256;
pub use blake2b_impl::Blake2b;

mod blake2b_impl;

use std::io;
/// Get the BLAKE2B (i.e. BLAKE2B) hash of the empty bytes string.
pub const BLAKE2B_EMPTY: H256 = H256([
    0x0e, 0x57, 0x51, 0xc0, 0x26, 0xe5, 0x43, 0xb2, 0xe8, 0xab, 0x2e, 0xb0, 0x60, 0x99, 0xda, 0xa1,
    0xd1, 0xe5, 0xdf, 0x47, 0x77, 0x8f, 0x77, 0x87, 0xfa, 0xab, 0x45, 0xcd, 0xf1, 0x2f, 0xe3, 0xa8,
]);

/// The BLAKE2B of the RLP encoding of empty data.
///
pub const BLAKE2B_NULL_RLP: H256 = H256([
    0x45, 0xb0, 0xcf, 0xc2, 0x20, 0xce, 0xec, 0x5b, 0x7c, 0x1c, 0x62, 0xc4, 0xd4, 0x19, 0x3d, 0x38,
    0xe4, 0xeb, 0xa4, 0x8e, 0x88, 0x15, 0x72, 0x9c, 0xe7, 0x5f, 0x9c, 0x0a, 0xb0, 0xe4, 0xc1, 0xc0,
]);

/// The BLAKE2B of the RLP encoding of empty list.
///
pub const BLAKE2B_EMPTY_LIST_RLP: H256 = H256([
    0xda, 0x22, 0x3b, 0x09, 0x96, 0x7c, 0x5b, 0xd2, 0x11, 0x07, 0x43, 0x30, 0x7e, 0x0a, 0xf6, 0xd3,
    0x9f, 0x61, 0x72, 0x0a, 0xa7, 0x21, 0x8a, 0x64, 0x0a, 0x08, 0xee, 0xd1, 0x2d, 0xd5, 0x75, 0xc7,
]);

pub fn blake2b<T: AsRef<[u8]>>(s: T) -> H256 {
    let mut result = [0u8; 32];
    let input = s.as_ref();
    let mut blake2b = Blake2b::new(32);
    blake2b.update(&input);
    blake2b.finalize(&mut result);
    H256(result)
}

pub fn blake2b_buffer(r: &mut io::BufRead) -> Result<H256, io::Error> {
    let mut output = [0u8; 32];
    let mut input = [0u8; 1024];
    let mut blake2b = Blake2b::new(32);

    // read file
    loop {
        let some = r.read(&mut input)?;
        if some == 0 {
            break;
        }
        blake2b.update(&input[0..some]);
    }

    blake2b.finalize(&mut output);
    Ok(output.into())
}
