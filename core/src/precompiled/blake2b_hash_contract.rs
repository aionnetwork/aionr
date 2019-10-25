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
                invokable_hashes: Default::default(),
            };
        }
        let hash = &Blake2b::hash_256(input);
        ExecutionResult {
            gas_left: U256::zero(),
            status_code: ExecStatus::Success,
            return_data: ReturnData::new(hash.to_vec(), 0, hash.len()),
            exception: String::default(),
            state_root: H256::default(),
            invokable_hashes: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Blake2bHashContract;
    use precompiled::builtin::{BuiltinParams, BuiltinExtImpl, BuiltinContext, BuiltinContract};
    use state::{State, Substate};
    use helpers::get_temp_state;
    use acore_bytes::to_hex;
    use aion_types::{Address, H256};
    use vms::ExecStatus;

    fn get_ext_default<'a>(
        state: &'a mut State<::db::StateDB>,
        substate: &'a mut Substate,
    ) -> BuiltinExtImpl<'a, ::db::StateDB>
    {
        BuiltinExtImpl::new(
            state,
            BuiltinContext {
                sender: Address::zero(),
                address: Address::zero(),
                tx_hash: H256::zero(),
                origin_tx_hash: H256::default(),
            },
            substate,
        )
    }

    fn get_contract() -> Blake2bHashContract {
        Blake2bHashContract::new(BuiltinParams {
            activate_at: 920000,
            deactivate_at: None,
            name: String::from("blake2b"),
            owner_address: None,
            contract_address: None,
        })
    }

    #[test]
    fn test_blake256() {
        let contract = get_contract();
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext_default(state, substate);

        let input = "a0010101010101010101010101".as_bytes();
        let result = contract.execute(&mut ext, &input);
        assert!(result.status_code == ExecStatus::Success);
        let ret_data = result.return_data;
        assert_eq!((*ret_data).len(), 32);
        let expected = "aa6648de0988479263cf3730a48ef744d238b96a5954aa77d647ae965d3f7715";
        assert_eq!(to_hex(&*ret_data), expected);
    }

    #[test]
    fn test_blake256_2() {
        let contract = get_contract();
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext_default(state, substate);

        let input = "1".as_bytes();
        let result = contract.execute(&mut ext, &input);
        assert!(result.status_code == ExecStatus::Success);
        let ret_data = result.return_data;
        assert_eq!((*ret_data).len(), 32);
        let expected = "92cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db";
        assert_eq!(to_hex(&*ret_data), expected);
    }

    // uncomment if uncomment box_syntax in root crate
    //    #[test]
    //    fn test_blake256_3() {
    //        // long input
    //        let contract = get_contract();
    //        let state = &mut get_temp_state();
    //        let substate = &mut Substate::new();
    //        let mut ext = get_ext_default(state, substate);
    //
    //        let input = box [0u8; 2 * 1024 * 1024];
    //        let result = contract.execute(&mut ext, input.as_ref());
    //        assert!(result.status_code == ExecStatus::Success);
    //        let ret_data = result.return_data;
    //        assert_eq!(ret_data.mem.len(), 32);
    //        let expected = "9852d74e002f23d14ba2638b905609419bd16e50843ac147ccf4d509ed2c9dfc";
    //        assert_eq!(to_hex(&ret_data.mem), expected);
    //    }

    #[test]
    fn test_blake256_invalid_length() {
        let contract = get_contract();
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext_default(state, substate);
        let input = "".as_bytes();
        let result = contract.execute(&mut ext, &input);
        assert!(result.status_code == ExecStatus::Failure);
    }

    // uncomment if uncomment box_syntax in root crate
    //    #[test]
    //    fn test_blake256_larger_length() {
    //        let contract = get_contract();
    //        let state = &mut get_temp_state();
    //        let substate = &mut Substate::new();
    //        let mut ext = get_ext_default(state, substate);
    //        let input = box [0u8; 2 * 1024 * 1024 + 1];
    //        let result = contract.execute(&mut ext, input.as_ref());
    //        assert!(result.status_code == ExecStatus::Failure);
    //    }

}
