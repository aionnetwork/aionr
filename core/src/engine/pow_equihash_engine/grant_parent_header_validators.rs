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

use super::DifficultyCalc;
use types::error::{Error, BlockError};
use header::Header;
use unexpected::{Mismatch};

pub trait GrantParentHeaderValidator {
    fn validate(
        &self,
        header: &Header,
        parent_header: &Header,
        grant_parent_header: Option<&Header>,
    ) -> Result<(), Error>;
}

pub struct DifficultyValidator<'a> {
    pub difficulty_calc: &'a DifficultyCalc,
}

impl<'a> GrantParentHeaderValidator for DifficultyValidator<'a> {
    fn validate(
        &self,
        header: &Header,
        parent_header: &Header,
        grant_parent_header: Option<&Header>,
    ) -> Result<(), Error>
    {
        let difficulty = *header.difficulty();
        let parent_difficulty = *parent_header.difficulty();
        if parent_header.number() == 0u64 {
            if difficulty != parent_difficulty {
                return Err(BlockError::InvalidDifficulty(Mismatch {
                    expected: parent_difficulty,
                    found: difficulty,
                })
                .into());
            } else {
                return Ok(());
            }
        }

        if grant_parent_header.is_none() {
            panic!(
                "non-1st block must have grant parent. block num: {}",
                header.number()
            );
        } else {
            let calc_difficulty = self.difficulty_calc.calculate_difficulty(
                header,
                parent_header,
                grant_parent_header,
            );
            if difficulty != calc_difficulty {
                Err(BlockError::InvalidDifficulty(Mismatch {
                    expected: calc_difficulty,
                    found: difficulty,
                })
                .into())
            } else {
                Ok(())
            }
        }
    }
}
