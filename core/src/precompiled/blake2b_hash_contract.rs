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

use super::builtin::{BuiltinParams, BuiltinContract, BuiltinExt};
use aion_types::{U256, H256};
use vms::{ExecutionResult, ExecStatus, ReturnData};
use blake2b::Blake2b;

const WORD_LENGTH: u64 = 4;
const NRG_CHARGE_PER_WORD: u64 = 2;

/// A pre-compiled contract for keccak hash computing
pub struct Blake2bHashContract {
    /// contract name
    name: String,
    /// block number from which this contract is supported
    activate_at: u64,
}

impl Blake2bHashContract {
    pub fn new(params: BuiltinParams) -> Self {
        Blake2bHashContract {
            name: params.name,
            activate_at: params.activate_at,
        }
    }
}

impl BuiltinContract for Blake2bHashContract {
    fn cost(&self, input: &[u8]) -> U256 {
        let len = input.len();
        if len == 0 || len > 2_097_152 {
            U256::from(10)
        } else {
            let additional_nrg =
                ((((len - 1) as f64 / WORD_LENGTH as f64).ceil()) as u64) * NRG_CHARGE_PER_WORD;
            U256::from(10 + additional_nrg)
        }
    }

    fn is_active(&self, at: u64) -> bool { at >= self.activate_at }

    fn name(&self) -> &str { &self.name }

    /// Returns the hash of given input
    /// @param input data input; must be less or equal than 2 MB
    /// @return the returned blake2b 256bits hash is in ExecutionResult.getOutput
    fn execute(&self, _ext: &mut BuiltinExt, input: &[u8]) -> ExecutionResult {
        // check length
        let len = input.len();
        if len == 0 || len > 2_097_152 {
            return ExecutionResult {
                gas_left: U256::zero(),
                status_code: ExecStatus::Failure,
                return_data: ReturnData::empty(),
                exception: "incorrect size of the input data.".into(),
                state_root: H256::default(),
            };
        }
        let hash = &Blake2b::hash_256(input);
        ExecutionResult {
            gas_left: U256::zero(),
            status_code: ExecStatus::Success,
            return_data: ReturnData::new(hash.to_vec(), 0, hash.len()),
            exception: String::default(),
            state_root: H256::default(),
        }
    }
}
