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

#![feature(test)]
extern crate aion_types as bigint;
extern crate rlp;

use bigint::U256;
use rlp::{RlpStream, Rlp};
use std::time::Instant;

#[test]
fn benchtest_stream_u64_value() {
    let count = 10000;
    let time = Instant::now();
    for _i in 0..count {
        let mut stream = RlpStream::new();
        stream.append(&0x1023456789abcdefu64);
        let _ = stream.out();
    }
    let took = time.elapsed();
    println!(
        "[benchtest_stream_u64_value] rlp encode u64 (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_decode_u64_value() {
    let count = 10000;
    let time = Instant::now();
    let data = vec![0x88, 0x10, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
    for _i in 0..count {
        // u64
        let rlp = Rlp::new(&data);
        let _l: u64 = rlp.as_val();
    }
    let took = time.elapsed();
    println!(
        "[benchtest_decode_u64_value] rlp decode u64 (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_stream_u256_value() {
    let count = 10000;
    let uint: U256 = "8090a0b0c0d0e0f00910203040506077000000000000000100000000000012f0".into();
    let time = Instant::now();
    for _i in 0..count {
        // u256
        let mut stream = RlpStream::new();
        stream.append(&uint);
        let _ = stream.out();
    }
    let took = time.elapsed();
    println!(
        "[benchtest_stream_u256_value] rlp encode u256 (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_decode_u256_value() {
    let count = 10000;
    let time = Instant::now();
    for _i in 0..count {
        // u256
        let data = vec![
            0xa0, 0x80, 0x90, 0xa0, 0xb0, 0xc0, 0xd0, 0xe0, 0xf0, 0x09, 0x10, 0x20, 0x30, 0x40,
            0x50, 0x60, 0x77, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x12, 0xf0,
        ];
        let rlp = Rlp::new(&data);
        let _: U256 = rlp.as_val();
    }
    let took = time.elapsed();
    println!(
        "[benchtest_decode_u256_value] rlp decode u256 (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_stream_nested_empty_lists() {
    let count = 10000;
    let time = Instant::now();
    for _i in 0..count {
        // [ [], [[]], [ [], [[]] ] ]
        let mut stream = RlpStream::new_list(3);
        stream.begin_list(0);
        stream.begin_list(1).begin_list(0);
        stream
            .begin_list(2)
            .begin_list(0)
            .begin_list(1)
            .begin_list(0);
        let _a = stream.out();
    }
    let took = time.elapsed();
    println!(
        "[benchtest_stream_nested_empty_lists] rlp encode nested empty list (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_decode_nested_empty_lists() {
    let count = 10000;
    let time = Instant::now();
    for _i in 0..count {
        // [ [], [[]], [ [], [[]] ] ]
        let data = vec![0xc7, 0xc0, 0xc1, 0xc0, 0xc3, 0xc0, 0xc1, 0xc0];
        let rlp = Rlp::new(&data);
        let _v0: Vec<u16> = rlp.at(0).as_list();
        let _v1: Vec<u16> = rlp.at(1).at(0).as_list();
        let nested_rlp = rlp.at(2);
        let _v2a: Vec<u16> = nested_rlp.at(0).as_list();
        let _v2b: Vec<u16> = nested_rlp.at(1).at(0).as_list();
    }
    let took = time.elapsed();
    println!(
        "[benchtest_decode_nested_empty_lists] rlp decode nested empty list (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_stream_1000_empty_lists() {
    let count = 10000;
    let time = Instant::now();
    for _i in 0..count {
        let mut stream = RlpStream::new_list(1000);
        for _ in 0..1000 {
            stream.begin_list(0);
        }
        let _ = stream.out();
    }
    let took = time.elapsed();
    println!(
        "[benchtest_stream_1000_empty_lists] rlp encode nested 1000 empty list (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}
