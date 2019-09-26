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

mod header_validators;
mod dependent_header_validators;
mod grand_parent_header_validators;
mod pos_validator;
#[cfg(test)]
mod test;

use std::sync::Arc;
use super::Engine;
use ajson;
use machine::EthereumMachine;
use aion_machine::{LiveBlock, WithBalances};
use aion_types::{U256, U512};
use header::{Header, SealType};
use block::ExecutedBlock;
use types::error::{BlockError, Error};
use std::cmp;
use std::sync::Mutex;
use types::BlockNumber;
use equihash::EquihashValidator;
use fixed_point::{FixedPoint};

use self::dependent_header_validators::{
    DependentHeaderValidator,
    NumberValidator,
    TimestampValidator,
    SealTypeValidator,
};
use self::header_validators::{
//    ExtraDataValidator,
    HeaderValidator,
    POWValidator,
    EnergyConsumedValidator,
    EquihashSolutionValidator,
    FutureTimestampValidator,
};
use self::grand_parent_header_validators::{GrandParentHeaderValidator, DifficultyValidator};
use self::pos_validator::PoSValidator;
use num_bigint::BigUint;

const ANNUAL_BLOCK_MOUNT: u64 = 3110400;
const COMPOUND_YEAR_MAX: u64 = 128;

// Our barrier should be log2*10 = 6.9xxxx
// but we only compare it against integer values, so we use 7
const BARRIER: u64 = 7;

lazy_static! {
    static ref DIFF_INC_RATE: FixedPoint = FixedPoint::from_str_radix("1.05", 10).unwrap();
    static ref DIFF_DEC_RATE: FixedPoint = FixedPoint::from_str_radix("0.952381", 10).unwrap();
}

#[derive(Debug, PartialEq)]
pub struct UnityEngineParams {
    pub rampup_upper_bound: U256,
    pub rampup_lower_bound: U256,
    pub rampup_start_value: U256,
    pub rampup_end_value: U256,
    pub upper_block_reward: U256,
    pub lower_block_reward: U256,
    pub difficulty_bound_divisor: U256,
    pub difficulty_bound_divisor_unity: u64,
    pub block_time_lower_bound: u64,
    pub block_time_upper_bound: u64,
    pub block_time_unity: u64,
    pub minimum_difficulty: U256,
}

impl From<ajson::spec::UnityEngineParams> for UnityEngineParams {
    fn from(p: ajson::spec::UnityEngineParams) -> Self {
        UnityEngineParams {
            rampup_upper_bound: p.rampup_upper_bound.map_or(U256::from(259200), Into::into),
            rampup_lower_bound: p.rampup_lower_bound.map_or(U256::zero(), Into::into),
            rampup_start_value: p
                .rampup_start_value
                .map_or(U256::from(748994641621655092u64), Into::into),
            rampup_end_value: p
                .rampup_end_value
                .map_or(U256::from(1497989283243310185u64), Into::into),
            upper_block_reward: p
                .upper_block_reward
                .map_or(U256::from(1497989283243310185u64), Into::into),
            lower_block_reward: p
                .lower_block_reward
                .map_or(U256::from(748994641621655092u64), Into::into),
            difficulty_bound_divisor: p
                .difficulty_bound_divisor
                .map_or(U256::from(2048), Into::into),
            difficulty_bound_divisor_unity: p
                .difficulty_bound_divisor_unity
                .map_or(20u64, Into::into),
            block_time_lower_bound: p.block_time_lower_bound.map_or(5u64, Into::into),
            block_time_upper_bound: p.block_time_upper_bound.map_or(15u64, Into::into),
            block_time_unity: p.block_time_unity.map_or(10u64, Into::into),
            minimum_difficulty: p.minimum_difficulty.map_or(U256::from(16), Into::into),
        }
    }
}

