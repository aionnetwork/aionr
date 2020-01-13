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
#[macro_use]
extern crate crunchy;
extern crate aion_types;

use std::{hash, mem};
use std::collections::{HashMap, HashSet};
use aion_types::H256;

/// Specialized version of `HashMap` with H256 keys and fast hashing function.
pub type H256FastMap<T> = HashMap<H256, T, hash::BuildHasherDefault<PlainHasher>>;
/// Specialized version of `HashSet` with H256 keys and fast hashing function.
pub type H256FastSet = HashSet<H256, hash::BuildHasherDefault<PlainHasher>>;

/// Hasher that just takes 8 bytes of the provided value.
/// May only be used for keys which are 32 bytes.
#[derive(Default)]
pub struct PlainHasher {
    prefix: u64,
}

impl hash::Hasher for PlainHasher {
    #[inline]
    fn finish(&self) -> u64 { self.prefix }

    #[inline]
    #[allow(unused_assignments)]
    fn write(&mut self, bytes: &[u8]) {
        debug_assert!(bytes.len() == 32);

        unsafe {
            let mut bytes_ptr = bytes.as_ptr();
            let prefix_u8: &mut [u8; 8] = mem::transmute(&mut self.prefix);
            let mut prefix_ptr = prefix_u8.as_mut_ptr();

            unroll! {
                for _i in 0..8 {
                    *prefix_ptr ^= (*bytes_ptr ^ *bytes_ptr.offset(8)) ^ (*bytes_ptr.offset(16) ^ *bytes_ptr.offset(24));

                    bytes_ptr = bytes_ptr.offset(1);
                    prefix_ptr = prefix_ptr.offset(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "benches")]
    use std::time::Instant;
    use std::hash::Hasher;
    use super::PlainHasher;
    #[cfg(feature = "benches")]
    use std::collections::hash_map::DefaultHasher;

    #[test]
    fn it_works() {
        let mut bytes = [32u8; 32];
        bytes[0] = 15;
        let mut hasher = PlainHasher::default();
        hasher.write(&bytes);
        assert_eq!(hasher.prefix, 47);
    }

    #[test]
    #[cfg(feature = "benches")]
    fn benchtest_write_plain_hasher() {
        let count = 1000;
        let time = Instant::now();

        for _ in 0..count {
            let n: u8 = 100;
            (0..n).fold(PlainHasher::default(), |mut old, new| {
                let bb = [new; 32];
                old.write(&bb as &[u8]);
                old
            });
        }

        let took = time.elapsed();
        println!(
            "[benchtest_write_plain_hasher] write plain hasher (ns/call): {}",
            (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
        );
    }

    #[test]
    #[cfg(feature = "benches")]
    fn benchtest_write_default_hasher() {
        let count = 1000;
        let time = Instant::now();

        for _ in 0..count {
            let n: u8 = 100;
            (0..n).fold(DefaultHasher::default(), |mut old, new| {
                let bb = [new; 32];
                old.write(&bb as &[u8]);
                old
            });
        }

        let took = time.elapsed();
        println!(
            "[benchtest_write_default_hasher] write default hasher (ns/call): {}",
            (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
        );
    }
}
