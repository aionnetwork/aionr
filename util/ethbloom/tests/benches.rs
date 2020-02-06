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
#![cfg(feature = "benches")]

extern crate ethbloom;
extern crate rand;
extern crate fixed_hash;
extern crate blake2b;
#[macro_use]
extern crate crunchy;

use std::time::Instant;
use rand::Rng;
use fixed_hash::rustc_hex::FromHex;
use blake2b::blake2b;
use ethbloom::{Bloom, Input};

fn random_data() -> [u8; 256] {
    let mut res = [0u8; 256];
    rand::thread_rng().fill_bytes(&mut res);
    res
}

fn test_bloom() -> Bloom {
    "00000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002020000000000000000000000000000000000000000000008000000001000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into()
}

fn test_topic() -> Vec<u8> {
    "02c69be41d0b7e40352fc85be1cd65eb03d40ef8427a0ca4596b1ead9a00e9fc"
        .from_hex()
        .unwrap()
}

fn test_address() -> Vec<u8> {
    "ef2d6d194084c2de36e0dabfce45d046b37d1106"
        .from_hex()
        .unwrap()
}

fn test_dummy() -> Vec<u8> { b"123456".to_vec() }

fn test_dummy2() -> Vec<u8> { b"654321".to_vec() }

#[test]
fn benchtest_forwards_with_crunchy() {
    let mut data = random_data();

    let count = 1000;
    let time = Instant::now();

    for _ in 0..count {
        let other_data = random_data();
        unroll! {
            for i in 0..255 {
                data[i] |= other_data[i];
            }
        }
    }

    let took = time.elapsed();
    println!(
        "[benchtest_forwards_with_crunchy] forwards with crunchy (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_backwards_with_crunchy() {
    let mut data = random_data();
    let count = 1000;
    let time = Instant::now();

    for _ in 0..count {
        let other_data = random_data();
        unroll! {
            for i in 0..255 {
                data[255-i] |= other_data[255-i];
            }
        }
    }

    let took = time.elapsed();
    println!(
        "[benchtest_backwards_with_crunchy] backwards with crunchy (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_forwards_without_crunchy() {
    let mut data = random_data();
    let count = 1000;
    let time = Instant::now();

    for _ in 0..count {
        let other_data = random_data();
        for i in 0..255 {
            data[i] |= other_data[i];
        }
    }

    let took = time.elapsed();
    println!(
        "[benchtest_forwards_without_crunchy] forwards without crunchy (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_backwards_without_crunchy() {
    let mut data = random_data();
    let count = 1000;
    let time = Instant::now();

    for _ in 0..count {
        let other_data = random_data();
        for i in 0..255 {
            data[255 - i] |= other_data[255 - i];
        }
    }

    let took = time.elapsed();
    println!(
        "[benchtest_backwards_without_crunchy] backwards without crunchy (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_accrue_raw() {
    let mut bloom = Bloom::default();
    let topic = test_topic();
    let address = test_address();
    let count = 1000;
    let time = Instant::now();

    for _ in 0..count {
        bloom.accrue(Input::Raw(&topic));
        bloom.accrue(Input::Raw(&address));
    }

    let took = time.elapsed();
    println!(
        "[benchtest_accrue_raw] accure raw (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_accrue_hash() {
    let mut bloom = Bloom::default();
    let topic = blake2b(&test_topic());
    let address = blake2b(&test_address());
    let count = 1000;
    let time = Instant::now();
    for _ in 0..count {
        bloom.accrue(Input::Hash(&topic.0));
        bloom.accrue(Input::Hash(&address.0));
    }
    let took = time.elapsed();
    println!(
        "[benchtest_accrue_hash] accure hash (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_does_not_contain_raw() {
    let bloom = test_bloom();
    let dummy = test_dummy();
    let dummy2 = test_dummy2();
    let count = 1000;
    let time = Instant::now();
    for _ in 0..count {
        assert!(!bloom.contains_input(Input::Raw(&dummy)));
        assert!(!bloom.contains_input(Input::Raw(&dummy2)));
    }
    let took = time.elapsed();
    println!(
        "[benchtest_does_not_contain_raw] does not contain raw (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_does_not_contain_hash() {
    let bloom = test_bloom();
    let dummy = blake2b(&test_dummy());
    let dummy2 = blake2b(&test_dummy2());
    let count = 1000;
    let time = Instant::now();
    for _ in 0..count {
        assert!(!bloom.contains_input(Input::Hash(&dummy.0)));
        assert!(!bloom.contains_input(Input::Hash(&dummy2.0)));
    }
    let took = time.elapsed();
    println!(
        "[benchtest_does_not_contain_input_hash] does not contain hash (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_does_not_contain_random_hash() {
    let bloom = test_bloom();
    let dummy: Vec<_> = (0..255u8).into_iter().map(|i| blake2b(&[i])).collect();
    let count = 1000;
    let time = Instant::now();
    for _ in 0..count {
        for d in &dummy {
            assert!(!bloom.contains_input(Input::Hash(&d.0)));
        }
    }
    let took = time.elapsed();
    println!(
        "[benchtest_does_not_contain_random_hash] does not contain random hash (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}
