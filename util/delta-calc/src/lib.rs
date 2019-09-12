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

pub fn calculate_delta(difficulty: U256, seed: &[u8], stake: BigUint) -> u64 {
    let hash_of_seed = blake2b(&seed[..]);
    trace!(target: "delta_calc", "difficulty: {:?}, hash: {}, stake: {}",
           difficulty.as_u64(), hash_of_seed, stake);

    let u = LN_BOUNDARY
        .subtruct(&FixedPoint::ln(&hash_of_seed.into()))
        .expect("H256 should smaller than 2^256");
    let delta: BigUint = u.multiply_uint(difficulty.into()).to_big_uint() / stake;
    trace!(target: "delta_calc", "delta: {}", delta);
    // use 1000000000 when delta overflow in to_u64()
    // there must be a new block generated during this 1000000000 seconds
    ::std::cmp::max(1u64, delta.to_u64().unwrap_or(1000000000u64))
}
