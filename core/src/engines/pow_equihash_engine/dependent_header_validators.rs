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

use header::Header;
use error::{BlockError, Error};
use unexpected::{Mismatch, OutOfBounds};
// use key::public_to_address_ed25519;
// use rcrypto::ed25519::verify;
// use aion_types::{H256, Address};

pub trait DependentHeaderValidator {
    fn validate(&self, header: &Header, dependent_header: &Header) -> Result<(), Error>;
}

pub struct NumberValidator;
impl DependentHeaderValidator for NumberValidator {
    fn validate(&self, header: &Header, dependent_header: &Header) -> Result<(), Error> {
        let number = header.number();
        let parent_number = dependent_header.number();
        if number != parent_number + 1 {
            trace!(target:"equihash",
                   "blockNumber ({}) is not equal to parentBlock number + 1 ({})", number, parent_number);
            return Err(BlockError::InvalidNumber(Mismatch {
                expected: parent_number + 1,
                found: number,
            })
            .into());
        }
        Ok(())
    }
}

pub struct TimestampValidator;
impl DependentHeaderValidator for TimestampValidator {
    fn validate(&self, header: &Header, dependent_header: &Header) -> Result<(), Error> {
        let current_timestamp = header.timestamp();
        let parent_timestamp = dependent_header.timestamp();
        if current_timestamp <= parent_timestamp {
            error!(target: "equihash", "timestamp ({}) is not greater than parent timestamp ({})", current_timestamp, parent_timestamp);
            return Err(BlockError::InvalidTimestamp(OutOfBounds {
                min: Some(parent_timestamp),
                max: None,
                found: current_timestamp,
            })
            .into());
        }
        Ok(())
    }
}

pub struct PoSValidator;
impl DependentHeaderValidator for PoSValidator {
    fn validate(&self, _header: &Header, _dependent_header: &Header) -> Result<(), Error> {
        // TODO-Unity: Java used 64 bytes signature and the signer of the seed does not match the block's author. So we disable the PoS validation for now.
        // // Get seal, check seal length
        // let seal = header.seal();
        // if seal.len() != 2 {
        //     error!(target: "pos", "seal length != 2");
        //     return Err(BlockError::InvalidSealArity(Mismatch {
        //         expected: 2,
        //         found: seal.len(),
        //     })
        //     .into());
        // }

        // // Get seed and signature
        // let signature = &seal[0];
        // let seed = &seal[1];
        // let parent_seed = dependent_header
        //     .seal()
        //     .get(1)
        //     .expect("parent pos block should have a seed");

        // // Verify seed
        // let public_from_seed = &seed[..32];
        // let sig_from_seed = &seed[32..96];
        // if !verify(&parent_seed, public_from_seed, sig_from_seed) {
        //     return Err(BlockError::InvalidSeal.into());
        // }
        // let author_from_seed: Address = public_to_address_ed25519(&H256::from(public_from_seed));

        // // Verify block signature
        // let public_from_signature = &signature[..32];
        // let sig_from_signature = &signature[32..96];
        // if !verify(
        //     &header.bare_hash().0,
        //     public_from_signature,
        //     sig_from_signature,
        // ) {
        //     return Err(BlockError::InvalidSeal.into());
        // }
        // let author_from_signature: Address =
        //     public_to_address_ed25519(&H256::from(public_from_signature));

        // // Verify seed and block signature are the same as the block producer
        // if (&author_from_seed, &author_from_signature) != (&author_from_signature, header.author())
        // {
        //     return Err(BlockError::InvalidSeal.into());
        // }

        // Verify timestamp
        // TODO-Unity: To verify the timestamp with (stake, seed, difficulty, parent_timestamp)

        Ok(())
    }
}
