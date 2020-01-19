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

mod kat;

use blake2b::{Blake2b, KEY_BYTES, OUT_BYTES};
use kat::{BLAKE2B_KAT_OUT_SIZE, BLAKE2B_KAT,BLAKE2B_KEYED_KAT};

// the same blake256 test case with that in aion java impl.
#[test]
fn test_blake2b_256() {
    let out: [u8; 32] = Blake2b::hash_256(&"test".as_bytes());
    let hex_vector: Vec<String> = out.iter().map(|b| format!("{:02x}", b)).collect();
    let expected = "928b20366943e2afd11ebc0eae2e53a93bf177a4fcf35bcc64d503704e65e202";
    let actual = hex_vector.join("");
    assert_eq!(expected, actual);
}

#[test]
fn test_blake2b_out_size() {
    let input = [0u8; 256];

    for i in 0..BLAKE2B_KAT_OUT_SIZE.len() {
        let out_size = i + 1;
        let mut out = [0u8; OUT_BYTES];
        let mut h = Blake2b::new(out_size);
        h.update(input.as_ref());
        h.finalize(&mut out[..out_size]);
        assert_eq!(&out[..out_size], BLAKE2B_KAT_OUT_SIZE[i]);
    }
}

#[test]
fn test_blake2b_kat() {
    let mut input = [0u8; 256];
    for i in 0..input.len() {
        input[i] = i as u8;
    }

    for i in 0..BLAKE2B_KAT.len() {
        let mut h = Blake2b::new(OUT_BYTES);
        let mut out = [0u8; OUT_BYTES];
        h.update(&input[..i]);
        h.finalize(&mut out);
        assert_eq!(out.as_ref(), BLAKE2B_KAT[i].as_ref());
    }
}

#[test]
fn test_blake2b_keyed_kat() {
    let mut input = [0u8; 256];
    let mut key = [0u8; KEY_BYTES];

    for i in 0..input.len() {
        input[i] = i as u8;
    }

    for i in 0..key.len() {
        key[i] = i as u8;
    }

    for i in 0..BLAKE2B_KEYED_KAT.len() {
        let mut h = Blake2b::new_with_key(OUT_BYTES, key.as_ref());
        let mut out = [0u8; OUT_BYTES];
        h.update(&input[..i]);
        h.finalize(&mut out);
        assert_eq!(out.as_ref(), BLAKE2B_KEYED_KAT[i].as_ref());
    }
}
