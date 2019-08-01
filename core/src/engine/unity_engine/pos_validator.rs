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

use std::cmp::max;

use header::{Header, SealType};
use types::error::{BlockError, Error};
use unexpected::Mismatch;
use key::public_to_address_ed25519;
use rcrypto::ed25519::verify;
use aion_types::{H256, Address};
use blake2b::blake2b;
use num_bigint::BigUint;

pub struct PoSValidator;
impl PoSValidator {
    pub fn validate(
        header: &Header,
        seal_parent_header: Option<&Header>,
        stake: Option<u64>,
    ) -> Result<(), Error>
    {
        // Return error if seal type is not PoS
        if header.seal_type() != &Some(SealType::PoS) {
            error!(target: "pos", "block seal type is not PoS");
            return Err(BlockError::InvalidPoSSealType.into());
        }

        // Return error if stake is none or 0
        let stake: u64 = match stake {
            Some(stake) if stake > 0 => stake,
            _ => {
                error!(target: "pos", "pos block producer's stake is null or 0");
                return Err(BlockError::NullStake.into());
            }
        };

        // Get seal, check seal length
        let seal = header.seal();
        if seal.len() != 3 {
            error!(target: "pos", "seal length != 3");
            return Err(BlockError::InvalidSealArity(Mismatch {
                expected: 3,
                found: seal.len(),
            })
            .into());
        }

        // Get seed and signature
        let seed = &seal[0];
        let signature = &seal[1];
        let pk = &seal[2];
        let parent_seed = seal_parent_header.map_or(vec![0u8; 64], |h| {
            h.seal()
                .get(0)
                .expect("parent pos block should have a seed")
                .clone()
        });

        // Verify seed
        if !verify(&parent_seed, pk, seed) {
            return Err(BlockError::InvalidPoSSeed.into());
        }

        // Verify block signature
        if !verify(&header.mine_hash().0, pk, signature) {
            return Err(BlockError::InvalidPoSSignature.into());
        }

        // Verify the signer of the seed and the signature are the same as the block producer
        let signer: Address = public_to_address_ed25519(&H256::from(pk.as_slice()));
        if &signer != header.author() {
            return Err(BlockError::InvalidPoSAuthor.into());
        }

        // Verify timestamp
        let difficulty = header.difficulty();
        let timestamp = header.timestamp();
        let parent_timestamp = seal_parent_header.map_or(0, |h| h.timestamp());
        let hash_of_seed = blake2b(&seed[..]);
        let a = BigUint::parse_bytes(
            b"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            16,
        )
        .unwrap();
        let b = BigUint::from_bytes_be(&hash_of_seed[..]);
        let u = ln(&a).unwrap() - ln(&b).unwrap();
        let delta = (difficulty.as_u64() as f64) * u / (stake as f64);
        let delta_uint: u64 = max(1u64, delta as u64);
        if timestamp - parent_timestamp < delta_uint {
            Err(BlockError::InvalidPoSTimestamp(timestamp, parent_timestamp, delta_uint).into())
        } else {
            Ok(())
        }
    }
}

// TODO-Unity: to do this better
fn ln(x: &BigUint) -> Result<f64, String> {
    let x: Vec<u8> = x.to_bytes_le();

    const BYTES: usize = 12;
    let start = if x.len() < BYTES { 0 } else { x.len() - BYTES };

    let mut n: f64 = 0.0;
    for i in start..x.len() {
        n = n / 256f64 + (x[i] as f64);
    }
    let ln_256: f64 = (256f64).ln();

    Ok(n.ln() + ln_256 * ((x.len() - 1) as f64))
}
