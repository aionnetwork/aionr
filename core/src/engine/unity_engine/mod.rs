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
use types::error::Error;
use std::cmp;
use std::sync::Mutex;
use types::BlockNumber;
use equihash::EquihashValidator;
use self::dependent_header_validators::{
    DependentHeaderValidator,
    NumberValidator,
    TimestampValidator,
};
use self::header_validators::{
//    ExtraDataValidator,
    HeaderValidator,
    POWValidator,
    EnergyConsumedValidator,
    EquihashSolutionValidator,
};
use self::grand_parent_header_validators::{GrandParentHeaderValidator, DifficultyValidator};
use self::pos_validator::PoSValidator;

const ANNUAL_BLOCK_MOUNT: u64 = 3110400;
const COMPOUND_YEAR_MAX: u64 = 128;

#[derive(Debug, PartialEq)]
pub struct UnityEngineParams {
    pub rampup_upper_bound: U256,
    pub rampup_lower_bound: U256,
    pub rampup_start_value: U256,
    pub rampup_end_value: U256,
    pub upper_block_reward: U256,
    pub lower_block_reward: U256,
    pub difficulty_bound_divisor: u64,
    pub block_time: u64,
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
            difficulty_bound_divisor: p.difficulty_bound_divisor.map_or(20u64, Into::into),
            block_time: p.block_time.map_or(10u64, Into::into),
            minimum_difficulty: p.minimum_difficulty.map_or(U256::from(16), Into::into),
        }
    }
}

/// Difficulty calculator. TODO: impl mfc trait.
pub struct DifficultyCalc {
    difficulty_bound_divisor: u64,
    block_time: u64,
    minimum_difficulty: U256,
}

impl DifficultyCalc {
    pub fn new(params: &UnityEngineParams) -> DifficultyCalc {
        DifficultyCalc {
            difficulty_bound_divisor: params.difficulty_bound_divisor,
            block_time: params.block_time,
            minimum_difficulty: params.minimum_difficulty,
        }
    }

    pub fn calculate_difficulty(
        &self,
        parent: Option<&Header>,
        grand_parent: Option<&Header>,
    ) -> U256
    {
        // First PoS block does not have seal parent.
        let parent = match parent {
            Some(header) => header,
            None => {
                return U256::from(2_000_000_000u64); // TODO-Unity: test setup to be comparable to the initial 1*10^9 stake. Change it in real setup or make it configurable in engine paremeter.
            }
        };

        // If no seal grand parent, return the difficulty of the seal parent
        let grand_parent = match grand_parent {
            Some(header) => header,
            None => {
                return parent.difficulty().to_owned();
            }
        };

        let parent_difficulty = parent.difficulty();
        let parent_timestamp = parent.timestamp();
        let grand_parent_timestamp = grand_parent.timestamp();
        let delta_time = parent_timestamp - grand_parent_timestamp;
        assert!(delta_time > 0);

        // TODO-Unity: To refine floating calculation
        let alpha = 1f64 / (self.difficulty_bound_divisor as f64);
        let lambda = 1f64 / (2f64 * self.block_time as f64);
        let diff = match (delta_time as f64) - (-0.5f64.ln() / lambda) {
            res if res > 0f64 => {
                cmp::min(
                    parent_difficulty.as_u64() - 1,
                    (parent_difficulty.as_u64() as f64 / (1f64 + alpha)) as u64,
                )
            }
            res if res < 0f64 => {
                cmp::max(
                    parent_difficulty.as_u64() + 1,
                    (parent_difficulty.as_u64() as f64 * (1f64 + alpha)) as u64,
                )
            }
            _ => parent_difficulty.as_u64(),
        };

        cmp::max(self.minimum_difficulty, U256::from(diff))
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
        let difficulty_calc = DifficultyCalc::new(&params);
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
}

impl Engine for Arc<UnityEngine> {
    fn name(&self) -> &str { "UnityEngine" }

    fn machine(&self) -> &EthereumMachine { &self.machine }

    fn calculate_difficulty(&self, parent: Option<&Header>, grand_parent: Option<&Header>) -> U256 {
        self.difficulty_calc
            .calculate_difficulty(parent, grand_parent)
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
        seal_parent: Option<&Header>,
        stake: Option<u64>,
    ) -> Result<(), Error>
    {
        PoSValidator::validate(header, seal_parent, stake)?;
        Ok(())
    }

    fn verify_block_family(
        &self,
        header: &Header,
        parent: &Header,
        seal_parent: Option<&Header>,
        seal_grand_parent: Option<&Header>,
    ) -> Result<(), Error>
    {
        // Verifications related to direct parent
        let mut parent_validators: Vec<Box<DependentHeaderValidator>> = Vec::with_capacity(2);
        parent_validators.push(Box::new(NumberValidator {}));
        parent_validators.push(Box::new(TimestampValidator {}));
        for v in parent_validators.iter() {
            v.validate(header, parent)?;
        }

        // Verifications related to seal parent and seal grand parent
        let mut grand_validators: Vec<Box<GrandParentHeaderValidator>> = Vec::with_capacity(1);
        grand_validators.push(Box::new(DifficultyValidator {
            difficulty_calc: &self.difficulty_calc,
        }));
        for v in grand_validators.iter() {
            v.validate(header, seal_parent, seal_grand_parent)?;
        }

        Ok(())
    }

    fn set_difficulty_from_parent(
        &self,
        header: &mut Header,
        parent: Option<&Header>,
        grand_parent: Option<&Header>,
    )
    {
        if header.number() == 0 {
            panic!("Can't calculate genesis block difficulty.");
        }

        let difficulty = self.calculate_difficulty(parent, grand_parent);
        header.set_difficulty(difficulty);
    }

    fn on_close_block(&self, block: &mut ExecutedBlock) -> Result<(), Error> {
        let result_block_reward;
        let author;
        {
            let header = LiveBlock::header(&*block);
            result_block_reward = self.calculate_reward(&header);
            debug!(target: "cons", "verify number: {}, reward: {} ", header.number(),  result_block_reward);
            author = *header.author();
        }
        block.header_mut().set_reward(result_block_reward.clone());
        self.machine
            .add_balance(block, &author, &result_block_reward)?;
        self.machine
            .note_rewards(block, &[(author, result_block_reward)])
    }
}
