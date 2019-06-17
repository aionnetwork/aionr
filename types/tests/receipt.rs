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
extern crate aion_types;
extern crate rlp;
extern crate rustc_hex;

use std::convert::Into;
use rustc_hex::FromHex;
use rlp::{ encode, decode };
use types::receipt::Receipt;
use types::log_entry::LogEntry;
use aion_types::{ H256, U256 };

#[test]
fn test_no_state_root() {
    let expected: Vec<u8> = FromHex::from_hex("f90171a00000000000000000000000000000000000000000000000000000000000000000b9010000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000f845f843a0a00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2cc0a000000000000000000000000000000000000000000000000000000000000000008083040cae80").unwrap();
    let r = Receipt::new(
        H256::zero(),
        0x40cae.into(),
        U256::zero(),
        vec![LogEntry {
            address: "a00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2c".into(),
            topics: vec![],
            data: vec![0u8; 32],
        }],
        Vec::new(),
        String::default(),
    );
    assert_eq!(encode(&r)[..], expected[..]);
}

#[test]
fn test_basic() {
    let expected: Vec<u8> = FromHex::from_hex("f9016ea0444bff1d8ca768bb93124792be91579e30cec9cab923c617b0a120f82f7b923bb9010000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000f845f843a0a00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2cc0a00000000000000000000000000000000000000000000000000000000000000000808080").unwrap();
    let r = Receipt::new(
        "444bff1d8ca768bb93124792be91579e30cec9cab923c617b0a120f82f7b923b".into(),
        U256::zero(),
        U256::zero(),
        vec![LogEntry {
            address: "a00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2c".into(),
            topics: vec![],
            data: vec![0u8; 32],
        }],
        Vec::new(),
        String::default(),
    );
    let encoded = encode(&r);
    assert_eq!(&encoded[..], &expected[..]);
    let decoded: Receipt = decode(&encoded as &[u8]);
    assert_eq!(decoded, r);
}
