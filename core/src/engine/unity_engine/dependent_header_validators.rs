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
use types::error::{BlockError, Error};
use unexpected::{Mismatch, OutOfBounds};

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

// AION 2.0
// Validate a block has different seal type with its parent
pub struct SealTypeValidator;
impl DependentHeaderValidator for SealTypeValidator {
    fn validate(&self, header: &Header, dependent_header: &Header) -> Result<(), Error> {
        let current_seal_type = header.seal_type();
        let parent_seal_type = dependent_header.seal_type();
        if current_seal_type == parent_seal_type {
            error!(target: "unity", "current block's seal type ({:?}) is the same as its parent's seal type ({:?})", current_seal_type, parent_seal_type);
            return Err(BlockError::InvalidSealType.into());
        }
        Ok(())
    }
}