/// Difficulty calculator. TODO: impl mfc trait.
pub struct DifficultyCalc {
    difficulty_bound_divisor: U256,
    //    difficulty_bound_divisor_unity: u64,
    block_time_lower_bound: u64,
    block_time_upper_bound: u64,
    //    block_time_unity: u64,
    minimum_difficulty: U256,
    unity_update: Option<BlockNumber>,
    unity_initial_pos_difficulty: Option<U256>,
}

impl DifficultyCalc {
    pub fn new(
        params: &UnityEngineParams,
        unity_update: Option<BlockNumber>,
        unity_initial_pos_difficulty: Option<U256>,
    ) -> DifficultyCalc
    {
        DifficultyCalc {
            difficulty_bound_divisor: params.difficulty_bound_divisor,
            //            difficulty_bound_divisor_unity: params.difficulty_bound_divisor_unity,
            block_time_lower_bound: params.block_time_lower_bound,
            block_time_upper_bound: params.block_time_upper_bound,
            minimum_difficulty: params.minimum_difficulty,
            //            block_time_unity: params.block_time_unity,
            unity_update: unity_update,
            unity_initial_pos_difficulty: unity_initial_pos_difficulty,
        }
    }

    pub fn calculate_difficulty(
        &self,
        parent: &Header,
        grand_parent: Option<&Header>,
        great_grand_parent: Option<&Header>,
    ) -> U256
    {
        // If no grand parent, return the difficulty of the parent
        let grand_parent = match grand_parent {
            Some(header) => header,
            None => {
                return parent.difficulty().to_owned();
            }
        };

        match self.unity_update {
            Some(fork_number) if parent.number() + 1 >= fork_number => {
                // If no great grand parent, return the difficulty of the grand parent
                let great_grand_parent = match great_grand_parent {
                    Some(header) => header,
                    None => {
                        return grand_parent.difficulty().to_owned();
                    }
                };
                self.calculate_difficulty_v2(grand_parent, great_grand_parent)
            }
            _ => self.calculate_difficulty_v1(parent, grand_parent),
        }
    }

    // Aion 1.0 difficulty adjustment algorithm (pure PoW)
    fn calculate_difficulty_v1(&self, parent: &Header, grand_parent: &Header) -> U256 {
        let parent_difficulty = parent.difficulty().to_owned();
        let mut diff_base = parent_difficulty / self.difficulty_bound_divisor;
        // if smaller than our bound divisor, always round up
        if diff_base.is_zero() {
            diff_base = U256::one();
        }
        let parent_timestamp = parent.timestamp();
        let grand_parent_timestamp = grand_parent.timestamp();
        let delta = parent_timestamp - grand_parent_timestamp;
        let bound_domain = 10;

        // split into our ranges 0 <= x <= min_block_time, min_block_time < x <
        // max_block_time, max_block_time < x
        let mut output_difficulty: U256;
        if delta <= self.block_time_lower_bound {
            output_difficulty = parent_difficulty + diff_base;
        } else if self.block_time_lower_bound < delta && delta < self.block_time_upper_bound {
            output_difficulty = parent_difficulty;
        } else {
            let bound_quotient =
                U256::from(((delta - self.block_time_upper_bound) / bound_domain) + 1);
            let lower_bound = U256::from(99);
            let multiplier = cmp::min(bound_quotient, lower_bound);
            if parent_difficulty > multiplier * diff_base {
                output_difficulty = parent_difficulty - multiplier * diff_base;
            } else {
                output_difficulty = self.minimum_difficulty;
            }
        }
        output_difficulty = cmp::max(output_difficulty, self.minimum_difficulty);
        output_difficulty
    }

