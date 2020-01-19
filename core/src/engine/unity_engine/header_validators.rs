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
use std::time::{SystemTime, UNIX_EPOCH};

use crate::header::Header;
use equihash::EquihashValidator;
use blake2b::Blake2b;
use aion_types::U256;
use crate::types::error::{BlockError, Error};
use unexpected::{Mismatch, OutOfBounds};
use acore_bytes::to_hex;

/// Tolerance of future blocks with greater timestamp than local system time
const FUTURE_TIME_TOLERANCE: u64 = 1;

/// Header validator.
pub trait HeaderValidator {
    /// validate the given header.
    fn validate(&self, header: &Header) -> Result<(), Error>;
}

// commented temporarilly in case when we will use this validator strucutre.

/*
pub struct ExtraDataValidator {
    pub maximum_extra_data_size: usize,
}

impl HeaderValidator for ExtraDataValidator {

    fn validate(&self, header: &Header) -> Result<(), Error> {
        let extra_data = header.extra_data();
        let extra_data_size = extra_data.len();
        if extra_data_size > self.maximum_extra_data_size {
            trace!(target: "equihash", "extraData ({}) > MAXIMUM_EXTRA_DATA_SIZE ({})",
                   extra_data_size,
                   self.maximum_extra_data_size);
            return Err(BlockError::ExtraDataOutOfBounds(
                OutOfBounds {
                    min: None,
                    max: Some(self.maximum_extra_data_size),
                    found: extra_data_size
                }
            ).into());
        }
        Ok(())
    }
}
*/

pub struct EnergyConsumedValidator;
impl HeaderValidator for EnergyConsumedValidator {
    fn validate(&self, header: &Header) -> Result<(), Error> {
        let gas_limit = *header.gas_limit();
        let gas_used = *header.gas_used();
        if gas_used > gas_limit {
            error!(target: "equihash", "energy consumed ({}) > energy limit ({})", gas_used, gas_limit);
            return Err(BlockError::TooMuchGasUsed(OutOfBounds {
                min: None,
                max: Some(gas_limit),
                found: gas_used,
            })
            .into());
        }
        Ok(())
    }
}

pub struct EquihashSolutionValidator {
    pub solution_validator: EquihashValidator,
}
impl HeaderValidator for EquihashSolutionValidator {
    fn validate(&self, header: &Header) -> Result<(), Error> {
        let seal = header.seal();
        if seal.len() != 2 {
            error!(target: "equihash", "seal length != 2");
            return Err(BlockError::InvalidSealArity(Mismatch {
                expected: 2,
                found: seal.len(),
            })
            .into());
        }

        let nonce = &seal[0];
        let solution = &seal[1];
        let hash = header.mine_hash();
        if !self.solution_validator.is_valid_solution(
            solution.as_slice(),
            hash.as_ref(),
            nonce.as_slice(),
        ) {
            return Err(BlockError::InvalidSolution.into());
        }
        Ok(())
    }
}

pub struct POWValidator;
impl HeaderValidator for POWValidator {
    fn validate(&self, header: &Header) -> Result<(), Error> {
        let seal = header.seal();
        if seal.len() != 2 {
            error!(target: "equihash", "seal length != 2");
            return Err(BlockError::InvalidSealArity(Mismatch {
                expected: 2,
                found: seal.len(),
            })
            .into());
        }

        let hdr_bytes = header.mine_hash();
        debug!(target: "equihash", "mine_hash: {}", to_hex(hdr_bytes.as_ref()));
        let nonce = &seal[0];
        debug!(target: "equihash", "nonce: {}", to_hex(nonce.as_slice()));
        let solution = &seal[1];
        debug!(target: "equihash", "solution: {}", to_hex(solution.as_slice()));

        let boundary = header.boundary();
        debug!(target: "equihash", "boundary: {}", U256::from(boundary));
        let mut input: Vec<u8> = Vec::with_capacity(32 + 32 + 1408);
        input.extend_from_slice(hdr_bytes.as_ref());
        input.extend_from_slice(nonce.as_slice());
        input.extend_from_slice(solution.as_slice());
        let hash = U256::from(Blake2b::hash_256(input.as_slice()));
        debug!(target: "equihash", "hash: {}", hash);
        if hash >= U256::from(boundary) {
            error!(target: "equihash", "computed output ({}) violates boundary condition ({})", hash, U256::from(boundary));
            return Err(BlockError::ResultOutOfBounds(OutOfBounds {
                min: None,
                max: Some(U256::from(boundary)),
                found: hash,
            })
            .into());
        }
        Ok(())
    }
}

pub struct FutureTimestampValidator;
impl HeaderValidator for FutureTimestampValidator {
    fn validate(&self, header: &Header) -> Result<(), Error> {
        let timestamp = header.timestamp();
        let timestamp_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if timestamp > timestamp_now + FUTURE_TIME_TOLERANCE {
            debug!(target: "validator", "block timestamp ({}) > local system time ({}) + {}", timestamp, timestamp_now, FUTURE_TIME_TOLERANCE);
            return Err(BlockError::InvalidFutureTimestamp(OutOfBounds {
                min: None,
                max: Some(timestamp_now + FUTURE_TIME_TOLERANCE),
                found: timestamp,
            })
            .into());
        }
        Ok(())
    }
}
