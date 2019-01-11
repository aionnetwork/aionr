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

#[allow(unused)]

extern crate time;
extern crate rand;
extern crate libc;

use core::FastVM;
use ffi::EvmJit;
use context::ExecutionContext;
use vm::{ExecutionResult, CallType};
use env_info::EnvInfo;

type Bytes = Vec<u8>;

pub fn random_from_bytes(len: usize) -> Vec<u8> {
    let mut random_vec: Vec<u8> = Vec::<u8>::with_capacity(len);
    for _x in 0..len {
        let x = rand::random::<u8>();
        random_vec.push(x);
    }

    debug!(target: "vm", "new list = {:?}", random_vec);
    random_vec
}

extern crate aion_types;
use context::{execution_kind, TransactionResult};
use basetypes::{DataWord};
use aion_types::Address;

#[derive(Clone)]
pub struct FastVMTest {
    pub tx_hash: Vec<u8>,
    pub origin: Address,
    pub caller: Address,
    pub address: Address,
    pub block_coinbase: Address,
    pub block_number: u64,
    pub block_timestamp: u64,
    pub block_nrglimit: u64,
    pub block_difficulty: DataWord,

    pub nrg_price: DataWord,
    pub nrg_limit: u64,

    pub call_value: DataWord,
    pub call_data: Vec<u8>,

    pub tx_result: TransactionResult,

    pub depth: i32,
    pub kind: i32,
    pub flags: i32,
}

impl FastVMTest {
    pub fn new() -> Self {
        let hash_len = 32;
        let origin: Address = random_from_bytes(hash_len).as_slice().into();
        let current_time = time::get_time();
        let milliseconds = current_time.sec as i64 * 1000 + current_time.nsec as i64 / 1000 / 1000;

        FastVMTest {
            tx_hash: random_from_bytes(hash_len),
            origin: origin.clone(),
            caller: origin.clone(),
            address: random_from_bytes(hash_len).as_slice().into(),
            block_coinbase: random_from_bytes(hash_len).as_slice().into(),

            block_number: 1,
            block_nrglimit: 5000000,
            block_timestamp: milliseconds as u64 / 1000,
            block_difficulty: DataWord::new_with_long(0x100000000i64),

            nrg_price: DataWord::one(),
            nrg_limit: 1000000,

            call_value: DataWord::zero(),
            call_data: Vec::new(),

            tx_result: TransactionResult::new(),

            depth: 0,
            kind: execution_kind::CREATE,
            flags: 0,
        }
    }
}

impl Into<ExecutionContext> for FastVMTest {
    fn into(self) -> ExecutionContext {
        ExecutionContext {
            tx_hash: self.tx_hash,
            address: self.address,
            origin: self.origin,
            caller: self.caller,
            nrg_price: self.nrg_price,
            nrg_limit: self.nrg_limit as u64,
            call_value: self.call_value,
            call_data: self.call_data,
            depth: self.depth,
            kind: self.kind,
            flags: self.flags,
            block_coinbase: self.block_coinbase,
            block_number: self.block_number as u64,
            block_timestamp: self.block_timestamp as i64,
            block_nrglimit: self.block_nrglimit,
            block_difficulty: self.block_difficulty,
            result: self.tx_result,
        }
    }
}

use aion_types::{H128, U256, H256};
use vm::Ext;

trait DummyCallbacks {
    fn storage_at(&self, key: &H128) -> H128;
    fn set_storage(&mut self, key: H128, value: H128);
    fn exists(&self, address: &Address) -> bool;
    fn balance(&self, address: &Address) -> U256;
}

struct TestEnv<'a> {
    env_info: &'a EnvInfo,
}

impl<'a> Ext for TestEnv<'a> {
    fn storage_at(&self, key: &H128) -> H128 {
        debug!(target: "vm", "TEST<get_storage>: key = {}", key);
        return H128::new();
    }
    fn set_storage(&mut self, key: H128, value: H128) {
        debug!(target: "vm", "TEST<set_storage>: key = {}, value = {}", key, value);
    }
    /// Returns a value for given key.
    fn storage_at_dword(&self, _key: &H128) -> H256 { 0.into() }

