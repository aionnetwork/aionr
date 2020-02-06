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

use super::builtin::{BuiltinContract, BuiltinExt, BuiltinParams};
use aion_types::{U256, H256, Address};
use vms::{ExecutionResult, ExecStatus, ReturnData};
use rcrypto::ed25519::verify;

/// A pre-copmiled contract for ed25519 verification.
pub struct EDVerifyContract {
    /// contract name
    name: String,
    /// block number from which this contract is supported.
    activate_at: u64,
}

impl EDVerifyContract {
    pub fn new(params: BuiltinParams) -> Self {
        EDVerifyContract {
            activate_at: params.activate_at,
            name: params.name,
        }
    }
}

impl BuiltinContract for EDVerifyContract {
    fn cost(&self, _input: &[u8]) -> U256 { U256::from(3000) }

    fn is_active(&self, at: u64) -> bool { at >= self.activate_at }

    fn name(&self) -> &str { &self.name }

    /// @param input 128 bytes of data input, [32-bytes message, 32-bytes public key, 64-bytes signature]
    //  @return the verification result of the given input (publickey address for pass, all-0's address for fail)
    fn execute(&self, _ext: &mut BuiltinExt, input: &[u8]) -> ExecutionResult {
        if input.len() != 128 {
            return ExecutionResult {
                gas_left: U256::zero(),
                status_code: ExecStatus::Failure,
                return_data: ReturnData::empty(),
                exception: "Incorrect input length".into(),
                state_root: H256::default(),
                invokable_hashes: Default::default(),
            };
        }

        let msg = &input[..32];
        let pub_key = &input[32..64];
        let sig = &input[64..128];

        if verify(msg, pub_key, sig) {
            ExecutionResult {
                gas_left: U256::zero(),
                status_code: ExecStatus::Success,
                return_data: ReturnData::new(pub_key.to_vec(), 0, pub_key.len()),
                exception: String::default(),
                state_root: H256::default(),
                invokable_hashes: Default::default(),
            }
        } else {
            let return_data = Address::zero();
            ExecutionResult {
                gas_left: U256::zero(),
                status_code: ExecStatus::Success,
                return_data: ReturnData::new(return_data.to_vec(), 0, return_data.len()),
                exception: String::default(),
                state_root: H256::default(),
                invokable_hashes: Default::default(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use key::{Ed25519Secret, Ed25519KeyPair, sign_ed25519, Message};
    use rustc_hex::FromHex;
    use tiny_keccak::keccak256;
    use super::EDVerifyContract;
    use precompiled::builtin::{BuiltinParams, BuiltinExtImpl, BuiltinContext, BuiltinContract};
    use helpers::get_temp_state;
    use vms::ExecStatus;
    use state::{State, Substate};
    use aion_types::{Address, H256};
    use acore_bytes::to_hex;

    fn get_test_data() -> Vec<u8> {
        let sec = Ed25519Secret::from_slice("5a90d8e67da5d1dfbf17916ae83bae04ef334f53ce8763932eba2c1116a62426fff4317ae351bda5e4fa24352904a9366d3a89e38d1ffa51498ba9acfbc65724".from_hex().unwrap().as_slice()).unwrap();
        let keypair = Ed25519KeyPair::from_secret(sec).unwrap();
        let pub_key = keypair.public();
        let data = "Our first test in AION1234567890".as_bytes();
        let hashed_message = keccak256(data);
        let sig = sign_ed25519(keypair.secret(), &Message::from(hashed_message)).unwrap();
        let mut input = Vec::with_capacity(128);
        input.extend_from_slice(&hashed_message);
        input.extend_from_slice(pub_key.as_ref());
        input.extend_from_slice(&sig.get_signature()[32..]);
        input
    }

    fn get_contract() -> EDVerifyContract {
        EDVerifyContract::new(BuiltinParams {
            activate_at: 9200000,
            deactivate_at: None,
            name: String::from("edverify"),
            owner_address: None,
            contract_address: None,
        })
    }

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

    #[test]
    fn test_edverify_contract_success() {
        let contract = get_contract();
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext_default(state, substate);

        let input = get_test_data();
        let result = contract.execute(&mut ext, input.as_slice());
        assert_eq!(result.status_code, ExecStatus::Success);
        let ret_data = result.return_data;
        let expected = "fff4317ae351bda5e4fa24352904a9366d3a89e38d1ffa51498ba9acfbc65724";
        assert_eq!(to_hex(&*ret_data), expected);
    }

    #[test]
    fn test_edverify_empty_input() {
        let contract = get_contract();
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext_default(state, substate);

        let input = [0u8; 128];
        let result = contract.execute(&mut ext, &input);
        assert_eq!(result.status_code, ExecStatus::Success);
        let ret_data = result.return_data;
        let expected = "0000000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(to_hex(&*ret_data), expected);
    }

    #[test]
    fn test_edverify_incorrect_input() {
        let contract = get_contract();
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext_default(state, substate);

        let mut input = get_test_data();
        let input = input.as_mut_slice();
        input[22] = (input[22] as u64 - 10) as u8;
        input[33] = (input[33] as u64 + 4) as u8;
        input[99] = (input[33] as u64 - 40) as u8;
        let result = contract.execute(&mut ext, &input);
        assert_eq!(result.status_code, ExecStatus::Success);
        let ret_data = result.return_data;
        let expected = "0000000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(to_hex(&*ret_data), expected);
    }

    #[test]
    fn test_edverify_invalid_input_length() {
        let contract = get_contract();
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext_default(state, substate);

        let mut input = [0u8; 129];
        input[128] = 1u8;
        let result = contract.execute(&mut ext, &input);
        assert_eq!(result.status_code, ExecStatus::Failure);
    }
}
