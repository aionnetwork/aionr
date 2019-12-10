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
//! Generetes trie root.
//!
//! This module should be used to generate trie root hash.

extern crate aion_types;
extern crate blake2b;
extern crate rlp;

use std::collections::BTreeMap;
use std::cmp;
use aion_types::H256;
use blake2b::blake2b;
use rlp::RlpStream;

pub fn shared_prefix_len<T: Eq>(first: &[T], second: &[T]) -> usize {
    let len = cmp::min(first.len(), second.len());
    (0..len).take_while(|&i| first[i] == second[i]).count()
}

/// Generates a trie root hash for a vector of values
///
/// ```rust
/// extern crate triehash;
/// use triehash::ordered_trie_root;
///
/// fn main() {
///     let v = &["doe", "reindeer"];
///     let root = "ac57bd4773cb143f3d553d4bd887170a6f18a6e6bd39957b97c35f47db0fe90a";
///     assert_eq!(ordered_trie_root(v), root.into());
/// }
/// ```
pub fn ordered_trie_root<I, A>(input: I) -> H256
where
    I: IntoIterator<Item = A>,
    A: AsRef<[u8]>,
{
    let gen_input: Vec<_> = input
        // first put elements into btree to sort them by nibbles
        // optimize it later
        .into_iter()
        .enumerate()
        .map(|(i, slice)| (rlp::encode(&i), slice))
        .collect::<BTreeMap<_, _>>()
        // then move them to a vector
        .into_iter()
        .map(|(k, v)| (as_nibbles(&k), v))
        .collect();

    gen_trie_root(&gen_input)
}

/// Generates a trie root hash for a vector of key-values
///
/// ```rust
/// extern crate triehash;
/// use triehash::trie_root;
///
/// fn main() {
///     let v = vec![
///         ("doe", "reindeer"),
///         ("dog", "puppy"),
///         ("dogglesworth", "cat"),
///     ];
///
///     //let root = "8aad789dff2f538bca5d8ea56e8abe10f4c7ba3a5dea95fea4cd6e7c3a1168d3";
///     let root = "6ca394ff9b13d6690a51dea30b1b5c43108e52944d30b9095227c49bae03ff8b";
///     assert_eq!(trie_root(v), root.into());
/// }
/// ```
pub fn trie_root<I, A, B>(input: I) -> H256
where
    I: IntoIterator<Item = (A, B)>,
    A: AsRef<[u8]> + Ord,
    B: AsRef<[u8]>,
{
    let gen_input: Vec<_> = input
        // first put elements into btree to sort them and to remove duplicates
        .into_iter()
        .collect::<BTreeMap<_, _>>()
        // then move them to a vector
        .into_iter()
        .map(|(k, v)| (as_nibbles(k.as_ref()), v))
        .collect();

    gen_trie_root(&gen_input)
}

/// Generates a key-hashed (secure) trie root hash for a vector of key-values.
///
/// ```rust
/// extern crate triehash;
/// use triehash::sec_trie_root;
///
/// fn main() {
///     let v = vec![
///         ("doe", "reindeer"),
///         ("dog", "puppy"),
///         ("dogglesworth", "cat"),
///     ];
///
///     //let root = "d4cd937e4a4368d7931a9cf51686b7e10abb3dce38a39000fd7902a092b64585";
///     let root = "9816e53f2e3960e056094915e839c355474f82329af2ef731dce76edc3dbfff5";
///     assert_eq!(sec_trie_root(v), root.into());
/// }
/// ```
pub fn sec_trie_root<I, A, B>(input: I) -> H256
where
    I: IntoIterator<Item = (A, B)>,
    A: AsRef<[u8]>,
    B: AsRef<[u8]>,
{
    let gen_input: Vec<_> = input
        // first put elements into btree to sort them and to remove duplicates
        .into_iter()
        .map(|(k, v)| (blake2b(k), v))
        .collect::<BTreeMap<_, _>>()
        // then move them to a vector
        .into_iter()
        .map(|(k, v)| (as_nibbles(&k), v))
        .collect();

    gen_trie_root(&gen_input)
}

fn gen_trie_root<A: AsRef<[u8]>, B: AsRef<[u8]>>(input: &[(A, B)]) -> H256 {
    let mut stream = RlpStream::new();
    hash256rlp(input, 0, &mut stream);
    blake2b(stream.out())
}

