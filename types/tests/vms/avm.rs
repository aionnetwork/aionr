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

extern crate types;

use types::vms::avm::{NativeDecoder, NativeEncoder};
use std::{u16, u32, u64, u8};

#[test]
pub fn test_codec() {
    let mut encoder = NativeEncoder::new();
    encoder.encode_byte(u8::MIN);
    encoder.encode_byte(u8::MAX);
    encoder.encode_short(u16::MIN);
    encoder.encode_short(u16::MAX);
    encoder.encode_int(u32::MIN);
    encoder.encode_int(u32::MAX);
    encoder.encode_long(u64::MIN);
    encoder.encode_long(u64::MAX);
    encoder.encode_bytes(&"".as_bytes().to_vec());
    encoder.encode_bytes(&"test".as_bytes().to_vec());
    let bytes = encoder.to_bytes();

    let mut decoder = NativeDecoder::new(&bytes);
    assert_eq!(u8::MIN, decoder.decode_byte().unwrap());
    assert_eq!(u8::MAX, decoder.decode_byte().unwrap());
    assert_eq!(u16::MIN, decoder.decode_short().unwrap());
    assert_eq!(u16::MAX, decoder.decode_short().unwrap());
    assert_eq!(u32::MIN, decoder.decode_int().unwrap());
    assert_eq!(u32::MAX, decoder.decode_int().unwrap());
    assert_eq!(u64::MIN, decoder.decode_long().unwrap());
    assert_eq!(u64::MAX, decoder.decode_long().unwrap());
    assert_eq!("".as_bytes().to_vec(), decoder.decode_bytes().unwrap());
    assert_eq!("test".as_bytes().to_vec(), decoder.decode_bytes().unwrap());
}