    /// Stores a value for given key.
    fn set_storage_dword(&mut self, _key: H128, _value: H256) {}

    /// Determine whether an account exists.
    fn exists(&self, _address: &Address) -> bool { return true; }

    /// Determine whether an account exists and is not null (zero balance/nonce, no code).
    fn exists_and_not_null(&self, _address: &Address) -> bool { return true; }

    /// Balance of the origin account.
    fn origin_balance(&self) -> U256 { 0.into() }

    /// Returns address balance.
    fn balance(&self, _address: &Address) -> U256 { 0.into() }

    /// Returns the hash of one of the 256 most recent complete blocks.
    fn blockhash(&mut self, _number: &U256) -> H256 { 0.into() }

    /// Creates new contract.
    ///
    /// Returns gas_left and contract address if contract creation was succesfull.
    fn create(&mut self, _gas: &U256, _value: &U256, _code: &[u8]) -> ExecutionResult {
        ExecutionResult::default()
    }

    /// Message call.
    ///
    /// Returns Err, if we run out of gas.
    /// Otherwise returns call_result which contains gas left
    /// and true if subcall was successfull.
    fn call(
        &mut self,
        _gas: &U256,
        _sender_address: &Address,
        _receive_address: &Address,
        _value: Option<U256>,
        _data: &[u8],
        _code_address: &Address,
        _call_type: CallType,
        _static_flag: bool,
    ) -> ExecutionResult
    {
        ExecutionResult::default()
    }

    /// Returns code at given address
    fn extcode(&self, _address: &Address) -> ::std::sync::Arc<Vec<u8>> {
        ::std::sync::Arc::new(Vec::<u8>::new())
    }

    /// Returns code size at given address
    fn extcodesize(&self, _address: &Address) -> usize { return 0; }

    /// Creates log entry with given topics and data
    fn log(&mut self, _topics: Vec<H256>, _data: &[u8]) {}

    /// Should be called when contract commits suicide.
    /// Address to which funds should be refunded.
    fn suicide(&mut self, _refund_address: &Address) {}

    /// Returns environment info.
    fn env_info(&self) -> &EnvInfo { self.env_info }

    /// Returns current depth of execution.
    ///
    /// If contract A calls contract B, and contract B calls C,
    /// then A depth is 0, B is 1, C is 2 and so on.
    fn depth(&self) -> usize { return 0; }

    /// Increments sstore refunds count by 1.
    fn inc_sstore_clears(&mut self) {}

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
    fn save_code(&mut self, _code: Bytes) {}

    fn set_special_empty_flag(&mut self) {}
}

impl<'a> EvmJit<::libc::c_void> for TestEnv<'a> {
    fn to_evm_jit(&mut self) -> *mut ::libc::c_void { unsafe { ::std::mem::transmute(self) } }
    fn from_evm_jit(input: *const ::libc::c_void) -> *const Self {
        unsafe { ::std::mem::transmute(input) }
    }
}

#[test]
fn fastvm_env() {
    let context = FastVMTest::new();
    let mut instance = FastVM::new();
    let ext = TestEnv {
        env_info: &EnvInfo::default(),
    };
    let raw_env: *mut ::libc::c_void = unsafe { ::std::mem::transmute(Box::new(&ext)) };
    instance.init(raw_env);
    println!("raw_ext = {:?}", raw_env);

    let code = vec![0x61, 0x01, 0x02];
    let res = instance.run(&code, &mut context.into());
    println!("TEST<fastvm_env>: res = {:?}", res);
}

#[test]
fn operation_underflow() {
    let context = FastVMTest::new();
    let mut instance = FastVM::new();
    let ext = TestEnv {
        env_info: &EnvInfo::default(),
    };
    let raw_env: *mut ::libc::c_void = unsafe { ::std::mem::transmute(Box::new(&ext)) };
    instance.init(raw_env);
    println!("raw_ext = {:?}", raw_env);

    let code = vec![0x06, 0x05, 0x06];
    let res = instance.run(&code, &mut context.into());
    println!("TEST<fastvm_env>: res = {:?}", res);
}
