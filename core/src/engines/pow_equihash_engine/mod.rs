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
mod grant_parent_header_validators;

use ajson;
use machine::EthereumMachine;
use engines::Engine;
use aion_types::{U256, U512};
use header::Header;
use block::ExecutedBlock;
use error::Error;
use std::cmp;
use std::sync::Mutex;
use std::sync::Arc;
use types::BlockNumber;

use equihash::EquihashValidator;
use self::dependent_header_validators::{
    DependentHeaderValidator,
    NumberValidator,
    TimestampValidator,
//    EnergyLimitValidator
};
use self::header_validators::{
    VersionValidator,
//    ExtraDataValidator,
    HeaderValidator,
    POWValidator,
    EnergyConsumedValidator,
    EquihashSolutionValidator
};
use self::grant_parent_header_validators::{GrantParentHeaderValidator, DifficultyValidator};

const ANNUAL_BLOCK_MOUNT : u64 = 3110400;
const COMPOUND_YEAR_MAX : u64 = 128;

#[derive(Debug, PartialEq)]
pub struct POWEquihashEngineParams {
    pub rampup_upper_bound: U256,
    pub rampup_lower_bound: U256,
    pub rampup_start_value: U256,
    pub rampup_end_value: U256,
    pub upper_block_reward: U256,
    pub lower_block_reward: U256,
    pub difficulty_bound_divisor: U256,
    pub block_time_lower_bound: u64,
    pub block_time_upper_bound: u64,
    pub minimum_difficulty: U256,
}

impl From<ajson::spec::POWEquihashEngineParams> for POWEquihashEngineParams {
    fn from(p: ajson::spec::POWEquihashEngineParams) -> Self {
        POWEquihashEngineParams {
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
            block_time_lower_bound: p.block_time_lower_bound.map_or(5u64, Into::into),
            block_time_upper_bound: p.block_time_upper_bound.map_or(15u64, Into::into),
            minimum_difficulty: p.minimum_difficulty.map_or(U256::from(16), Into::into),
        }
    }
}

/// Difficulty calculator. TODO: impl mfc trait.
pub struct DifficultyCalc {
    difficulty_bound_divisor: U256,
    block_time_lower_bound: u64,
    block_time_upper_bound: u64,
    minimum_difficulty: U256,
}

