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

use super::builtin::{BuiltinExt, BuiltinContract, BuiltinParams};
use aion_types::{U256, H128, U128, H256, Address};
use types::vms::{ExecutionResult, ExecStatus, ReturnData};
use key::{Ed25519Signature, public_to_address_ed25519, verify_signature_ed25519};

/// A pre-compiled contract for retrieving and updating the total amount of currency.
pub struct TotalCurrencyContract {
    /// block number from which this contract is supported.
    deactivate_at: Option<u64>,
    /// contract name.
    name: String,
    /// contract owner address.
    owner_address: Address,
}

impl TotalCurrencyContract {
    /// Constructs a new TotalCurrencyContract
    pub fn new(params: BuiltinParams) -> Self {
        TotalCurrencyContract {
            deactivate_at: params.deactivate_at,
            name: params.name,
            owner_address: params
                .owner_address
                .expect("total current contract' needs owner address"),
        }
    }

    fn fail(&self, err_msg: String) -> ExecutionResult {
        ExecutionResult {
            gas_left: U256::zero(),
            status_code: ExecStatus::Failure,
            return_data: ReturnData::empty(),
            exception: err_msg,
            state_root: H256::default(),
        }
    }

    fn query_network_balance(&self, ext: &mut BuiltinExt, input: &[u8]) -> ExecutionResult {
        let result: Vec<u8> = ext.storage_at(&H128::from(input)).to_vec();
        let length: usize = result.len();
        ExecutionResult {
            gas_left: U256::zero(),
            status_code: ExecStatus::Success,
            return_data: ReturnData::new(result, 0, length),
            exception: String::default(),
            state_root: H256::default(),
        }
    }

    fn execute_update_total_balance(&self, ext: &mut BuiltinExt, input: &[u8]) -> ExecutionResult {
        if input.len() < 114 {
            return self.fail(String::from("internal error: input length < 114."));
        }

        let mut chain_id_bytes = [0x0; 16];
        chain_id_bytes[0] = input[0];
        let chain_id = H128(chain_id_bytes);

        let signum = input[1];
        let amount = &input[2..18];
        let sign = &input[18..114];

        // payload: chainid+signum+amount
        let payload = &input[0..18];

        // verify payload signature
        let signature = Ed25519Signature::from(sign.to_vec());
        let public = signature.get_public();
        let address = public_to_address_ed25519(&public);

        // parse from signature again to avoid borrow issue.
        let b = verify_signature_ed25519(public, signature, &H256::from(payload));
        if !b {
            return self.fail(String::from("internal error: verify signature failed."));
        }

        // verify owner address
        if self.owner_address != address {
            return self.fail(String::from("internal error: owner address doesn't match."));
        }

        // verify signum
        if signum != 0u8 && signum != 1u8 {
            return self.fail(String::from(
                "signum is invalid, possible value is 0 and 1.",
            ));
        }

        // payload processing
        let total_curr = U128::from(ext.storage_at(&chain_id));
        let value = U128::from(H128::from_slice(amount));

        let final_value: U128;
        if signum == 0u8 {
            // addition
            final_value = total_curr + value;
        } else {
            // subtraction
            if value > total_curr {
                return self.fail(String::from("internal error: value > total_curr."));
            }
            final_value = total_curr - value;
        }
        ext.set_storage(chain_id, H128::from(final_value));
        ExecutionResult {
            gas_left: U256::zero(),
            status_code: ExecStatus::Success,
            return_data: ReturnData::empty(),
            exception: String::default(),
            state_root: H256::default(),
        }
    }
}

impl BuiltinContract for TotalCurrencyContract {
    /// if this contract is active.
    /// @param at block number from which block this contract is deployed.
    fn is_active(&self, at: u64) -> bool {
        if self.deactivate_at.is_none() {
            return true;
        }
        if at < self.deactivate_at.unwrap() {
            return true;
        }
        false
    }

