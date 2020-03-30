/*******************************************************************************
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

//! delta calculation. Used to calculate PoS block timestamp.

#![warn(unused_extern_crates)]

extern crate num_bigint;
extern crate fixed_point;
extern crate aion_types;
extern crate blake2b;
extern crate num_traits;
#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

use num_traits::ToPrimitive;
use aion_types::U256;
use blake2b::blake2b;
use num_bigint::BigUint;
use fixed_point::{FixedPoint,LogApproximator};

lazy_static! {
    static ref LN_BOUNDARY: FixedPoint = FixedPoint::ln(
        &BigUint::parse_bytes(
            b"10000000000000000000000000000000000000000000000000000000000000000",
            16,
        )
        .unwrap()
    );
}

/// Calculate delta with given parameters
///
/// # parameter
/// * difficulty: difficulty of current PoS block
/// * seed: seed of current PoS block
/// * stake: total stake of current PoS block author
///
/// `delta = difficulty * ( ln(2^256) - ln(hash(seed)) ) / stake`
pub fn calculate_delta(difficulty: U256, seed: &[u8], stake: BigUint) -> u64 {
    let hash_of_seed = blake2b(&seed[..]);
    trace!(target: "delta_calc", "difficulty: {:?}, hash: {}, stake: {}",
           difficulty, hash_of_seed, stake);

    let u = LN_BOUNDARY
        .subtract(&FixedPoint::ln(&hash_of_seed.into()))
        .expect("H256 should smaller than 2^256");
    let delta: BigUint = u.multiply_uint(difficulty.into()).to_big_uint() / stake;
    trace!(target: "delta_calc", "delta: {}", delta);
    // use 1000000000 when delta overflow in to_u64()
    // there must be a new block generated during this 1000000000 seconds
    ::std::cmp::max(1u64, delta.to_u64().unwrap_or(1000000000u64))
}

#[cfg(test)]
mod test {
    use super::*;
    fn ln_sub(hash_seed: &BigUint) -> Result<f64, String> {
        // convert BigUint to little endian bytes
        let hash_seed_bytes: Vec<u8> = hash_seed.to_bytes_le();

        // this is the length of the bytes we need to convert to double
        // we do not need full precision of BigInt, for two reasons:
        // 1.double only have 53-bit precision
        // 2.precision after 53-bit has little effect on the results after ln()
        const BYTES: usize = 12;

        // convert hash_seed_bytes to eps*(256)^l where 1 < eps < 256 , l = bytes_length - 1
        let start = if hash_seed_bytes.len() < BYTES {
            0
        } else {
            hash_seed_bytes.len() - BYTES
        };
        let mut eps_hash_seed: f64 = 0.0;
        for i in start..hash_seed_bytes.len() {
            eps_hash_seed = eps_hash_seed / 256f64 + (hash_seed_bytes[i] as f64);
        }

        // calculate ln(256)
        let ln_256: f64 = (256f64).ln();

        // return  (33 - l_hash_seed) * ln(256) - ln(eps_hash_seed)
        // 1 is eps of 2^256, and 33 is the length of 2^256
        // 2^256 = (2^8)^32, so it has a '1' and 32 '0's, so its length is 33
        Ok(ln_256 * ((33 - hash_seed_bytes.len()) as f64) - eps_hash_seed.ln())
    }

    #[test]
    fn test_calculate_delta() {
        let seed = [2u8; 64];
        let hash_of_seed = blake2b(&seed[..]);
        let ln_sub = ln_sub(&hash_of_seed.into()).unwrap();
        assert_eq!(
            calculate_delta(U256::from(10000u64), &seed, BigUint::from(333u64)),
            (10000f64 * ln_sub / 333f64) as u64
        );
    }
}