/// Hex-prefix Notation. First nibble has flags: oddness = 2^0 & termination = 2^1.
///
/// The "termination marker" and "leaf-node" specifier are completely equivalent.
///
/// Input values are in range `[0, 0xf]`.
///
/// ```markdown
///  [0,0,1,2,3,4,5]   0x10012345 // 7 > 4
///  [0,1,2,3,4,5]     0x00012345 // 6 > 4
///  [1,2,3,4,5]       0x112345   // 5 > 3
///  [0,0,1,2,3,4]     0x00001234 // 6 > 3
///  [0,1,2,3,4]       0x101234   // 5 > 3
///  [1,2,3,4]         0x001234   // 4 > 3
///  [0,0,1,2,3,4,5,T] 0x30012345 // 7 > 4
///  [0,0,1,2,3,4,T]   0x20001234 // 6 > 4
///  [0,1,2,3,4,5,T]   0x20012345 // 6 > 4
///  [1,2,3,4,5,T]     0x312345   // 5 > 3
///  [1,2,3,4,T]       0x201234   // 4 > 3
/// ```
pub fn hex_prefix_encode(nibbles: &[u8], leaf: bool) -> Vec<u8> {
    let inlen = nibbles.len();
    let oddness_factor = inlen % 2;
    // next even number divided by two
    let reslen = (inlen + 2) >> 1;
    let mut res = Vec::with_capacity(reslen);

    let first_byte = {
        let mut bits = ((inlen as u8 & 1) + (2 * leaf as u8)) << 4;
        if oddness_factor == 1 {
            bits += nibbles[0];
        }
        bits
    };

    res.push(first_byte);

    let mut offset = oddness_factor;
    while offset < inlen {
        let byte = (nibbles[offset] << 4) + nibbles[offset + 1];
        res.push(byte);
        offset += 2;
    }

    res
}

/// Converts slice of bytes to nibbles.
fn as_nibbles(bytes: &[u8]) -> Vec<u8> {
    let mut res = Vec::with_capacity(bytes.len() * 2);
    for i in 0..bytes.len() {
        let byte = bytes[i];
        res.push(byte >> 4);
        res.push(byte & 0b1111);
    }
    res
}

fn hash256rlp<A: AsRef<[u8]>, B: AsRef<[u8]>>(
    input: &[(A, B)],
    pre_len: usize,
    stream: &mut RlpStream,
)
{
    let inlen = input.len();

    // in case of empty slice, just append empty data
    if inlen == 0 {
        stream.append_empty_data();
        return;
    }

    // take slices
    let key: &[u8] = &input[0].0.as_ref();
    let value: &[u8] = &input[0].1.as_ref();

    // if the slice contains just one item, append the suffix of the key
    // and then append value
    if inlen == 1 {
        stream.begin_list(2);
        stream.append(&hex_prefix_encode(&key[pre_len..], true));
        stream.append(&value);
        return;
    }

    // get length of the longest shared prefix in slice keys
    let shared_prefix = input
        .iter()
        // skip first element
        .skip(1)
        // get minimum number of shared nibbles between first and each successive
        .fold(key.len(), |acc, &(ref k, _)| {
            cmp::min(shared_prefix_len(key, k.as_ref()), acc)
        });

    // if shared prefix is higher than current prefix append its
    // new part of the key to the stream
    // then recursively append suffixes of all items who had this key
    if shared_prefix > pre_len {
        stream.begin_list(2);
        stream.append(&hex_prefix_encode(&key[pre_len..shared_prefix], false));
        hash256aux(input, shared_prefix, stream);
        return;
    }

    // an item for every possible nibble/suffix
    // + 1 for data
    stream.begin_list(17);

    // if first key len is equal to prefix_len, move to next element
    let mut begin = match pre_len == key.len() {
        true => 1,
        false => 0,
    };

    // iterate over all possible nibbles
    for i in 0..16 {
        // cout how many successive elements have same next nibble
        let len = match begin < input.len() {
            true => {
                input[begin..]
                    .iter()
                    .take_while(|pair| pair.0.as_ref()[pre_len] == i)
                    .count()
            }
            false => 0,
        };

        // if at least 1 successive element has the same nibble
        // append their suffixes
        match len {
            0 => {
                stream.append_empty_data();
            }
            _ => hash256aux(&input[begin..(begin + len)], pre_len + 1, stream),
        }
        begin += len;
    }

    // if fist key len is equal prefix, append its value
    match pre_len == key.len() {
        true => {
            stream.append(&value);
        }
        false => {
            stream.append_empty_data();
        }
    };
}

fn hash256aux<A: AsRef<[u8]>, B: AsRef<[u8]>>(
    input: &[(A, B)],
    pre_len: usize,
    stream: &mut RlpStream,
)
{
    let mut s = RlpStream::new();
    hash256rlp(input, pre_len, &mut s);
    let out = s.out();
    match out.len() {
        0..=31 => stream.append_raw(&out, 1),
        _ => stream.append(&blake2b(out)),
    };
}

#[test]
fn test_nibbles() {
    let v = vec![0x31, 0x23, 0x45];
    let e = vec![3, 1, 2, 3, 4, 5];
    assert_eq!(as_nibbles(&v), e);

    // A => 65 => 0x41 => [4, 1]
    let v: Vec<u8> = From::from("A");
    let e = vec![4, 1];
    assert_eq!(as_nibbles(&v), e);
}