    /// contract name.
    fn name(&self) -> &str { &self.name }

    /// Define the input data format as the following:
    /// <p>
    /// <pre>
    ///   {@code
    ///   [<1b - chainId> | <1b - signum> | <16b - uint128 amount> | <96b signature>]
    ///   total: 1 + 1 + 16 + 96 = 114
    ///   }
    /// </pre>
    /// <p>
    /// Where the chainId is intended to be our current chainId, in the case of
    /// the first AION network this should be set to 1. Note the presence of signum
    /// byte (bit) to check for addition or subtraction
    /// <p>
    /// Note: as a consequence of us storing the pk and signature as part of the call
    /// we can send a transaction to this contract from any address. As long as we
    /// hold the private key preset in this contract.
    /// <p>
    /// Within the contract, the storage is modelled as the following:
    /// <pre>
    ///   {@code
    ///   [1, total]
    ///   [2, total]
    ///   [3, total]
    ///   ...
    ///   }
    /// </pre>
    /// <p>
    /// Therefore retrieval should be relatively simple. There is also a retrieval (query)
    /// function provided, given that the input length is 4. In such a case, the input value
    /// is treated as a integer indicating the chainId, and thus the corresponding offset
    /// in the storage row to query the value from.
    /// <p>
    /// <pre>
    ///     {@code
    ///     [<1b - chainId>]
    ///     }
    /// </pre>
    ///
    fn execute(&self, ext: &mut BuiltinExt, input: &[u8]) -> ExecutionResult {
        if input.len() == 1 {
            // query
            self.query_network_balance(ext, input)
        } else {
            // update
            self.execute_update_total_balance(ext, input)
        }
    }

