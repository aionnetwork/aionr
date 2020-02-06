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

extern crate triehash;

use triehash::{trie_root, shared_prefix_len, hex_prefix_encode};

#[test]
fn test_hex_prefix_encode() {
    let v = vec![0, 0, 1, 2, 3, 4, 5];
    let e = vec![0x10, 0x01, 0x23, 0x45];
    let h = hex_prefix_encode(&v, false);
    assert_eq!(h, e);

    let v = vec![0, 1, 2, 3, 4, 5];
    let e = vec![0x00, 0x01, 0x23, 0x45];
    let h = hex_prefix_encode(&v, false);
    assert_eq!(h, e);

    let v = vec![0, 1, 2, 3, 4, 5];
    let e = vec![0x20, 0x01, 0x23, 0x45];
    let h = hex_prefix_encode(&v, true);
    assert_eq!(h, e);

    let v = vec![1, 2, 3, 4, 5];
    let e = vec![0x31, 0x23, 0x45];
    let h = hex_prefix_encode(&v, true);
    assert_eq!(h, e);

    let v = vec![1, 2, 3, 4];
    let e = vec![0x00, 0x12, 0x34];
    let h = hex_prefix_encode(&v, false);
    assert_eq!(h, e);

    let v = vec![4, 1];
    let e = vec![0x20, 0x41];
    let h = hex_prefix_encode(&v, true);
    assert_eq!(h, e);
}

#[test]
fn simple_test() {
    assert_eq!(
        trie_root(vec![(
            b"A",
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" as &[u8],
        )]),
        //            "d23786fb4a010da3ce639d66d5e904a11dbc02746d1ce25029e53290cabf28ab".into()
        // keccak->blake2b, update expected hash as well.
        "e9d7f23f40cd82fe35f5a7a6778c3503f775f3623ba7a71fb335f0eee29dac8a".into()
    );
}

#[test]
fn test_triehash_out_of_order() {
    assert!(
        trie_root(vec![
            (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
            (vec![0x81u8, 0x23], vec![0x81u8, 0x23]),
            (vec![0xf1u8, 0x23], vec![0xf1u8, 0x23]),
        ]) == trie_root(vec![
            (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
            (vec![0xf1u8, 0x23], vec![0xf1u8, 0x23]),
            (vec![0x81u8, 0x23], vec![0x81u8, 0x23]),
        ])
    );
}

#[test]
fn test_shared_prefix() {
    let a = vec![1, 2, 3, 4, 5, 6];
    let b = vec![4, 2, 3, 4, 5, 6];
    assert_eq!(shared_prefix_len(&a, &b), 0);
}

#[test]
fn test_shared_prefix2() {
    let a = vec![1, 2, 3, 3, 5];
    let b = vec![1, 2, 3];
    assert_eq!(shared_prefix_len(&a, &b), 3);
}

#[test]
fn test_shared_prefix3() {
    let a = vec![1, 2, 3, 4, 5, 6];
    let b = vec![1, 2, 3, 4, 5, 6];
    assert_eq!(shared_prefix_len(&a, &b), 6);
}
