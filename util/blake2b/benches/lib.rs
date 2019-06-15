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

extern crate tiny_keccak;
extern crate blake2b;

use std::iter::repeat;
use std::time::Instant;
use tiny_keccak::keccak256;
use blake2b::{Blake2b, OUT_BYTES};

fn bench_chunk_size(n: usize, count: u64) {
    let mut h = Blake2b::new(OUT_BYTES);
    let input: Vec<u8> = repeat(0).take(n).collect();
    for _ in 0..count {
        h.update(input.as_ref());
    }
}

#[test]
fn benchtest_blake2b_16() {
    let time = Instant::now();
    let count = 1000;
    bench_chunk_size(16, count);
    let took = time.elapsed();
    println!(
        "[bench_blake2b_16] blake2b 16 (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_blake2b_1k() {
    let time = Instant::now();
    let count = 1000;
    bench_chunk_size(1 << 10, count);
    let took = time.elapsed();
    println!(
        "[bench_blake2b_1k] blake2b 1k (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_blake2b_64k() {
    let time = Instant::now();
    let count = 1000;
    bench_chunk_size(1 << 16, count);
    let took = time.elapsed();
    println!(
        "[bench_blake2b_64k] blake2b 64k (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_blake2b_256() {
    let count = 1000;
    let time = Instant::now();
    for _ in 0..count {
        Blake2b::hash_256(&"test".as_bytes());
    }
    let took = time.elapsed();
    println!(
        "[benchtest_blake2b_256] blake2b 256 (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_hash() {
    let input = Blake2b::hash_256(&"test".as_bytes());
    let count = 10000;

    // warm up
    for _i in 0..count {
        Blake2b::hash_256(&input);
        keccak256(&input);
    }

    // blake2b
    let mut ellapse = Instant::now();
    for _i in 0..count {
        Blake2b::hash_256(&input);
    }
    let mut took = ellapse.elapsed();

    println!(
        "[benchtest_hash_blake2b] Blake2b (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );

    // keccak
    ellapse = Instant::now();
    for _i in 0..count {
        keccak256(&input);
    }
    took = ellapse.elapsed();
    println!(
        "[benchtest_hash_keccak] keccak (ns/call): {} ",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}