    // Aion 2.0 (Unity) difficulty adjusmtnet algorithm (PoS and PoW hybrid)
    fn calculate_difficulty_v2(&self, parent: &Header, grand_parent: &Header) -> U256 {
        let parent_difficulty = parent.difficulty().clone();
        let parent_timestamp = parent.timestamp();
        let grand_parent_timestamp = grand_parent.timestamp();
        let delta_time = parent_timestamp - grand_parent_timestamp;
        assert!(delta_time > 0);

        // TODO-Unity: To refine floating calculation
        //        let lambda = 1f64 / (2f64 * self.block_time_unity as f64);
        //        let diff = match (delta_time as f64) - (-0.5f64.ln() / lambda) {

        let diff: U256 = match delta_time >= BARRIER {
            true => {
                DIFF_DEC_RATE.multiply_uint(parent_difficulty.into()).to_big_uint().into()
            }
            false => {
                let temp :U256 = DIFF_INC_RATE.multiply_uint(parent_difficulty.into()).to_big_uint().into();
                if temp == parent_difficulty {
                    temp + 1u64.into()
                } else {
                    temp
                }
            }
//            _ => parent_difficulty.as_u64(),
        };

        match parent.seal_type() {
            None | Some(SealType::PoW) => cmp::max(self.minimum_difficulty, diff),
            // TODO：
            Some(SealType::PoS) => {
                cmp::max(
                    self.unity_initial_pos_difficulty
                        .unwrap_or(2_000_000_000u64.into()),
                    diff,
                )
            }
        }
    }
}

/// Reward calculator. TODO: impl mcf trait.
pub struct RewardsCalculator {
    rampup_upper_bound: U256,
    rampup_lower_bound: U256,
    rampup_start_value: U256,
    lower_block_reward: U256,
    upper_block_reward: U256,
    monetary_policy_update: Option<BlockNumber>,
    m: U256,
    current_term: Mutex<u64>,
    current_reward: Mutex<U256>,
    compound_lookup_table: Vec<U256>,
}

impl RewardsCalculator {
    fn new(
        params: &UnityEngineParams,
        monetary_policy_update: Option<BlockNumber>,
        premine: U256,
    ) -> RewardsCalculator
    {
        // precalculate the desired increment.
        let delta = params.rampup_upper_bound - params.rampup_lower_bound;
        let m = (params.rampup_end_value - params.rampup_start_value) / delta;

        let mut compound_lookup_table: Vec<U256> = Vec::new();
        if let Some(number) = monetary_policy_update {
            let total_supply =
                Self::calculate_total_supply_before_monetary_update(premine, number, params);

            for i in 0..COMPOUND_YEAR_MAX {
                compound_lookup_table.push(Self::calculate_compound(i, total_supply));
            }
        }

        RewardsCalculator {
            rampup_upper_bound: params.rampup_upper_bound,
            rampup_lower_bound: params.rampup_lower_bound,
            rampup_start_value: params.rampup_start_value,
            lower_block_reward: params.lower_block_reward,
            upper_block_reward: params.upper_block_reward,
            monetary_policy_update,
            m,
            current_term: Mutex::new(0),
            current_reward: Mutex::new(U256::from(0)),
            compound_lookup_table,
        }
    }

    fn calculate_reward(&self, header: &Header) -> U256 {
        if let Some(n) = self.monetary_policy_update {
            if header.number() > n {
                return self.calculate_reward_after_monetary_update(header.number());
            }
        }

        let number = U256::from(header.number());
        if number <= self.rampup_lower_bound {
            self.lower_block_reward
        } else if number <= self.rampup_upper_bound {
            (number - self.rampup_lower_bound) * self.m + self.rampup_start_value
        } else {
            self.upper_block_reward
        }
    }

    fn calculate_total_supply_before_monetary_update(
        initial_supply: U256,
        monetary_change_block_num: u64,
        params: &UnityEngineParams,
    ) -> U256
    {
        if monetary_change_block_num < 1 {
            return initial_supply;
        } else {
            let mut ts = initial_supply;
            let delta = params.rampup_upper_bound - params.rampup_lower_bound;
            let m = (params.rampup_end_value - params.rampup_start_value) / delta;
            for i in 1..(monetary_change_block_num + 1) {
                ts = ts + Self::calculate_reward_before_monetary_update(i, params, m);
            }
            return ts;
        }
    }

