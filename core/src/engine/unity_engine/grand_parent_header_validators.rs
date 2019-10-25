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
use client::BlockChainClient;

pub trait GrandParentHeaderValidator {
    fn validate(
        &self,
        header: &Header,
        parent_header: &Header,
        grand_parent_header: Option<&Header>,
        great_grand_parent_header: Option<&Header>,
        client: &BlockChainClient,
    ) -> Result<(), Error>;
}

pub struct DifficultyValidator<'a> {
    pub difficulty_calc: &'a DifficultyCalc,
}

impl<'a> GrandParentHeaderValidator for DifficultyValidator<'a> {
    fn validate(
        &self,
        header: &Header,
        parent_header: &Header,
        grand_parent_header: Option<&Header>,
        great_grand_parent_header: Option<&Header>,
        client: &BlockChainClient,
    ) -> Result<(), Error>
    {
        if header.number() == 0 {
            panic!("Genesis block should never be validated here");
        }

        let difficulty = header.difficulty().to_owned();
        let calc_difficulty = self.difficulty_calc.calculate_difficulty(
            parent_header,
            grand_parent_header,
            great_grand_parent_header,
            client,
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
