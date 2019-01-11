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

extern crate acore_bytes as bytes;
extern crate blake2b;
#[macro_use]
extern crate log;
extern crate rustc_hex as hex;

mod equihash_validator;

pub use equihash_validator::EquihashValidator;

fn extend_array(input: &[u8], output: &mut [u8], bit_len: i32, byte_pad: i32) {
    let out_width: i32 = (bit_len + 7) / 8 + byte_pad;
    let bit_len_mask: i32 = (1 << bit_len) - 1;
    let mut acc_bits: i32 = 0;
    let mut acc_value: i32 = 0;
    let mut j = 0;
    for i in 0..input.len() {
        acc_value = (acc_value << 8) | ((input[i] as i32) & 0xff);
        acc_bits += 8;

        if acc_bits >= bit_len {
            acc_bits -= bit_len;

            for x in byte_pad..out_width {
                let temp = 8 * (out_width - x - 1);
                output[(j + x) as usize] = ((
                    // Big-endian
                    // it's >>> in java.
                    (acc_value as u32) >> ((acc_bits + temp) % 32)
                ) & (
                    // Apply bit_len_mask across byte boundaries
                    ((bit_len_mask as u32) >> (temp % 32)) & 0xFF
                )) as u8;
            }
            j = j + out_width;
        }
    }
}