    fn calculate_compound(term: u64, initial_supply: U256) -> U256 {
        let mut compound = initial_supply.full_mul(U256::from(10000));
        let mut pre_compound = compound;
        for _i in 0..term {
            pre_compound = compound;
            compound = pre_compound * 10100 / U512::from(10000);
        }
        compound = compound - pre_compound;
        compound = (compound / U512::from(ANNUAL_BLOCK_MOUNT)) / U512::from(10000);

        return U256::from(compound);
    }

    fn calculate_reward_after_monetary_update(&self, number: u64) -> U256 {
        let term = (number - self.monetary_policy_update.unwrap() - 1) / ANNUAL_BLOCK_MOUNT + 1;
        let mut current_term = self.current_term.lock().unwrap();
        let mut current_reward = self.current_reward.lock().unwrap();

        if term != *current_term {
            for _ in self.compound_lookup_table.iter() {}
            *current_reward = self
                .compound_lookup_table
                .get(term as usize)
                .unwrap_or(&U256::from(0))
                .clone();
            *current_term = term;
        }

        return *current_reward;
    }

    fn calculate_reward_before_monetary_update(
        number: u64,
        params: &UnityEngineParams,
        m: U256,
    ) -> U256
    {
        let num: U256 = U256::from(number);

        if num <= params.rampup_lower_bound {
            return params.lower_block_reward;
        } else if num <= params.rampup_upper_bound {
            return (num - params.rampup_lower_bound) * m + params.rampup_start_value;
        } else {
            return params.upper_block_reward;
        }
    }
}

/// Engine using Equihash proof-of-work concensus algorithm.
pub struct UnityEngine {
    machine: EthereumMachine,
    rewards_calculator: RewardsCalculator,
    difficulty_calc: DifficultyCalc,
}

impl UnityEngine {
    pub fn new(params: UnityEngineParams, machine: EthereumMachine) -> Arc<Self> {
        let rewards_calculator = RewardsCalculator::new(
            &params,
            machine.params().monetary_policy_update,
            machine.premine(),
        );
        let difficulty_calc = DifficultyCalc::new(
            &params,
            machine.params().unity_update,
            machine.params().unity_initial_pos_difficulty,
        );
        Arc::new(UnityEngine {
            machine,
            rewards_calculator,
            difficulty_calc,
        })
    }

    fn calculate_reward(&self, header: &Header) -> U256 {
        self.rewards_calculator.calculate_reward(header)
    }

    // TODO-Unity: duplcation of verify_block_basic. Handle this better. Some functions in trait EthereumMachine do not need *self*.
    pub fn validate_block_header(header: &Header) -> Result<(), Error> {
        let mut cheap_validators: Vec<Box<HeaderValidator>> = Vec::with_capacity(3);
        cheap_validators.push(Box::new(EnergyConsumedValidator {}));
        cheap_validators.push(Box::new(FutureTimestampValidator {}));
        if header.seal_type() == &Some(SealType::PoW) {
            cheap_validators.push(Box::new(POWValidator {}));
        }

        for v in cheap_validators.iter() {
            v.validate(header)?;
        }

        Ok(())
    }
}

impl Engine for Arc<UnityEngine> {
    fn name(&self) -> &str { "UnityEngine" }

    fn machine(&self) -> &EthereumMachine { &self.machine }

    fn calculate_difficulty(
        &self,
        parent: &Header,
        grand_parent: Option<&Header>,
        great_grand_parent: Option<&Header>,
    ) -> U256
    {
        self.difficulty_calc
            .calculate_difficulty(parent, grand_parent, great_grand_parent)
    }

    fn seal_fields(&self, header: &Header) -> usize {
        match header.seal_type() {
            Some(SealType::PoS) => 3,
            _ => 2,
        }
    }