    /// gas cost
    fn cost(&self, _input: &[u8]) -> U256 {
        // set to a default cost for now, this will need to be adjusted
        U256::from(21000)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use aion_types::{H128, U128, H256, U256, Address};
    use types::vms::ExecStatus;
    use super::TotalCurrencyContract;
    use precompiled::builtin::{BuiltinParams, BuiltinContract, BuiltinExt, BuiltinContext};
    use key::{sign_ed25519, Ed25519Secret, Ed25519KeyPair};
    use std::str::FromStr;
    use bytes::to_hex;
    use log_entry::LogEntry;

    struct TestBuiltinExt {
        map: BTreeMap<H128, H128>,
        context: BuiltinContext,
    }

    impl BuiltinExt for TestBuiltinExt {
        fn storage_at(&self, key: &H128) -> H128 {
            let ov = self.map.get(key);
            if ov.is_none() {
                H128::zero()
            } else {
                *ov.unwrap()
            }
        }

        #[allow(dead_code)]
        fn inc_nonce(&mut self, _a: &Address) { unimplemented!() }

        #[allow(dead_code)]
        fn transfer_balance(&mut self, _from: &Address, _to: &Address, _by: &U256) {
            unimplemented!()
        }

        #[allow(dead_code)]
        fn balance(&self, _address: &Address) -> U256 { unimplemented!() }

        fn set_storage(&mut self, key: H128, value: H128) { self.map.insert(key, value); }

        fn context(&self) -> &BuiltinContext { &self.context }

        fn storage_at_dword(&self, _key: &H128) -> H256 { H256::new() }

        fn set_storage_dword(&mut self, _key: H128, _value: H256) {}

        fn log(&mut self, _topics: Vec<H256>, _data: Option<&[u8]>) {}

        fn get_logs(&self) -> Vec<LogEntry> {
            unimplemented!();
        }

        fn add_balance(&mut self, _to: &Address, _incr: &U256) {
            unimplemented!();
        }
    }

    fn get_total_currency_contract() -> TotalCurrencyContract {
        TotalCurrencyContract::new(builtin_params())
    }

    fn get_ext() -> TestBuiltinExt {
        TestBuiltinExt {
            map: BTreeMap::new(),
            context: BuiltinContext {
                sender: "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .unwrap(),
                address: "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .unwrap(),
                tx_hash: H256::default(),
                origin_tx_hash: H256::default(),
            },
        }
    }

    fn construct_update_input(chain_id: u8, signum: u8, amount: u64) -> Vec<u8> {
        let mut payload = Vec::with_capacity(18);
        payload.push(chain_id);
        payload.push(signum);
        let amt = H128::from(U128::from(amount));
        payload.extend_from_slice(&amt.0);

        let secret = Ed25519Secret::from_str("7ea8af7d0982509cd815096d35bc3a295f57b2a078e4e25731e3ea977b9544626702b86f33072a55f46003b1e3e242eb18556be54c5ab12044c3c20829e0abb5").unwrap();
        let kp = Ed25519KeyPair::from_secret(secret).unwrap();
        let sig = sign_ed25519(&kp.secret(), &H256::from_slice(payload.as_slice())).unwrap();
        let sig_bytes: [u8; 96] = sig.into();

        payload.extend_from_slice(&sig_bytes);
        payload
    }

    fn builtin_params() -> BuiltinParams {
        BuiltinParams {
            activate_at: 0,
            deactivate_at: None,
            name: String::from("total_currency"),
            owner_address: Some(
                "a07bfd7baa8497fd43258a5442a26f277206f62a98668ae2212ab3f4c71a10c8"
                    .parse()
                    .unwrap(),
            ),
            contract_address: Some(
                "0000000000000000000000000000000000000000000000000000000000000100"
                    .parse()
                    .unwrap(),
            ),
        }
    }

    #[test]
    fn tcc_get_total_amount() {
        let contract = get_total_currency_contract();
        let input = [0u8]; // input = chain_id
        let result = contract.execute(&mut get_ext(), &input);
        assert!(result.status_code == ExecStatus::Success);
    }

    #[test]
    fn tcc_get_total_amount_empty_payload() {
        let contract = get_total_currency_contract();
        let input = []; // empty input size
        let result = contract.execute(&mut get_ext(), &input);
        assert!(result.status_code == ExecStatus::Failure);
        println!("error: {}", result.exception);
    }

    //    fn get_total_amount_insufficient_nrg() {}
    #[test]
    fn tcc_update_and_get_total_amount() {
        let contract = get_total_currency_contract();
        let mut ext = get_ext();

        let amount = 1000u64;
        let input_vector = construct_update_input(0u8, 0u8, amount);

        let result = contract.execute(&mut ext, &input_vector.as_slice());
        assert!(result.status_code == ExecStatus::Success);

        let query_input = [0u8];
        let query_result = { contract.execute(&mut ext, &query_input) };
        assert!(query_result.status_code == ExecStatus::Success);
        assert_eq!(
            U128::from(H128::from_slice(&*query_result.return_data)).as_u64(),
            amount
        );
    }

    #[test]
    fn tcc_update_and_get_different_chain_ids() {
        let contract = get_total_currency_contract();
        let mut ext = get_ext();

        let amount = 1000u64;
        let input_vector = construct_update_input(0u8, 0u8, amount);

        let result = contract.execute(&mut ext, &input_vector.as_slice());
        assert!(result.status_code == ExecStatus::Success);

        let query_input = [1u8];
        let query_result = { contract.execute(&mut ext, &query_input) };
        assert!(query_result.status_code == ExecStatus::Success);
        assert_eq!(
            U128::from(H128::from_slice(&*query_result.return_data)).as_u64(),
            0u64
        );
    }

    #[test]
    fn tcc_multiple_updates() {
        let contract = get_total_currency_contract();
        let mut ext = get_ext();

        let amount = 1000u64;
        let input_vector = construct_update_input(0u8, 0u8, amount);

        let mut result = contract.execute(&mut ext, &input_vector.as_slice());
        assert!(result.status_code == ExecStatus::Success);
        result = contract.execute(&mut ext, &input_vector.as_slice());
        assert!(result.status_code == ExecStatus::Success);
        result = contract.execute(&mut ext, &input_vector.as_slice());
        assert!(result.status_code == ExecStatus::Success);
        result = contract.execute(&mut ext, &input_vector.as_slice());
        assert!(result.status_code == ExecStatus::Success);

        let query_input = [0u8];
        let query_result = { contract.execute(&mut ext, &query_input) };
        assert!(query_result.status_code == ExecStatus::Success);
        assert_eq!(
            U128::from(H128::from_slice(&*query_result.return_data)).as_u64(),
            amount * 4
        );
    }

    #[test]
    fn tcc_update_total_incorrect_sig_size() {
        let contract = get_total_currency_contract();
        let mut ext = get_ext();
        let amount = 1000u64;
        let mut input_vector = construct_update_input(0u8, 0u8, amount);
        input_vector.truncate(100);
        let result = contract.execute(&mut ext, &input_vector.as_slice());
        assert!(result.status_code == ExecStatus::Failure);
        println!("error: {}", result.exception);
    }

    #[test]
    fn tcc_update_total_not_owner() {
        let params = BuiltinParams {
            activate_at: 0,
            deactivate_at: None,
            name: String::from("total_currency"),
            owner_address: Some(
                "a07bfd7baa8497fd43258a5442a26f277206f62a98668ae2212ab3f4c71a10c9"
                    .parse()
                    .unwrap(),
            ),
            contract_address: Some(
                "0000000000000000000000000000000000000000000000000000000000000100"
                    .parse()
                    .unwrap(),
            ),
        };
        let contract = TotalCurrencyContract::new(params);
        let mut ext = get_ext();
        let amount = 1000u64;
        let input_vector = construct_update_input(0u8, 0u8, amount);
        let result = contract.execute(&mut ext, &input_vector.as_slice());
        assert!(result.status_code == ExecStatus::Failure);
        println!("error: {}", result.exception);
    }

    #[test]
    fn tcc_update_total_incorrect_sig() {
        let contract = get_total_currency_contract();
        let mut ext = get_ext();
        let amount = 1000u64;
        let mut input_vector = construct_update_input(0u8, 0u8, amount);
        let input_slice = input_vector.as_mut_slice();
        input_slice[30] = !input_slice[30];
        let result = contract.execute(&mut ext, &input_slice);
        assert!(result.status_code == ExecStatus::Failure);
        println!("error: {}", result.exception);
    }

    #[test]
    fn tcc_subtract_total_amount() {
        let contract = get_total_currency_contract();
        let mut ext = get_ext();

        // First give some positive balance to take away.
        let amount = 1000u64;
        let input_vector = construct_update_input(0u8, 0u8, amount);

        let mut result = contract.execute(&mut ext, &input_vector.as_slice());
        assert!(result.status_code == ExecStatus::Success);
        result = contract.execute(&mut ext, &input_vector.as_slice());
        assert!(result.status_code == ExecStatus::Success);
        result = contract.execute(&mut ext, &input_vector.as_slice());
        assert!(result.status_code == ExecStatus::Success);

        // Remove the balance
        let subtract_input = construct_update_input(0u8, 1u8, amount);
        let subtract_result = contract.execute(&mut ext, &subtract_input.as_slice());
        assert!(subtract_result.status_code == ExecStatus::Success);

        // query
        let query_input = [0u8];
        let query_result = { contract.execute(&mut ext, &query_input) };
        assert!(query_result.status_code == ExecStatus::Success);
        assert_eq!(
            U128::from(H128::from_slice(&*query_result.return_data)).as_u64(),
            amount * 2
        );

        // remove to zero
        let subtract_result = contract.execute(&mut ext, &subtract_input.as_slice());
        assert!(subtract_result.status_code == ExecStatus::Success);
        let subtract_result = contract.execute(&mut ext, &subtract_input.as_slice());
        assert!(subtract_result.status_code == ExecStatus::Success);

        // query
        let query_result = { contract.execute(&mut ext, &query_input) };
        assert!(query_result.status_code == ExecStatus::Success);
        assert_eq!(
            U128::from(H128::from_slice(&*query_result.return_data)).as_u64(),
            0u64
        );
    }

    #[test]
    fn tcc_subtract_total_amount_below_zero() {
        let contract = get_total_currency_contract();
        let mut ext = get_ext();

        // First give some positive balance to take away.
        let amount = 1000u64;
        let input = construct_update_input(0u8, 1u8, amount);
        let result = contract.execute(&mut ext, &input.as_slice());
        assert!(result.status_code == ExecStatus::Failure);
        println!("error: {}", result.exception);

        // query
        let query_input = [0u8];
        let query_result = { contract.execute(&mut ext, &query_input) };
        assert!(query_result.status_code == ExecStatus::Success);
        println!("output buffer:{}", to_hex(&*query_result.return_data));
        assert_eq!(
            U128::from(H128::from_slice(&*query_result.return_data)).as_u64(),
            0u64
        );
    }

    #[test]
    fn tcc_bad_signum() {
        let contract = get_total_currency_contract();
        let mut ext = get_ext();
        let amount = 1000u64;
        // only 0 and 1 are valid.
        let input = construct_update_input(0u8, 2u8, amount);
        let result = contract.execute(&mut ext, &input.as_slice());
        assert!(result.status_code == ExecStatus::Failure);
        // "signum is invalid, possible value is 0 and 1."
        println!("error: {}", result.exception);
    }

    #[test]
    fn tcc_update_multiple_chains() {
        let contract = get_total_currency_contract();
        let mut ext = get_ext();
        let amount = 1000u64;
        let input0 = construct_update_input(0u8, 0u8, amount);
        let input1 = construct_update_input(1u8, 0u8, amount);
        let input2 = construct_update_input(16u8, 0u8, amount);

        let result = contract.execute(&mut ext, &input0.as_slice());
        assert!(result.status_code == ExecStatus::Success);
        let result = contract.execute(&mut ext, &input1.as_slice());
        assert!(result.status_code == ExecStatus::Success);
        let result = contract.execute(&mut ext, &input1.as_slice());
        assert!(result.status_code == ExecStatus::Success);
        let result = contract.execute(&mut ext, &input2.as_slice());
        assert!(result.status_code == ExecStatus::Success);
        let result = contract.execute(&mut ext, &input2.as_slice());
        assert!(result.status_code == ExecStatus::Success);
        let result = contract.execute(&mut ext, &input2.as_slice());
        assert!(result.status_code == ExecStatus::Success);
        let result = contract.execute(&mut ext, &input2.as_slice());
        assert!(result.status_code == ExecStatus::Success);

        //query0
        let query_input = [0u8];
        let query_result = { contract.execute(&mut ext, &query_input) };
        assert!(query_result.status_code == ExecStatus::Success);
        println!("output buffer:{}", to_hex(&*query_result.return_data));
        assert_eq!(
            U128::from(H128::from_slice(&*query_result.return_data)).as_u64(),
            amount
        );
        // query2
        let query_input = [1u8];
        let query_result = { contract.execute(&mut ext, &query_input) };
        assert!(query_result.status_code == ExecStatus::Success);
        println!("output buffer:{}", to_hex(&*query_result.return_data));
        assert_eq!(
            U128::from(H128::from_slice(&*query_result.return_data)).as_u64(),
            amount * 2
        );
        // query3
        let query_input = [16u8];
        let query_result = { contract.execute(&mut ext, &query_input) };
        assert!(query_result.status_code == ExecStatus::Success);
        println!("output buffer:{}", to_hex(&*query_result.return_data));
        assert_eq!(
            U128::from(H128::from_slice(&*query_result.return_data)).as_u64(),
            amount * 4
        );
    }
}
