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

use uint::Uint;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct POWEquihashEngineParams {
    #[serde(rename = "rampupUpperBound")]
    pub rampup_upper_bound: Option<Uint>,
    #[serde(rename = "rampupLowerBound")]
    pub rampup_lower_bound: Option<Uint>,
    #[serde(rename = "rampupStartValue")]
    pub rampup_start_value: Option<Uint>,
    #[serde(rename = "rampupEndValue")]
    pub rampup_end_value: Option<Uint>,
    #[serde(rename = "upperBlockReward")]
    pub upper_block_reward: Option<Uint>,
    #[serde(rename = "lowerBlockReward")]
    pub lower_block_reward: Option<Uint>,
    #[serde(rename = "difficultyBoundDivisor")]
    pub difficulty_bound_divisor: Option<Uint>,
    #[serde(rename = "blockTimeLowerBound")]
    pub block_time_lower_bound: Option<u64>,
    #[serde(rename = "blockTimeUpperBound")]
    pub block_time_upper_bound: Option<u64>,
    #[serde(rename = "minimumDifficulty")]
    pub minimum_difficulty: Option<Uint>,
}

/// pow equihash engine deserialization
#[derive(Debug, PartialEq, Deserialize)]
pub struct POWEquihashEngine {
    /// pow equihash engine params.
    pub params: POWEquihashEngineParams,
}
