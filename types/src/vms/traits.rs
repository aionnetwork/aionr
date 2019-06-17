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

use std::sync::Arc;
use bytes::Bytes;
use aion_types::{ H128, U256, H256, Address };
use super::{ EnvInfo, ExecutionResult, CallType };

/// Externalities interface for EVMs
pub trait Ext {
    /// Returns a value for given key.
    fn storage_at(&self, key: &H128) -> H128;

    /// Stores a value for given key.
    fn set_storage(&mut self, key: H128, value: H128);

    /// Returns a value for given key.
    fn storage_at_dword(&self, key: &H128) -> H256;

    /// Stores a value for given key.
    fn set_storage_dword(&mut self, key: H128, value: H256);

    /// Determine whether an account exists.
    fn exists(&self, address: &Address) -> bool;

    /// Determine whether an account exists and is not null (zero balance/nonce, no code).
    fn exists_and_not_null(&self, address: &Address) -> bool;

    /// Balance of the origin account.
    fn origin_balance(&self) -> U256;

    /// Returns address balance.
    fn balance(&self, address: &Address) -> U256;

    /// Returns the hash of one of the 256 most recent complete blocks.
    fn blockhash(&mut self, number: &U256) -> H256;

    /// Creates new contract.
    ///
    /// Returns gas_left and contract address if contract creation was succesfull.
    fn create(&mut self, gas: &U256, value: &U256, code: &[u8]) -> ExecutionResult;

    /// Message call.
    ///
    /// Returns Err, if we run out of gas.
    /// Otherwise returns call_result which contains gas left
    /// and true if subcall was successfull.
    fn call(
        &mut self,
        gas: &U256,
        sender_address: &Address,
        receive_address: &Address,
        value: Option<U256>,
        data: &[u8],
        code_address: &Address,
        call_type: CallType,
        static_flag: bool,
    ) -> ExecutionResult;

    /// Returns code at given address
    fn extcode(&self, address: &Address) -> Arc<Bytes>;

    /// Returns code size at given address
    fn extcodesize(&self, address: &Address) -> usize;

    /// Creates log entry with given topics and data
    fn log(&mut self, topics: Vec<H256>, data: &[u8]);

    /// Should be called when contract commits suicide.
    /// Address to which funds should be refunded.
    fn suicide(&mut self, refund_address: &Address);

    /// Returns environment info.
    fn env_info(&self) -> &EnvInfo;

    /// Returns current depth of execution.
    ///
    /// If contract A calls contract B, and contract B calls C,
    /// then A depth is 0, B is 1, C is 2 and so on.
    fn depth(&self) -> usize;

    /// Increments sstore refunds count by 1.
    fn inc_sstore_clears(&mut self);

    /// Decide if any more operations should be traced. Passthrough for the VM trace.
    fn trace_next_instruction(&mut self, _pc: usize, _instruction: u8, _current_gas: U256) -> bool {
        false
    }

    /// Prepare to trace an operation. Passthrough for the VM trace.
    fn trace_prepare_execute(&mut self, _pc: usize, _instruction: u8, _gas_cost: U256) {}

    /// Trace the finalised execution of a single instruction.
    fn trace_executed(
        &mut self,
        _gas_used: U256,
        _stack_push: &[U256],
        _mem_diff: Option<(usize, &[u8])>,
        _store_diff: Option<(U256, U256)>,
    )
    {
    }

    /// Save code to newly created contract.
    fn save_code(&mut self, code: Bytes);

    /// get code
    fn code(&self, address: &Address) -> Option<Arc<Bytes>>;

    /// TODO: special account flag for fastvm which is empty but should be committed
    fn set_special_empty_flag(&mut self);

    /// avm set storage
    fn sstore(&mut self, address: &Address, key: Bytes, value: Bytes);

    /// avm get storage
    fn sload(&self, address: &Address, key: &Bytes) -> Option<Bytes>;

    // avm remove storage
    fn remove_storage(&mut self, a: &Address, key: Vec<u8>);

    /// avm create account
    fn create_account(&mut self, address: &Address);

    /// avm kill account
    fn kill_account(&mut self, address: &Address);

    /// avm increase balance
    fn inc_balance(&mut self, address: &Address, inc: &U256);

    /// avm decrease balance
    fn dec_balance(&mut self, address: &Address, dec: &U256);

    /// avm get nonce
    fn nonce(&self, address: &Address) -> u64;

    /// avm increase nonce
    fn inc_nonce(&mut self, address: &Address);

    /// avm save code at address
    fn save_code_at(&mut self, address: &Address, code: Bytes);

    fn touch_account(&mut self, address: &Address, index: i32);

    fn send_signal(&mut self, signal: i32);

    fn commit(&mut self);

    fn root(&self) -> H256;

    fn avm_log(&mut self, address: &Address, topics: Vec<H256>, data: Bytes, idx: i32);

    fn get_transformed_code(&self, address: &Address) -> Option<Arc<Bytes>>;

    fn save_transformed_code(&mut self, address: &Address, code: Bytes);

    fn get_objectgraph(&self, address: &Address) -> Option<Arc<Bytes>>;

    fn set_objectgraph(&mut self, address: &Address, data: Bytes);
}
