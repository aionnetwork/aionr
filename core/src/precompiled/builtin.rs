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

use aion_types::{U256, Address, H128, H256};
use ajson;
use super::total_currency_contract::TotalCurrencyContract;
use super::edverify_contract::EDVerifyContract;
use super::blake2b_hash_contract::Blake2bHashContract;
use super::tx_hash_contract::TxHashContract;
use super::atb::token_bridge_contract::TokenBridgeContract;
use std::fmt;
use state::{State, Substate, Backend as StateBackend,CleanupMode};
use vms::vm::ExecutionResult;
use log_entry::LogEntry;

pub trait BuiltinContract: Send + Sync {
    /// gas cost.
    fn cost(&self, input: &[u8]) -> U256;
    /// is contract active at the given block.
    fn is_active(&self, at: u64) -> bool;
    /// contract name.
    fn name(&self) -> &str;
    /// execute the contract.
    fn execute(&self, ext: &mut BuiltinExt, input: &[u8]) -> ExecutionResult;
}

/// aion precompiled contracts
pub fn builtin_contract(params: BuiltinParams) -> Box<BuiltinContract> {
    trace!(target:"builtin","initialize builtin contract: {}", params);
    let name = params.name.clone();
    match name.as_ref() {
        "total_currency_contract" => {
            Box::new(TotalCurrencyContract::new(params)) as Box<BuiltinContract>
        }
        "atb" => Box::new(TokenBridgeContract::new(params)) as Box<BuiltinContract>,
        "ed_verify" => Box::new(EDVerifyContract::new(params)) as Box<BuiltinContract>,
        "tx_hash" => Box::new(TxHashContract::new(params)) as Box<BuiltinContract>,
        "blake2b_hash" => Box::new(Blake2bHashContract::new(params)) as Box<BuiltinContract>,
        _ => panic!("invalid builtin name: {}", name),
    }
}

/// builtin contract common parameters set in spec file.
#[derive(Clone)]
pub struct BuiltinParams {
    // block number from which this contract is active.
    pub activate_at: u64,
    // block number from which this contract is inactive.
    pub deactivate_at: Option<u64>,
    /// builtin contract name.
    pub name: String,
    /// owner address.
    pub owner_address: Option<Address>,
    /// contract address.
    pub contract_address: Option<Address>,
}

impl fmt::Display for BuiltinParams {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "builtin params: name={}, activate_at={}, deactivate_at={:?}, contract_address={:?}, \
             owner_address={:?}",
            self.name,
            self.activate_at,
            self.deactivate_at,
            self.contract_address,
            self.owner_address
        )
    }
}

impl From<ajson::spec::Builtin> for BuiltinParams {
    fn from(b: ajson::spec::Builtin) -> Self {
        let params = BuiltinParams {
            name: b.name.clone(),
            activate_at: b.activate_at.map(Into::into).unwrap_or(0),
            deactivate_at: b.deactivate_at.map(Into::into),
            owner_address: b.owner_address.map(|a| a.into()),
            contract_address: b.address.map(|a| a.into()),
        };
        params
    }
}

pub struct BuiltinContext {
    pub sender: Address,
    pub address: Address,
    pub tx_hash: H256,
    pub origin_tx_hash: H256,
}

pub trait BuiltinExt {
    /// Returns a value for given key.
    fn storage_at(&self, key: &H128) -> H128;

    /// Stores a value for given key.
    fn set_storage(&mut self, key: H128, value: H128);

    /// Returns a 32 bytes value for given key.
    fn storage_at_dword(&self, key: &H128) -> H256;

    /// Stores a 32 bytes value for given key.
    fn set_storage_dword(&mut self, key: H128, value: H256);

    fn context(&self) -> &BuiltinContext;

    fn inc_nonce(&mut self, a: &Address);

    fn transfer_balance(&mut self, from: &Address, to: &Address, by: &U256);
    /// Add log
    fn log(&mut self, topics: Vec<H256>, data: Option<&[u8]>);

    fn get_logs(&self) -> Vec<LogEntry>;

    fn balance(&self, address: &Address) -> U256;

    fn add_balance(&mut self, to: &Address, incr: &U256);
}

pub struct BuiltinExtImpl<'a, B: 'a>
where B: StateBackend
{
    state: &'a mut State<B>,
    substate: &'a mut Substate,
    context: BuiltinContext,
}

impl<'a, B: 'a> BuiltinExtImpl<'a, B>
where B: StateBackend
{
    pub fn new(
        state: &'a mut State<B>,
        context: BuiltinContext,
        substate: &'a mut Substate,
    ) -> Self
    {
        BuiltinExtImpl {
            state: state,
            substate: substate,
            context: context,
        }
    }

    #[cfg(test)]
    pub fn change_context(&mut self, new_context: BuiltinContext) { self.context = new_context; }
}

impl<'a, B: 'a> BuiltinExt for BuiltinExtImpl<'a, B>
where B: StateBackend
{
    fn storage_at(&self, key: &H128) -> H128 {
        self.state
            .storage_at(&self.context.address, key)
            .expect("Fatal error occurred when getting storage.")
    }

    fn set_storage(&mut self, key: H128, value: H128) {
        self.state
            .set_storage(&self.context.address, key, value)
            .expect("Fatal error occurred when putting storage.")
    }

    fn storage_at_dword(&self, key: &H128) -> H256 {
        self.state
            .storage_at_dword(&self.context.address, key)
            .expect("Fatal error occurred when getting storage.")
    }

    fn set_storage_dword(&mut self, key: H128, value: H256) {
        self.state
            .set_storage_dword(&self.context.address, key, value)
            .expect("Fatal error occurred when putting storage.")
    }

    fn context(&self) -> &BuiltinContext { &self.context }

    fn log(&mut self, topics: Vec<H256>, data: Option<&[u8]>) {
        self.substate.logs.push(LogEntry {
            address: self.context.address.clone(),
            topics: topics,
            data: match data {
                Some(value) => value.to_vec(),
                None => Vec::new(),
            },
        });
    }

    fn get_logs(&self) -> Vec<LogEntry> { self.substate.logs.clone() }

    fn inc_nonce(&mut self, a: &Address) {
        self.state
            .inc_nonce(a)
            .expect("Fatal error occurred when incrementing nonce.")
    }

    fn transfer_balance(&mut self, from: &Address, to: &Address, by: &U256) {
        self.state
            .transfer_balance(from, to, by, CleanupMode::ForceCreate)
            .expect("Fatal error occurred when transfering balance.")
    }

    fn add_balance(&mut self, to: &Address, incr: &U256) {
        self.state
            .add_balance(to, incr, CleanupMode::ForceCreate)
            .expect("Fatal error occurred when adding balance.")
    }

    fn balance(&self, address: &Address) -> U256 {
        self.state
            .balance(address)
            .expect("Fatal error occurred when getting balance.")
    }
}

#[cfg(test)]
mod tests {}
