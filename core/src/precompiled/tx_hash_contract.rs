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
use aion_types::U256;
use vms::{ExecutionResult, ExecStatus, ReturnData};

pub struct TxHashContract {
    /// contract name
    name: String,
    /// block number from which this contract is supported
    activate_at: u64,
}

impl TxHashContract {
    pub fn new(params: BuiltinParams) -> Self {
        TxHashContract {
            name: params.name,
            activate_at: params.activate_at,
        }
    }
}

impl BuiltinContract for TxHashContract {
    fn cost(&self, _input: &[u8]) -> U256 { U256::from(20) }

    fn is_active(&self, at: u64) -> bool { at >= self.activate_at }

    fn name(&self) -> &str { &self.name }

    fn execute(&self, ext: &mut BuiltinExt, _input: &[u8]) -> ExecutionResult {
        let tx_hash = ext.context().origin_tx_hash.clone();
        ExecutionResult {
            gas_left: U256::zero(),
            status_code: ExecStatus::Success,
            return_data: ReturnData::new(tx_hash.to_vec(), 0, tx_hash.len()),
            exception: String::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TxHashContract;
    use precompiled::builtin::{BuiltinParams, BuiltinExtImpl, BuiltinContext, BuiltinContract};
    use tests::helpers::get_temp_state;
    use state::Substate;
    use vms::ExecStatus;
    use aion_types::{H256, Address};
    use rustc_hex::ToHex;
    use bytes::to_hex;

    #[test]
    fn test_txhash_contract() {
        let contract = TxHashContract::new(BuiltinParams {
            activate_at: 9200000,
            deactivate_at: None,
            name: String::from("txhash"),
            owner_address: None,
            contract_address: None,
        });
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let random_txhash = H256::random();
        let mut ext = BuiltinExtImpl::new(
            state,
            BuiltinContext {
                sender: Address::zero(),
                address: Address::zero(),
                tx_hash: random_txhash,
                origin_tx_hash: random_txhash,
            },
            substate,
        );
        let result = contract.execute(&mut ext, &vec![]);
        assert_eq!(result.status_code, ExecStatus::Success);
        let ret_data = result.return_data;
        assert_eq!(to_hex(&ret_data.mem), random_txhash.as_ref().to_hex());
    }
}
