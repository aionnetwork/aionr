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

extern crate trie_standardmap;
extern crate triehash;
extern crate aion_types;
extern crate blake2b;

use std::time::Instant;
use aion_types::H256;
use blake2b::blake2b;
use trie_standardmap::{Alphabet, ValueMode, StandardMap};
use triehash::{trie_root, shared_prefix_len, hex_prefix_encode};

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
