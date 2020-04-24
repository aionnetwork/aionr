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

use header::{Header, SealType};
use types::error::{BlockError, Error};
use unexpected::Mismatch;
use rcrypto::{ed25519, ecvrf};
use acore_bytes::{slice_to_array_80, slice_to_array_32};
use num::Zero;
use num_bigint::BigUint;
use delta_calc::calculate_delta;
use blake2b::blake2b;
use key::public_to_address_ed25519;
use aion_types::{H256, Address};

pub struct PoSValidator;
impl PoSValidator {
    pub fn validate(
        header: &Header,
        parent_header: &Header,
        grand_parent_header: Option<&Header>,
        stake: Option<BigUint>,
        unity_hybrid_seed_update: bool,
        unity_ecvrf_seed_update: bool,
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

        // Get seed, signature and public key, and check their length
        let mut seed = seal[0].clone();
        if seed.len() != if unity_ecvrf_seed_update { 80 } else { 64 } {
            return Err(BlockError::InvalidPoSSeed.into());
        }
        let signature = &seal[1];
        if signature.len() != 64 {
            return Err(BlockError::InvalidPoSSignature.into());
        }
        let pk: &[u8; 32] = match slice_to_array_32(&seal[2]) {
            Some(pk) => pk,
            None => return Err(BlockError::InvalidPoSPublicKey.into()),
        };

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
        if !Self::validate_seed(
            unity_hybrid_seed_update,
            unity_ecvrf_seed_update,
            &seed,
            &parent_seed,
            pk,
            parent_header,
        ) {
            return Err(BlockError::InvalidPoSSeed.into());
        }

        // Unity-3: calculate the real seed from the proof
        if unity_ecvrf_seed_update {
            if let Some(proof) = slice_to_array_80(&seed.clone()) {
                seed = match ecvrf::proof_to_hash(proof) {
                    Ok(output) => output.to_vec(),
                    Err(_) => return Err(BlockError::InvalidPoSSeed.into()),
                }
            }
        }

        // Verify block signature
        if !ed25519::verify(&header.mine_hash().0, pk, signature) {
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

    // Validate the seed of the pos seal
    fn validate_seed(
        unity_hybrid_seed_update: bool,
        unity_ecvrf_seed_update: bool,
        seed: &[u8],
        grand_parent_seed: &[u8],
        pk: &[u8; 32],
        parent_header: &Header,
    ) -> bool
    {
        // Unity-3: ecvrf seed is generated from the proof which needs to be validated
        if unity_ecvrf_seed_update {
            if let Some(proof) = slice_to_array_80(&seed) {
                if let Some(parent_proof) = slice_to_array_80(&grand_parent_seed) {
                    match ecvrf::proof_to_hash(parent_proof) {
                        Ok(ref previous_seed) => ecvrf::verify(pk, proof, previous_seed).is_ok(),
                        Err(_) => false,
                    }
                } else if grand_parent_seed.len() == 64 {
                    // First block after ecvrf hard fork
                    ecvrf::verify(pk, proof, grand_parent_seed).is_ok()
                } else {
                    false
                }
            } else {
                false
            }
        }
        // Unity-2: validate the new hybrid seed
        else if unity_hybrid_seed_update {
            let mut hybrid_seed: Vec<u8> = Vec::new();
            let signing_address: Address = public_to_address_ed25519(&H256::from(pk.as_ref()));
            let parent_mine_hash: H256 = parent_header.mine_hash();
            let parent_seal = parent_header.seal();
            if parent_seal.len() != 2 {
                error!(target: "pos", "parent seal length != 2");
                return false;
            }
            let parent_nonce: &[u8] = &parent_seal[0];
            // X = PoS-seed_n-1 || Signing-addr || Pow-HeaderHashForMiners_n-1 || Pow-nonce_n-1
            hybrid_seed.extend(grand_parent_seed);
            hybrid_seed.extend(&signing_address.to_vec());
            hybrid_seed.extend(&parent_mine_hash.to_vec());
            hybrid_seed.extend(parent_nonce);
            // left = x || 0
            let mut hybrid_left: Vec<u8> = Vec::new();
            hybrid_left.extend(&hybrid_seed);
            hybrid_left.extend(&[0u8]);
            // right = x || 1
            let mut hybrid_right: Vec<u8> = Vec::new();
            hybrid_right.extend(&hybrid_seed);
            hybrid_right.extend(&[1u8]);
            // PoS-seed_n = Blake2b(X || 0) || Blake2b(X || 1)
            let seed_left: H256 = blake2b(&hybrid_left);
            let seed_right: H256 = blake2b(&hybrid_right);
            let mut new_seed: Vec<u8> = Vec::new();
            new_seed.extend(&seed_left.to_vec());
            new_seed.extend(&seed_right.to_vec());
            debug!(target: "pos", "block {:?}, hybrid_left {:?}, hybrid_right {:?}, seed_left {:?},
                seed_right {:?}, new_seed {:?}, seed {:?}",
                parent_header.number() + 1, hybrid_left, hybrid_right, seed_left,
                seed_right, new_seed, seed);
            seed == new_seed.as_slice()
        }
        // Old unity seed
        else {
            ed25519::verify(grand_parent_seed, pk, seed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use header::{Header, SealType};
    use types::error::{Error, BlockError};
    use unexpected::Mismatch;
    use aion_types::{U256, Address};

    #[test]
    fn test_pos_validator_invalid_seal_type() {
        let mut header = Header::default();
        header.set_seal_type(SealType::PoW);
        let parent_header = Header::default();
        let grand_parent_header = Header::default();
        let stake = None;
        let result = PoSValidator::validate(
            &header,
            &parent_header,
            Some(&grand_parent_header),
            stake,
            false,
            false,
        );
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
        let result = PoSValidator::validate(
            &header,
            &parent_header,
            Some(&grand_parent_header),
            stake,
            false,
            false,
        );
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
        let result = PoSValidator::validate(
            &header,
            &parent_header,
            Some(&grand_parent_header),
            stake,
            false,
            false,
        );
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
        let result = PoSValidator::validate(
            &header,
            &parent_header,
            Some(&grand_parent_header),
            stake,
            false,
            false,
        );
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
        let result = PoSValidator::validate(
            &header,
            &parent_header,
            Some(&grand_parent_header),
            stake,
            false,
            false,
        );
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
        let result = PoSValidator::validate(
            &header,
            &parent_header,
            Some(&grand_parent_header),
            stake,
            false,
            false,
        );
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
        let result = PoSValidator::validate(
            &header,
            &parent_header,
            Some(&grand_parent_header),
            stake,
            false,
            false,
        );
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
        let result = PoSValidator::validate(
            &header,
            &parent_header,
            Some(&grand_parent_header),
            stake,
            false,
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_pos_validator_invalid_hybrid_seed() {
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
        let result = PoSValidator::validate(
            &header,
            &parent_header,
            Some(&grand_parent_header),
            stake,
            true,
            false,
        );
        match result.err().unwrap() {
            Error::Block(error) => assert_eq!(error, BlockError::InvalidPoSSeed),
            _ => panic!("Should return block error."),
        };
    }

    #[test]
    fn test_pos_validator_valid_hybrid_seed() {
        let mut header = Header::default();
        header.set_seal_type(SealType::PoS);
        let mut seal = Vec::with_capacity(3);
        seal.push(vec![
            222, 28, 167, 23, 250, 67, 26, 227, 116, 151, 244, 74, 225, 248, 203, 141, 25, 107,
            111, 30, 147, 187, 179, 73, 211, 89, 172, 143, 131, 12, 56, 151, 63, 240, 93, 59, 125,
            85, 150, 229, 228, 254, 18, 16, 228, 135, 126, 39, 43, 67, 133, 239, 222, 171, 134,
            153, 238, 126, 213, 49, 88, 138, 99, 68,
        ]);
        seal.push(vec![
            121, 114, 2, 99, 60, 89, 198, 46, 48, 111, 67, 8, 77, 75, 179, 108, 207, 152, 51, 14,
            194, 200, 59, 155, 84, 175, 94, 28, 71, 59, 198, 119, 136, 56, 112, 189, 108, 126, 195,
            116, 212, 30, 135, 59, 128, 115, 252, 46, 123, 131, 121, 25, 22, 218, 124, 152, 225,
            210, 50, 10, 76, 79, 175, 1,
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
        header.set_timestamp(42u64);

        let mut parent_header = Header::default();
        let mut parent_seal = Vec::with_capacity(2);
        parent_seal.push(vec![0u8; 32]);
        parent_seal.push(vec![0u8; 32]);
        parent_header.set_seal(parent_seal);
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
        let result = PoSValidator::validate(
            &header,
            &parent_header,
            Some(&grand_parent_header),
            stake,
            true,
            false,
        );
        assert!(result.is_ok());
    }
}
