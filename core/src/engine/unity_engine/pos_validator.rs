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

use crate::header::{Header, SealType};
use crate::types::error::{BlockError, Error};
use unexpected::Mismatch;
use rcrypto::ed25519::verify;
use num::Zero;
use num_bigint::BigUint;
use delta_calc::calculate_delta;

pub struct PoSValidator;
impl PoSValidator {
    pub fn validate(
        header: &Header,
        parent_header: &Header,
        grand_parent_header: Option<&Header>,
        stake: Option<BigUint>,
    ) -> Result<(), Error>
    {
        // Return error if seal type is not PoS
        if header.seal_type() != &Some(SealType::PoS) {
            error!(target: "pos", "block seal type is not PoS");
            return Err(BlockError::InvalidPoSSealType.into());
        }

        // Return error if stake is none or 0
        let stake: BigUint = stake.unwrap_or(BigUint::from(0u32));
        if stake.is_zero() {
            error!(target: "pos", "pos block producer's stake is null or 0");
            return Err(BlockError::NullStake.into());
        }

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
        let parent_seed = grand_parent_header.map_or(vec![0u8; 64], |h| {
            if h.seal_type() == &Some(SealType::PoS) {
                h.seal()
                    .get(0)
                    .expect("parent pos block should have a seed")
                    .clone()
            } else {
                vec![0u8; 64]
            }
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
        // Signer and coinbase can be different
        // let signer: Address = public_to_address_ed25519(&H256::from(pk.as_slice()));
        // if &signer != header.author() {
        //     return Err(BlockError::InvalidPoSAuthor.into());
        // }

        // Verify timestamp
        let difficulty = header.difficulty().clone();
        let timestamp = header.timestamp();
        let parent_timestamp = parent_header.timestamp();

        let delta_uint = calculate_delta(difficulty, &seed, stake.clone());

        if timestamp - parent_timestamp != delta_uint {
            Err(BlockError::InvalidPoSTimestamp(timestamp, parent_timestamp, delta_uint).into())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{Header, SealType};
    use crate::types::error::{Error, BlockError};
    use unexpected::Mismatch;
    use aion_types::{U256, Address};

    #[test]
    fn test_pos_validator_invalid_seal_type() {
        let mut header = Header::default();
        header.set_seal_type(SealType::PoW);
        let parent_header = Header::default();
        let grand_parent_header = Header::default();
        let stake = None;
        let result =
            PoSValidator::validate(&header, &parent_header, Some(&grand_parent_header), stake);
        match result.err().unwrap() {
            Error::Block(error) => assert_eq!(error, BlockError::InvalidPoSSealType),
            _ => panic!("Should return block error."),
        };
    }

    #[test]
    fn test_pos_validator_null_stake() {
        let mut header = Header::default();
        header.set_seal_type(SealType::PoS);
        let parent_header = Header::default();
        let grand_parent_header = Header::default();
        let stake = None;
        let result =
            PoSValidator::validate(&header, &parent_header, Some(&grand_parent_header), stake);
        match result.err().unwrap() {
            Error::Block(error) => assert_eq!(error, BlockError::NullStake),
            _ => panic!("Should return block error."),
        };
    }

    #[test]
    fn test_pos_validator_zero_stake() {
        let mut header = Header::default();
        header.set_seal_type(SealType::PoS);
        let parent_header = Header::default();
        let grand_parent_header = Header::default();
        let stake = Some(BigUint::from(0u64));
        let result =
            PoSValidator::validate(&header, &parent_header, Some(&grand_parent_header), stake);
        match result.err().unwrap() {
            Error::Block(error) => assert_eq!(error, BlockError::NullStake),
            _ => panic!("Should return block error."),
        };
    }

    #[test]
    fn test_pos_validator_invalid_seal_length() {
        let mut header = Header::default();
        header.set_seal_type(SealType::PoS);
        let mut seal = Vec::with_capacity(2);
        seal.push(vec![0u8; 64]);
        seal.push(vec![0u8; 64]);
        header.set_seal(seal);
        let parent_header = Header::default();
        let grand_parent_header = Header::default();
        let stake = Some(BigUint::from(1u64));
        let result =
            PoSValidator::validate(&header, &parent_header, Some(&grand_parent_header), stake);
        match result.err().unwrap() {
            Error::Block(error) => {
                assert_eq!(
                    error,
                    BlockError::InvalidSealArity(Mismatch {
                        expected: 3,
                        found: 2,
                    })
                )
            }
            _ => panic!("Should return block error."),
        };
    }

    #[test]
    fn test_pos_validator_invalid_seed_first_pos() {
        let mut header = Header::default();
        header.set_seal_type(SealType::PoS);
        let mut seal = Vec::with_capacity(3);
        seal.push(vec![0u8; 64]);
        seal.push(vec![0u8; 64]);
        seal.push(vec![
            6, 147, 70, 202, 119, 21, 45, 62, 66, 177, 99, 8, 38, 254, 239, 54, 86, 131, 3, 140,
            59, 0, 255, 32, 176, 234, 66, 215, 193, 33, 250, 159,
        ]);
        header.set_seal(seal);
        let parent_header = Header::default();
        let grand_parent_header = Header::default();
        let stake = Some(BigUint::from(1u64));
        let result =
            PoSValidator::validate(&header, &parent_header, Some(&grand_parent_header), stake);
        match result.err().unwrap() {
            Error::Block(error) => assert_eq!(error, BlockError::InvalidPoSSeed),
            _ => panic!("Should return block error."),
        };
    }

    // #[test]
    // fn test_pos_validator_invalid_author() {
    //     let mut header = Header::default();
    //     header.set_seal_type(SealType::PoS);
    //     let mut seal = Vec::with_capacity(3);
    //     seal.push(vec![
    //         7, 240, 237, 211, 34, 55, 220, 1, 14, 9, 46, 39, 197, 62, 146, 106, 191, 19, 97, 18,
    //         151, 7, 243, 94, 161, 254, 84, 212, 101, 154, 128, 225, 27, 188, 162, 13, 213, 93, 220,
    //         86, 68, 73, 251, 180, 158, 144, 248, 78, 210, 230, 20, 151, 147, 83, 19, 207, 138, 88,
    //         39, 29, 28, 15, 4, 0,
    //     ]);
    //     seal.push(vec![
    //         80, 220, 254, 84, 10, 2, 113, 162, 173, 189, 105, 4, 138, 68, 114, 254, 248, 110, 55,
    //         179, 146, 62, 196, 50, 132, 109, 203, 233, 246, 69, 160, 1, 18, 199, 70, 137, 103, 173,
    //         159, 222, 157, 31, 77, 198, 196, 138, 254, 27, 43, 69, 187, 236, 107, 106, 169, 242,
    //         17, 87, 10, 58, 174, 11, 31, 10,
    //     ]);
    //     seal.push(vec![
    //         6, 147, 70, 202, 119, 21, 45, 62, 66, 177, 99, 8, 38, 254, 239, 54, 86, 131, 3, 140,
    //         59, 0, 255, 32, 176, 234, 66, 215, 193, 33, 250, 159,
    //     ]);
    //     header.set_seal(seal);
    //     header.set_author(Address::from(
    //         "0xa02df9004be3c4a20aeb50c459212412b1d0a58da3e1ac70ba74dde6b4accf4a",
    //     ));

    //     let mut parent_header = Header::default();
    //     parent_header.set_seal_type(SealType::PoS);
    //     let mut parent_seal = Vec::with_capacity(3);
    //     parent_seal.push(vec![
    //         97, 14, 49, 52, 139, 205, 231, 71, 40, 173, 229, 105, 74, 96, 74, 12, 232, 89, 79, 114,
    //         158, 9, 23, 133, 166, 22, 217, 233, 27, 73, 107, 207, 21, 245, 107, 127, 40, 197, 235,
    //         162, 78, 39, 142, 45, 242, 219, 146, 162, 194, 95, 250, 109, 207, 171, 133, 190, 243,
    //         119, 21, 14, 149, 29, 222, 3,
    //     ]);
    //     parent_seal.push(vec![0u8; 64]);
    //     parent_seal.push(vec![0u8; 32]);
    //     parent_header.set_seal(parent_seal);

    //        let stake = Some(BigUint::from(1u64));
    //        let result = PoSValidator::validate(&header, Some(&parent_header), stake);
    //        match result.err().unwrap() {
    //            Error::Block(error) => assert_eq!(error, BlockError::InvalidPoSAuthor),
    //            _ => panic!("Should return block error."),
    //        };
    //    }

    #[test]
    fn test_pos_validator_invalid_timestamp() {
        let mut header = Header::default();
        header.set_seal_type(SealType::PoS);
        let mut seal = Vec::with_capacity(3);
        seal.push(vec![
            7, 240, 237, 211, 34, 55, 220, 1, 14, 9, 46, 39, 197, 62, 146, 106, 191, 19, 97, 18,
            151, 7, 243, 94, 161, 254, 84, 212, 101, 154, 128, 225, 27, 188, 162, 13, 213, 93, 220,
            86, 68, 73, 251, 180, 158, 144, 248, 78, 210, 230, 20, 151, 147, 83, 19, 207, 138, 88,
            39, 29, 28, 15, 4, 0,
        ]);
        seal.push(vec![
            75, 86, 53, 76, 103, 121, 157, 135, 221, 231, 209, 80, 10, 104, 17, 208, 118, 46, 122,
            205, 174, 252, 139, 185, 59, 105, 162, 76, 223, 96, 147, 251, 102, 114, 214, 11, 158,
            207, 155, 87, 102, 190, 126, 100, 216, 14, 71, 62, 196, 75, 160, 232, 27, 39, 217, 236,
            178, 183, 195, 204, 11, 13, 34, 4,
        ]);
        seal.push(vec![
            6, 147, 70, 202, 119, 21, 45, 62, 66, 177, 99, 8, 38, 254, 239, 54, 86, 131, 3, 140,
            59, 0, 255, 32, 176, 234, 66, 215, 193, 33, 250, 159,
        ]);
        header.set_seal(seal);
        header.set_author(Address::from(
            "0xa02df9004be3c4a20aeb50c459212412b1d0a58da3e1ac70ba74dde6b4accf4b",
        ));
        header.set_difficulty(U256::from(1_000_000u64));
        header.set_timestamp(15u64);

        let mut parent_header = Header::default();
        parent_header.set_timestamp(1u64);
        let mut grand_parent_header = Header::default();
        grand_parent_header.set_seal_type(SealType::PoS);
        let mut grand_parent_seal = Vec::with_capacity(3);
        grand_parent_seal.push(vec![
            97, 14, 49, 52, 139, 205, 231, 71, 40, 173, 229, 105, 74, 96, 74, 12, 232, 89, 79, 114,
            158, 9, 23, 133, 166, 22, 217, 233, 27, 73, 107, 207, 21, 245, 107, 127, 40, 197, 235,
            162, 78, 39, 142, 45, 242, 219, 146, 162, 194, 95, 250, 109, 207, 171, 133, 190, 243,
            119, 21, 14, 149, 29, 222, 3,
        ]);
        grand_parent_seal.push(vec![0u8; 64]);
        grand_parent_seal.push(vec![0u8; 32]);
        grand_parent_header.set_seal(grand_parent_seal);

        let stake = Some(BigUint::from(10_000u64));
        let result =
            PoSValidator::validate(&header, &parent_header, Some(&grand_parent_header), stake);
        match result.err().unwrap() {
            Error::Block(error) => assert_eq!(error, BlockError::InvalidPoSTimestamp(15, 1, 15)),
            _ => panic!("Should return block error."),
        };
    }

    #[test]
    fn test_pos_validator_valid_first_pos() {
        let mut header = Header::default();
        header.set_seal_type(SealType::PoS);
        let mut seal = Vec::with_capacity(3);
        seal.push(vec![
            97, 14, 49, 52, 139, 205, 231, 71, 40, 173, 229, 105, 74, 96, 74, 12, 232, 89, 79, 114,
            158, 9, 23, 133, 166, 22, 217, 233, 27, 73, 107, 207, 21, 245, 107, 127, 40, 197, 235,
            162, 78, 39, 142, 45, 242, 219, 146, 162, 194, 95, 250, 109, 207, 171, 133, 190, 243,
            119, 21, 14, 149, 29, 222, 3,
        ]);
        seal.push(vec![
            139, 247, 58, 87, 39, 2, 111, 203, 1, 80, 41, 165, 111, 124, 62, 104, 254, 162, 65,
            105, 211, 140, 75, 219, 165, 30, 54, 120, 5, 141, 182, 119, 3, 107, 15, 160, 71, 136,
            27, 243, 232, 34, 66, 112, 130, 43, 96, 224, 2, 13, 146, 53, 231, 121, 142, 73, 131,
            12, 97, 216, 240, 148, 90, 1,
        ]);
        seal.push(vec![
            6, 147, 70, 202, 119, 21, 45, 62, 66, 177, 99, 8, 38, 254, 239, 54, 86, 131, 3, 140,
            59, 0, 255, 32, 176, 234, 66, 215, 193, 33, 250, 159,
        ]);
        header.set_seal(seal);
        header.set_author(Address::from(
            "0xa02df9004be3c4a20aeb50c459212412b1d0a58da3e1ac70ba74dde6b4accf4b",
        ));
        header.set_difficulty(U256::from(1_000_000u64));
        header.set_timestamp(25u64);
        let parent_header = Header::default();
        let grand_parent_header = Header::default();
        let stake = Some(BigUint::from(10_000u64));
        let result =
            PoSValidator::validate(&header, &parent_header, Some(&grand_parent_header), stake);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pos_validator_valid() {
        let mut header = Header::default();
        header.set_seal_type(SealType::PoS);
        let mut seal = Vec::with_capacity(3);
        seal.push(vec![
            7, 240, 237, 211, 34, 55, 220, 1, 14, 9, 46, 39, 197, 62, 146, 106, 191, 19, 97, 18,
            151, 7, 243, 94, 161, 254, 84, 212, 101, 154, 128, 225, 27, 188, 162, 13, 213, 93, 220,
            86, 68, 73, 251, 180, 158, 144, 248, 78, 210, 230, 20, 151, 147, 83, 19, 207, 138, 88,
            39, 29, 28, 15, 4, 0,
        ]);
        seal.push(vec![
            75, 86, 53, 76, 103, 121, 157, 135, 221, 231, 209, 80, 10, 104, 17, 208, 118, 46, 122,
            205, 174, 252, 139, 185, 59, 105, 162, 76, 223, 96, 147, 251, 102, 114, 214, 11, 158,
            207, 155, 87, 102, 190, 126, 100, 216, 14, 71, 62, 196, 75, 160, 232, 27, 39, 217, 236,
            178, 183, 195, 204, 11, 13, 34, 4,
        ]);
        seal.push(vec![
            6, 147, 70, 202, 119, 21, 45, 62, 66, 177, 99, 8, 38, 254, 239, 54, 86, 131, 3, 140,
            59, 0, 255, 32, 176, 234, 66, 215, 193, 33, 250, 159,
        ]);
        header.set_seal(seal);
        header.set_author(Address::from(
            "0xa02df9004be3c4a20aeb50c459212412b1d0a58da3e1ac70ba74dde6b4accf4b",
        ));
        header.set_difficulty(U256::from(1_000_000u64));
        header.set_timestamp(15u64);

        let parent_header = Header::default();
        let mut grand_parent_header = Header::default();
        grand_parent_header.set_seal_type(SealType::PoS);
        let mut grand_parent_seal = Vec::with_capacity(3);
        grand_parent_seal.push(vec![
            97, 14, 49, 52, 139, 205, 231, 71, 40, 173, 229, 105, 74, 96, 74, 12, 232, 89, 79, 114,
            158, 9, 23, 133, 166, 22, 217, 233, 27, 73, 107, 207, 21, 245, 107, 127, 40, 197, 235,
            162, 78, 39, 142, 45, 242, 219, 146, 162, 194, 95, 250, 109, 207, 171, 133, 190, 243,
            119, 21, 14, 149, 29, 222, 3,
        ]);
        grand_parent_seal.push(vec![0u8; 64]);
        grand_parent_seal.push(vec![0u8; 32]);
        grand_parent_header.set_seal(grand_parent_seal);

        let stake = Some(BigUint::from(10_000u64));
        let result =
            PoSValidator::validate(&header, &parent_header, Some(&grand_parent_header), stake);
        assert!(result.is_ok());
    }
}
