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

use token::{Tokenizer, StrictTokenizer};
use util::{pad_u32, pad_i32};
use errors::Error;

/// Tries to parse string as a token. Does not require string to clearly represent the value.
pub struct LenientTokenizer;

impl Tokenizer for LenientTokenizer {
    fn tokenize_address(value: &str) -> Result<[u8; 32], Error> {
        StrictTokenizer::tokenize_address(value)
    }

    fn tokenize_string(value: &str) -> Result<String, Error> {
        StrictTokenizer::tokenize_string(value)
    }

    fn tokenize_bool(value: &str) -> Result<bool, Error> { StrictTokenizer::tokenize_bool(value) }

    fn tokenize_bytes(value: &str) -> Result<Vec<u8>, Error> {
        StrictTokenizer::tokenize_bytes(value)
    }

    fn tokenize_fixed_bytes(value: &str, len: usize) -> Result<Vec<u8>, Error> {
        StrictTokenizer::tokenize_fixed_bytes(value, len)
    }

    fn tokenize_uint(value: &str) -> Result<[u8; 32], Error> {
        let result = StrictTokenizer::tokenize_uint(value);
        if result.is_ok() {
            return result;
        }

        let uint = try!(u32::from_str_radix(value, 10));
        Ok(pad_u32(uint))
    }

    fn tokenize_int(value: &str) -> Result<[u8; 32], Error> {
        let result = StrictTokenizer::tokenize_int(value);
        if result.is_ok() {
            return result;
        }

        let int = try!(i32::from_str_radix(value, 10));
        Ok(pad_i32(int))
    }
}
