/*******************************************************************************
 * Copyright (c) 2015-2018 Parity Technologies (UK) Ltd.
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

//! Ethereum-like state machine definition.

use std::collections::BTreeMap;
use std::cmp;
use std::sync::Arc;

use block::{ExecutedBlock, IsBlock};
use precompiled::builtin::BuiltinContract;
use types::error::Error;
use executive::{Executive};
use header::{BlockNumber, Header};
use spec::CommonParams;
use state::{CleanupMode, Substate};
use transaction::{SYSTEM_ADDRESS, UnverifiedTransaction, SignedTransaction};
use aion_types::{U256, H256, Address};
use vms::{ActionParams, ActionValue, CallType, ParamsType};

/// An ethereum-like state machine.
#[cfg_attr(test, derive(Default))]
pub struct EthereumMachine {
    params: CommonParams,
    builtins: Arc<BTreeMap<Address, Box<dyn BuiltinContract>>>,
    premine: U256,
}

impl EthereumMachine {
    /// Regular ethereum machine.
    pub fn regular(
        params: CommonParams,
        builtins: BTreeMap<Address, Box<dyn BuiltinContract>>,
        premine: U256,
    ) -> EthereumMachine
    {
        EthereumMachine {
            params,
            builtins: Arc::new(builtins),
            premine,
        }
    }
}

impl EthereumMachine {
    /// Execute a call as the system address.
    pub fn execute_as_system(
        &self,
        block: &mut ExecutedBlock,
        contract_address: Address,
        gas: U256,
        data: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, Error>
    {
        let env_info = {
            let mut env_info = block.env_info();
            env_info.gas_limit = env_info.gas_used + gas;
            env_info
        };

        let mut state = block.state_mut();
        let params = ActionParams {
            code_address: contract_address.clone(),
            address: contract_address.clone(),
            sender: SYSTEM_ADDRESS.clone(),
            origin: SYSTEM_ADDRESS.clone(),
            gas,
            gas_price: 0.into(),
            value: ActionValue::Transfer(0.into()),
            code: state.code(&contract_address)?,
            code_hash: Some(state.code_hash(&contract_address)?),
            data,
            call_type: CallType::Call,
            static_flag: false,
            params_type: ParamsType::Separate,
            transaction_hash: H256::default(),
            original_transaction_hash: H256::default(),
            nonce: 0,
        };
        let mut ex = Executive::new(&mut state, &env_info, self);
        let mut substate = Substate::new();
        let result = ex.call(params, &mut substate);
        match result.exception.as_str() {
            "" => {
                return Ok(result.return_data.to_vec());
            }
            error => {
                warn!(target:"executive","Encountered error on making system call: {}", error);
                return Ok(Vec::new());
            }
        }
    }

    /// Push last known block hash to the state.
    fn push_last_hash(&self, _block: &mut ExecutedBlock) -> Result<(), Error> {
        // originally eip210. store block hash to a contract.
        Ok(())
    }

    /// Logic to perform on a new block: updating last hashes.
    pub fn on_new_block(&self, block: &mut ExecutedBlock) -> Result<(), Error> {
        self.push_last_hash(block)?;
        Ok(())
    }

    /// Populate a header's gas limit based on its parent's header.
    /// Usually implements the chain scoring rule based on weight.
    /// The gas floor target must not be lower than the engine's minimum gas limit.
    pub fn set_gas_limit_from_parent(
        &self,
        header: &mut Header,
        parent: &Header,
        gas_floor_target: U256,
        gas_ceil_target: U256,
    )
    {
        // clamped-decay
        header.set_gas_limit({
            let gas_limit = parent.gas_limit().clone();
            let bound_divisor = self.params().gas_limit_bound_divisor;
            if gas_limit < gas_floor_target {
                gas_limit + gas_limit / bound_divisor
            } else if gas_limit > gas_ceil_target {
                gas_limit - gas_limit / bound_divisor
            } else {
                let gas_used: i64 = parent.gas_used().low_u64() as i64;
                let gas_limit_i64: i64 = gas_limit.low_u64() as i64;
                let bound_divisor_i64: i64 = bound_divisor.low_u64() as i64;
                let delta: i64 = (gas_used * 4 / 3 - gas_limit_i64) / bound_divisor_i64;
                cmp::max(0, gas_limit_i64 + delta).into()
            }
        });
    }

    /// Get the general parameters of the chain.
    pub fn params(&self) -> &CommonParams { &self.params }

    /// set monetary policy
    pub fn set_monetary(&mut self, block_number: u64) {
        self.params.monetary_policy_update = Some(block_number);
    }

    /// Builtin-contracts for the chain..
    pub fn builtins(&self) -> &BTreeMap<Address, Box<dyn BuiltinContract>> { &*self.builtins }

    pub fn premine(&self) -> U256 { self.premine }

    /// Attempt to get a handle to a built-in contract.
    /// Only returns references to activated built-ins.
    // TODO: builtin contract routing - to do this properly, it will require removing the built-in configuration-reading logic
    // from Spec into here and removing the Spec::builtins field.
    pub fn builtin(
        &self,
        a: &Address,
        block_number: BlockNumber,
    ) -> Option<&Box<dyn BuiltinContract>>
    {
        self.builtins().get(a).and_then(|b| {
            if b.is_active(block_number) {
                Some(b)
            } else {
                None
            }
        })
    }

    /// Some intrinsic operation parameters; by default they take their value from the `spec()`'s `engine_params`.
    pub fn maximum_extra_data_size(&self) -> usize { self.params().maximum_extra_data_size }

    /// The nonce with which accounts begin at given block.
    pub fn account_start_nonce(&self, _block: u64) -> U256 {
        // self.params().account_start_nonce
        U256::zero()
    }

    /// Verify a transaction's signature is valid
    pub fn verify_transaction_signature(
        &self,
        t: UnverifiedTransaction,
        _header: &Header,
    ) -> Result<SignedTransaction, Error>
    {
        Ok(SignedTransaction::new(t)?)
    }

    /// Does basic verification of the transaction.
    pub fn verify_transaction_basic(
        &self,
        t: &UnverifiedTransaction,
        block_num: Option<BlockNumber>,
    ) -> Result<(), Error>
    {
        if block_num.is_some() {
            t.is_allowed_type(self.params().monetary_policy_update, block_num.unwrap())?;
        }
        t.verify_basic(None)?;

        Ok(())
    }
}

/// Auxiliary data fetcher for an Ethereum machine. In Ethereum-like machines
/// there are two kinds of auxiliary data: bodies and receipts.
#[derive(Default, Clone)]
pub struct AuxiliaryData<'a> {
    /// The full block bytes, including the header.
    pub bytes: Option<&'a [u8]>,
    /// The block receipts.
    pub receipts: Option<&'a [::receipt::Receipt]>,
}

/// Type alias for a function we can make calls through synchronously.
/// Returns the call result and state proof for each call.
pub type Call<'a> = dyn Fn(Address, Vec<u8>) -> Result<(Vec<u8>, Vec<Vec<u8>>), String> + 'a;

/// Request for auxiliary data of a block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AuxiliaryRequest {
    /// Needs the body.
    Body,
    /// Needs the receipts.
    Receipts,
    /// Needs both body and receipts.
    Both,
}

impl ::aion_machine::Machine for EthereumMachine {
    type Header = Header;

    type LiveBlock = ExecutedBlock;
    type EngineClient = dyn (::client::EngineClient);
    type AuxiliaryRequest = AuxiliaryRequest;

    type Error = Error;
}

impl<'a> ::aion_machine::LocalizedMachine<'a> for EthereumMachine {
    type StateContext = Call<'a>;
    type AuxiliaryData = AuxiliaryData<'a>;
}

impl ::aion_machine::WithBalances for EthereumMachine {
    fn balance(&self, live: &ExecutedBlock, address: &Address) -> Result<U256, Error> {
        live.state().balance(address).map_err(Into::into)
    }

    fn add_balance(
        &self,
        live: &mut ExecutedBlock,
        address: &Address,
        amount: &U256,
    ) -> Result<(), Error>
    {
        live.state_mut()
            .add_balance(address, amount, CleanupMode::NoEmpty)
            .map_err(Into::into)
    }
}