    fn verify_block_basic(&self, header: &Header) -> Result<(), Error> {
        let mut cheap_validators: Vec<Box<HeaderValidator>> = Vec::with_capacity(2);
        cheap_validators.push(Box::new(EnergyConsumedValidator {}));
        if header.seal_type() == &Some(SealType::PoW) {
            cheap_validators.push(Box::new(POWValidator {}));
        }

        for v in cheap_validators.iter() {
            v.validate(header)?;
        }

        Ok(())
    }

    fn verify_block_unordered(&self, header: &Header) -> Result<(), Error> {
        let mut costly_validators: Vec<Box<HeaderValidator>> = Vec::with_capacity(1);
        if header.seal_type() == &Some(SealType::PoW) {
            costly_validators.push(Box::new(EquihashSolutionValidator {
                solution_validator: EquihashValidator::new(210, 9),
            }));
        }
        for v in costly_validators.iter() {
            v.validate(header)?;
        }
        Ok(())
    }

    fn verify_local_seal_pow(&self, header: &Header) -> Result<(), Error> {
        self.verify_block_basic(header)
            .and_then(|_| self.verify_block_unordered(header))
    }

    /// Verify the seal of locally produced PoS block
    fn verify_seal_pos(
        &self,
        header: &Header,
        parent: &Header,
        grand_parent: Option<&Header>,
        stake: Option<BigUint>,
    ) -> Result<(), Error>
    {
        if self
            .machine
            .params()
            .unity_update
            .map_or(true, |fork_number| header.number() < fork_number)
        {
            Err(BlockError::InvalidPoSBlockNumber.into())
        } else {
            PoSValidator::validate(header, parent, grand_parent, stake)?;
            Ok(())
        }
    }

    fn verify_block_family(
        &self,
        header: &Header,
        parent: &Header,
        grand_parent: Option<&Header>,
        great_grand_parent: Option<&Header>,
    ) -> Result<(), Error>
    {
        // Verifications related to direct parent
        let mut parent_validators: Vec<Box<DependentHeaderValidator>> = Vec::with_capacity(3);
        parent_validators.push(Box::new(NumberValidator {}));
        parent_validators.push(Box::new(TimestampValidator {}));
        // AION 2.0
        // After Unity hard fork, verify if a block has different seal type with its parent
        if self
            .machine
            .params()
            .unity_update
            .map_or(false, |fork_number| header.number() >= fork_number)
        {
            parent_validators.push(Box::new(SealTypeValidator {}));
        }
        for v in parent_validators.iter() {
            v.validate(header, parent)?;
        }

        // Verifications related to seal parent and seal grand parent
        let mut grand_validators: Vec<Box<GrandParentHeaderValidator>> = Vec::with_capacity(1);
        grand_validators.push(Box::new(DifficultyValidator {
            difficulty_calc: &self.difficulty_calc,
        }));
        for v in grand_validators.iter() {
            v.validate(header, parent, grand_parent, great_grand_parent)?;
        }

        Ok(())
    }

    fn set_difficulty_from_parent(
        &self,
        header: &mut Header,
        parent: &Header,
        grand_parent: Option<&Header>,
        great_grand_parent: Option<&Header>,
    )
    {
        if header.number() == 0 {
            panic!("Can't calculate genesis block difficulty.");
        }

        let difficulty = self.calculate_difficulty(parent, grand_parent, great_grand_parent);
        header.set_difficulty(difficulty);
    }

    fn on_close_block(&self, block: &mut ExecutedBlock) -> Result<(), Error> {
        let result_block_reward;
        let author;
        {
            let header = LiveBlock::header(&*block);
            result_block_reward = self.calculate_reward(&header);
            author = *header.author();
            debug!(target: "cons", "verify number: {}, coinbase: {}, reward: {} ", header.number(), &author, result_block_reward);
        }
        block.header_mut().set_reward(result_block_reward.clone());
        self.machine
            .add_balance(block, &author, &result_block_reward)?;
        self.machine
            .note_rewards(block, &[(author, result_block_reward)])
    }
}