impl DifficultyCalc {
    pub fn new(params: &POWEquihashEngineParams) -> DifficultyCalc {
        DifficultyCalc {
            difficulty_bound_divisor: params.difficulty_bound_divisor,
            block_time_lower_bound: params.block_time_lower_bound,
            block_time_upper_bound: params.block_time_upper_bound,
            minimum_difficulty: params.minimum_difficulty,
        }
    }
    pub fn calculate_difficulty(
        &self,
        header: &Header,
        parent: &Header,
        grant_parent: Option<&Header>,
    ) -> U256
    {
        if header.number() == 0 {
            panic!("Can't calculate genesis block difficulty.");
        }
        if parent.number() == 0 {
            return parent.difficulty().clone();
        }
        if header.number() != 2 && grant_parent.is_none() {
            panic!("grant_parent must exist.");
        }

        let parent_difficulty = *parent.difficulty();

        let mut diff_base = parent_difficulty / self.difficulty_bound_divisor;

        // if smaller than our bound divisor, always round up
        if diff_base.is_zero() {
            diff_base = U256::one();
        }

        let current_timestamp = parent.timestamp();
        let parent_timestamp = grant_parent.unwrap().timestamp();

        let delta = current_timestamp - parent_timestamp;
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
    fn new(params: &POWEquihashEngineParams, monetary_policy_update: Option<BlockNumber>, premine: U256) -> RewardsCalculator {
        // precalculate the desired increment.
        let delta = params.rampup_upper_bound - params.rampup_lower_bound;
        let m = (params.rampup_end_value - params.rampup_start_value) / delta;

        let mut compound_lookup_table : Vec<U256> = Vec::new();
        if let Some(number) = monetary_policy_update {
            let total_supply = Self::calculate_total_supply_before_monetary_update(premine, number, params);

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

    fn calculate_total_supply_before_monetary_update(initial_supply: U256, monetary_change_block_num: u64, params: &POWEquihashEngineParams) -> U256 {
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
            for _ in self.compound_lookup_table.iter() {
            }
            *current_reward = self.compound_lookup_table.get(term as usize).unwrap_or(&U256::from(0)).clone();
            *current_term = term;
        }

        return *current_reward;
    }

    fn calculate_reward_before_monetary_update(number: u64, params: &POWEquihashEngineParams, m: U256) -> U256 {
        let num : U256 = U256::from(number);

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
pub struct POWEquihashEngine {
    machine: EthereumMachine,
    rewards_calculator: RewardsCalculator,
    difficulty_calc: DifficultyCalc,
}

impl POWEquihashEngine {
    /// Create a new instance of Equihash engine
    pub fn new(params: POWEquihashEngineParams, machine: EthereumMachine) -> Arc<Self> {
        let rewards_calculator = RewardsCalculator::new(&params, machine.params().monetary_policy_update, machine.premine());
        let difficulty_calc = DifficultyCalc::new(&params);
        Arc::new(POWEquihashEngine {
            machine,
            rewards_calculator,
            difficulty_calc,
        })
    }

    fn calculate_difficulty(
        &self,
        header: &Header,
        parent: &Header,
        grant_parent: Option<&Header>,
    ) -> U256
    {
        self.difficulty_calc
            .calculate_difficulty(header, parent, grant_parent)
    }

    fn calculate_reward(&self, header: &Header) -> U256 {
        self.rewards_calculator.calculate_reward(header)
    }

    pub fn validate_block_header(header: &Header) -> Result<(), Error> {
        let mut block_header_validators: Vec<Box<HeaderValidator>> = Vec::with_capacity(4);
        block_header_validators.push(Box::new(VersionValidator {}));
        block_header_validators.push(Box::new(EnergyConsumedValidator {}));
        block_header_validators.push(Box::new(POWValidator {}));

        for v in block_header_validators.iter() {
            v.validate(header)?;
        }

        Ok(())
    }
}

impl Engine<EthereumMachine> for Arc<POWEquihashEngine> {
    fn name(&self) -> &str { "POWEquihashEngine" }

    fn machine(&self) -> &EthereumMachine { &self.machine }

    fn seal_fields(&self, _header: &Header) -> usize {
        // we don't add nonce and solution in header, continue to encapsulate them in seal field.
        // nonce and solution.
        2
    }

    fn populate_from_parent(
        &self,
        header: &mut Header,
        parent: &Header,
        grant_parent: Option<&Header>,
    )
    {
        let difficulty = self.calculate_difficulty(header, parent, grant_parent);
        header.set_difficulty(difficulty);
    }

    fn on_close_block(&self, block: &mut ExecutedBlock) -> Result<(), Error> {
        use aion_machine::{LiveBlock, WithBalances};

        let result_block_reward;
        let author;
        {
            let header = LiveBlock::header(&*block);
            result_block_reward = self.calculate_reward(&header);
            //result_block_reward = U256::from(3028549382716049382u64);
            debug!(target: "cons", "verify number: {}, reward: {} ", header.number(),  result_block_reward);
            author = *header.author();
        }
        block.header_mut().set_reward(result_block_reward.clone());
        self.machine
            .add_balance(block, &author, &result_block_reward)?;
        self.machine
            .note_rewards(block, &[(author, result_block_reward)])
    }

    fn verify_local_seal(&self, header: &Header) -> Result<(), Error> {
        self.verify_block_basic(header)
            .and_then(|_| self.verify_block_unordered(header))
    }

    fn verify_block_basic(&self, header: &Header) -> Result<(), Error> {
        let mut cheap_validators: Vec<Box<HeaderValidator>> = Vec::with_capacity(4);
        cheap_validators.push(Box::new(VersionValidator {}));
        cheap_validators.push(Box::new(EnergyConsumedValidator {}));
        cheap_validators.push(Box::new(POWValidator {}));

        for v in cheap_validators.iter() {
            v.validate(header)?;
        }

        Ok(())
    }

    fn verify_block_unordered(&self, header: &Header) -> Result<(), Error> {
        let mut costly_validators: Vec<Box<HeaderValidator>> = Vec::with_capacity(1);
        costly_validators.push(Box::new(EquihashSolutionValidator {
            solution_validator: EquihashValidator::new(210, 9),
        }));
        for v in costly_validators.iter() {
            v.validate(header)?;
        }
        Ok(())
    }

    fn verify_block_family(
        &self,
        header: &Header,
        parent: &Header,
        grant_parent: Option<&Header>,
    ) -> Result<(), Error>
    {
        // verify parent
        let mut parent_validators: Vec<Box<DependentHeaderValidator>> = Vec::with_capacity(3);
        parent_validators.push(Box::new(NumberValidator {}));
        parent_validators.push(Box::new(TimestampValidator {}));
        for v in parent_validators.iter() {
            v.validate(header, parent)?;
        }

        // verify grant parent
        let mut grant_validators: Vec<Box<GrantParentHeaderValidator>> = Vec::with_capacity(1);
        grant_validators.push(Box::new(DifficultyValidator {
            difficulty_calc: &self.difficulty_calc,
        }));
        for v in grant_validators.iter() {
            v.validate(header, parent, grant_parent)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Header;
    use super::U256;
    use super::RewardsCalculator;
    use super::POWEquihashEngineParams;
    use super::DifficultyCalc;

    #[test]
    fn test_calculate_rewards_number1() {
        let params = POWEquihashEngineParams {
            rampup_upper_bound: U256::from(259200),
            rampup_lower_bound: U256::zero(),
            rampup_start_value: U256::from(748994641621655092u64),
            rampup_end_value: U256::from(1497989283243310185u64),
            lower_block_reward: U256::from(748994641621655092u64),
            upper_block_reward: U256::from(1497989283243310185u64),
            difficulty_bound_divisor: U256::zero(),
            block_time_lower_bound: 0u64,
            block_time_upper_bound: 0u64,
            minimum_difficulty: U256::zero(),
        };
        let calculator = RewardsCalculator::new(&params, None, U256::from(0));
        let mut header = Header::default();
        header.set_number(1);
        assert_eq!(
            calculator.calculate_reward(&header),
            U256::from(748997531261476163u64)
        );
    }

    #[test]
    fn test_calculate_rewards_number10000() {
        let params = POWEquihashEngineParams {
            rampup_upper_bound: U256::from(259200),
            rampup_lower_bound: U256::zero(),
            rampup_start_value: U256::from(748994641621655092u64),
            rampup_end_value: U256::from(1497989283243310185u64),
            lower_block_reward: U256::from(748994641621655092u64),
            upper_block_reward: U256::from(1497989283243310185u64),
            difficulty_bound_divisor: U256::zero(),
            block_time_lower_bound: 0u64,
            block_time_upper_bound: 0u64,
            minimum_difficulty: U256::zero(),
        };
        let calculator = RewardsCalculator::new(&params, None, U256::from(0));
        let mut header = Header::default();
        header.set_number(10000);
        assert_eq!(
            calculator.calculate_reward(&header),
            U256::from(777891039832365092u64)
        );
    }

    #[test]
    fn test_calculate_rewards_number259200() {
        let params = POWEquihashEngineParams {
            rampup_upper_bound: U256::from(259200),
            rampup_lower_bound: U256::zero(),
            rampup_start_value: U256::from(748994641621655092u64),
            rampup_end_value: U256::from(1497989283243310185u64),
            lower_block_reward: U256::from(748994641621655092u64),
            upper_block_reward: U256::from(1497989283243310185u64),
            difficulty_bound_divisor: U256::zero(),
            block_time_lower_bound: 0u64,
            block_time_upper_bound: 0u64,
            minimum_difficulty: U256::zero(),
        };
        let calculator = RewardsCalculator::new(&params, None, U256::from(0));
        let mut header = Header::default();
        header.set_number(259200);
        assert_eq!(
            calculator.calculate_reward(&header),
            U256::from(1497989283243258292u64)
        );
    }

    #[test]
    fn test_calculate_rewards_number300000() {
        let params = POWEquihashEngineParams {
            rampup_upper_bound: U256::from(259200),
            rampup_lower_bound: U256::zero(),
            rampup_start_value: U256::from(748994641621655092u64),
            rampup_end_value: U256::from(1497989283243310185u64),
            lower_block_reward: U256::from(748994641621655092u64),
            upper_block_reward: U256::from(1497989283243310185u64),
            difficulty_bound_divisor: U256::zero(),
            block_time_lower_bound: 0u64,
            block_time_upper_bound: 0u64,
            minimum_difficulty: U256::zero(),
        };
        let calculator = RewardsCalculator::new(&params, None, U256::from(0));
        let mut header = Header::default();
        header.set_number(300000);
        assert_eq!(
            calculator.calculate_reward(&header),
            U256::from(1497989283243310185u64)
        );
    }

    #[test]
    fn test_calculate_difficulty() {
        let params = POWEquihashEngineParams {
            rampup_upper_bound: U256::zero(),
            rampup_lower_bound: U256::zero(),
            rampup_start_value: U256::zero(),
            rampup_end_value: U256::zero(),
            lower_block_reward: U256::zero(),
            upper_block_reward: U256::zero(),
            difficulty_bound_divisor: U256::from(2048),
            block_time_lower_bound: 5u64,
            block_time_upper_bound: 15u64,
            minimum_difficulty: U256::from(16),
        };
        let calculator = DifficultyCalc::new(&params);
        let mut header = Header::default();
        header.set_number(3);
        let mut parent_header = Header::default();
        parent_header.set_timestamp(1524538000u64);
        parent_header.set_difficulty(U256::from(1));
        parent_header.set_number(2);
        let mut grant_parent_header = Header::default();
        grant_parent_header.set_timestamp(1524528000u64);
        grant_parent_header.set_number(1);
        let difficulty =
            calculator.calculate_difficulty(&header, &parent_header, Some(&grant_parent_header));
        assert_eq!(difficulty, U256::from(16));
    }
    #[test]
    fn test_calculate_difficulty2() {
        let params = POWEquihashEngineParams {
            rampup_upper_bound: U256::zero(),
            rampup_lower_bound: U256::zero(),
            rampup_start_value: U256::zero(),
            rampup_end_value: U256::zero(),
            lower_block_reward: U256::zero(),
            upper_block_reward: U256::zero(),
            difficulty_bound_divisor: U256::from(2048),
            block_time_lower_bound: 5u64,
            block_time_upper_bound: 15u64,
            minimum_difficulty: U256::from(16),
        };
        let calculator = DifficultyCalc::new(&params);
        let mut header = Header::default();
        header.set_number(3);
        let mut parent_header = Header::default();
        parent_header.set_timestamp(1524528005u64);
        parent_header.set_number(2);
        parent_header.set_difficulty(U256::from(2000));
        let mut grant_parent_header = Header::default();
        grant_parent_header.set_timestamp(1524528000u64);
        grant_parent_header.set_number(1);
        let difficulty =
            calculator.calculate_difficulty(&header, &parent_header, Some(&grant_parent_header));
        assert_eq!(difficulty, U256::from(2001));
    }
    #[test]
    fn test_calculate_difficulty3() {
        let params = POWEquihashEngineParams {
            rampup_upper_bound: U256::zero(),
            rampup_lower_bound: U256::zero(),
            rampup_start_value: U256::zero(),
            rampup_end_value: U256::zero(),
            lower_block_reward: U256::zero(),
            upper_block_reward: U256::zero(),
            difficulty_bound_divisor: U256::from(2048),
            block_time_lower_bound: 5u64,
            block_time_upper_bound: 15u64,
            minimum_difficulty: U256::from(16),
        };
        let calculator = DifficultyCalc::new(&params);
        let mut header = Header::default();
        header.set_number(3);
        let mut parent_header = Header::default();
        parent_header.set_timestamp(1524528010u64);
        parent_header.set_difficulty(U256::from(3000));
        parent_header.set_number(2);
        let mut grant_parent_header = Header::default();
        grant_parent_header.set_timestamp(1524528000u64);
        grant_parent_header.set_number(1);
        let difficulty =
            calculator.calculate_difficulty(&header, &parent_header, Some(&grant_parent_header));
        assert_eq!(difficulty, U256::from(3000));
    }
    #[test]
    fn test_calculate_difficulty4() {
        let params = POWEquihashEngineParams {
            rampup_upper_bound: U256::zero(),
            rampup_lower_bound: U256::zero(),
            rampup_start_value: U256::zero(),
            rampup_end_value: U256::zero(),
            lower_block_reward: U256::zero(),
            upper_block_reward: U256::zero(),
            difficulty_bound_divisor: U256::from(2048),
            block_time_lower_bound: 5u64,
            block_time_upper_bound: 15u64,
            minimum_difficulty: U256::from(16),
        };
        let calculator = DifficultyCalc::new(&params);
        let mut header = Header::default();
        header.set_number(3);
        let mut parent_header = Header::default();
        parent_header.set_timestamp(1524528020u64);
        parent_header.set_difficulty(U256::from(3000));
        parent_header.set_number(2);
        let mut grant_parent_header = Header::default();
        grant_parent_header.set_timestamp(1524528000u64);
        grant_parent_header.set_number(1);
        let difficulty =
            calculator.calculate_difficulty(&header, &parent_header, Some(&grant_parent_header));
        assert_eq!(difficulty, U256::from(2999));
    }

}
