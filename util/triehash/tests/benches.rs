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

use triehash::trie_root;
use std::time::Instant;
use aion_types::H256;
use blake2b::blake2b;
use trie_standardmap::{Alphabet, ValueMode, StandardMap};

fn random_word(alphabet: &[u8], min_count: usize, diff_count: usize, seed: &mut H256) -> Vec<u8> {
    assert!(min_count + diff_count <= 32);
    *seed = blake2b(&seed);
    let r = min_count + (seed[31] as usize % (diff_count + 1));
    let mut ret: Vec<u8> = Vec::with_capacity(r);
    for i in 0..r {
        ret.push(alphabet[seed[i] as usize % alphabet.len()]);
    }
    ret
}

fn random_bytes(min_count: usize, diff_count: usize, seed: &mut H256) -> Vec<u8> {
    assert!(min_count + diff_count <= 32);
    *seed = blake2b(&seed);
    let r = min_count + (seed[31] as usize % (diff_count + 1));
    seed[0..r].to_vec()
}

fn random_value(seed: &mut H256) -> Vec<u8> {
    *seed = blake2b(&seed);
    match seed[0] % 2 {
        1 => vec![seed[31]; 1],
        _ => seed.to_vec(),
    }
}

#[test]
fn benchtest_triehash_insertions_32_mir_1k() {
    let st = StandardMap {
        alphabet: Alphabet::All,
        min_key: 32,
        journal_key: 0,
        value_mode: ValueMode::Mirror,
        count: 1000,
    };
    let d = st.make();

    let count = 1000;
    let time = Instant::now();

    let mut result = H256::default();
    for _ in 0..count {
        result = trie_root(d.clone()).clone();
    }

    assert!(result.0.len() != 0);

    let took = time.elapsed();
    println!(
        "[benchtest_triehash_insertions_32_mir_1k] triehash insertions 32 mirror 1k (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_triehash_insertions_32_ran_1k() {
    let st = StandardMap {
        alphabet: Alphabet::All,
        min_key: 32,
        journal_key: 0,
        value_mode: ValueMode::Random,
        count: 1000,
    };
    let d = st.make();

    let count = 1000;
    let time = Instant::now();

    let mut result = H256::default();
    for _ in 0..count {
        result = trie_root(d.clone()).clone();
    }

    assert!(result.0.len() != 0);

    let took = time.elapsed();
    println!(
        "[benchtest_triehash_insertions_32_ran_1k] triehash insertions 32 random 1k (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_triehash_insertions_six_high() {
    let mut d: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut seed = H256::new();
    for _ in 0..1000 {
        let k = random_bytes(6, 0, &mut seed);
        let v = random_value(&mut seed);
        d.push((k, v))
    }

    let count = 1000;
    let time = Instant::now();

    for _ in 0..count {
        trie_root(d.clone());
    }

    let took = time.elapsed();
    println!(
        "[benchtest_triehash_insertions_six_high] triehash insertions six high (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_triehash_insertions_six_mid() {
    let alphabet = b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_";
    let mut d: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut seed = H256::new();
    for _ in 0..1000 {
        let k = random_word(alphabet, 6, 0, &mut seed);
        let v = random_value(&mut seed);
        d.push((k, v))
    }

    let count = 1000;
    let time = Instant::now();

    for _ in 0..count {
        trie_root(d.clone());
    }

    let took = time.elapsed();
    println!(
        "[benchtest_triehash_insertions_six_mid] triehash insertions six mid (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_triehash_insertions_random_mid() {
    let alphabet = b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_";
    let mut d: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut seed = H256::new();
    for _ in 0..1000 {
        let k = random_word(alphabet, 1, 5, &mut seed);
        let v = random_value(&mut seed);
        d.push((k, v))
    }

    let count = 1000;
    let time = Instant::now();

    for _ in 0..count {
        trie_root(d.clone());
    }

    let took = time.elapsed();
    println!(
        "[benchtest_triehash_insertions_random_mid] triehash insertions six random mid (ns/call): \
         {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}

#[test]
fn benchtest_triehash_insertions_six_low() {
    let alphabet = b"abcdef";
    let mut d: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut seed = H256::new();
    for _ in 0..1000 {
        let k = random_word(alphabet, 6, 0, &mut seed);
        let v = random_value(&mut seed);
        d.push((k, v))
    }

    let count = 1000;
    let time = Instant::now();

    for _ in 0..count {
        trie_root(d.clone());
    }

    let took = time.elapsed();
    println!(
        "[benchtest_triehash_insertions_six_low] triehash insertions six low (ns/call): {}",
        (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
    );
}
