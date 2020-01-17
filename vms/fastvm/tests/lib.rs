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

#![warn(unused_extern_crates)]

extern crate time;
extern crate rand;
extern crate libc;
extern crate fastvm;
extern crate aion_types;
extern crate rustc_hex;
extern crate vm_common;
#[macro_use]
extern crate log;

use std::sync::Arc;
use std::collections::HashMap;
use std::convert::Into;
use fastvm::core::FastVM;
use fastvm::basetypes::{DataWord};
use fastvm::context::{execution_kind, TransactionResult, ExecutionContext};
use fastvm::ffi::EvmJit;
use aion_types::{Address, H128, U256, H256};
use vm_common::traits::Ext;
use vm_common::{ExecutionResult, CallType, EnvInfo};

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

trait DummyCallbacks {
    fn storage_at(&self, key: &H128) -> H128;
    fn set_storage(&mut self, key: H128, value: H128);
    fn exists(&self, address: &Address) -> bool;
    fn balance(&self, address: &Address) -> U256;
}

struct TestEnv<'a> {
    env_info: &'a EnvInfo,
    storage: HashMap<H128, H128>,
    storage_dword: HashMap<H128, H256>,
    accounts: HashMap<H256, bool>,
    _balance: HashMap<Address, H128>,
    log_topics: Vec<Vec<H256>>,
    log_data: Vec<u8>,
}

impl<'a> Ext for TestEnv<'a> {
    fn storage_at(&self, key: &H128) -> H128 { *self.storage.get(key).unwrap_or(&H128::default()) }

    fn set_storage(&mut self, key: H128, value: H128) { self.storage.insert(key, value); }
    /// Returns a value for given key.
    fn storage_at_dword(&self, key: &H128) -> H256 { return *self.storage_dword.get(key).unwrap(); }

    /// Stores a value for given key.
    fn set_storage_dword(&mut self, key: H128, value: H256) {
        self.storage_dword.insert(key, value);
    }

    fn remove_storage(&mut self, _a: &Address, _key: Vec<u8>) {}

    /// Determine whether an account exists.
    fn exists(&self, address: &Address) -> bool { return *self.accounts.get(address).unwrap(); }

    /// Determine whether an account exists and is not null (zero balance/nonce, no code).
    fn exists_and_not_null(&self, _address: &Address) -> bool { return true; }

    /// Balance of the origin account.
    fn origin_balance(&self) -> U256 { 0.into() }

    /// Returns address balance.
    fn balance(&self, _address: &Address) -> U256 { 0.into() }

    /// Returns the hash of one of the 256 most recent complete blocks.
    fn blockhash(&mut self, _number: &U256) -> H256 {
        println!("Ext: get blockhash of {:?}", _number);
        0xf.into()
    }

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
    fn log(&mut self, topics: Vec<H256>, data: &[u8]) {
        self.log_topics.push(topics);
        self.log_data.extend_from_slice(data);
    }

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

    fn code(&self, _address: &Address) -> Option<Arc<Bytes>> { None }

    fn sstore(&mut self, _address: &Address, _key: Bytes, _value: Bytes) {}

    fn sload(&self, _address: &Address, _key: &Bytes) -> Option<Bytes> { None }

    fn create_account(&mut self, _address: &Address) {}

    fn kill_account(&mut self, _address: &Address) {}

    fn inc_balance(&mut self, _address: &Address, _inc: &U256) {}

    fn dec_balance(&mut self, _address: &Address, _dec: &U256) {}

    fn nonce(&self, _address: &Address) -> u64 { 0 }

    fn inc_nonce(&mut self, _address: &Address) {}

    fn save_code_at(&mut self, _address: &Address, _code: Bytes) {}

    fn touch_account(&mut self, _address: &Address, _index: i32) {}

    fn send_signal(&mut self, _signal: i32) {}

    fn commit(&mut self) {}

    fn root(&self) -> H256 { H256::default() }

    fn avm_log(&mut self, _address: &Address, _topics: Vec<H256>, _data: Bytes, _idx: i32) {}

    fn get_transformed_code(&self, _address: &Address) -> Option<Arc<Bytes>> { None }

    fn save_transformed_code(&mut self, _address: &Address, _code: Bytes) {}

    fn get_objectgraph(&self, _address: &Address) -> Option<Arc<Bytes>> { None }

    fn set_objectgraph(&mut self, _address: &Address, _data: Bytes) {}
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
        accounts: HashMap::new(),
        _balance: HashMap::new(),
        storage: HashMap::new(),
        storage_dword: HashMap::new(),
        log_topics: Vec::new(),
        log_data: Vec::new(),
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
        accounts: HashMap::new(),
        _balance: HashMap::new(),
        storage: HashMap::new(),
        storage_dword: HashMap::new(),
        log_topics: Vec::new(),
        log_data: Vec::new(),
    };
    let raw_env: *mut ::libc::c_void = unsafe { ::std::mem::transmute(Box::new(&ext)) };
    instance.init(raw_env);
    println!("raw_ext = {:?}", raw_env);

    let code = vec![0x06, 0x05, 0x06];
    let res = instance.run(&code, &mut context.into());
    println!("TEST<fastvm_env>: res = {:?}", res);
}

#[test]
fn evm_storage() {
    let context = FastVMTest::new();
    let mut instance = FastVM::new();
    let ext = TestEnv {
        env_info: &EnvInfo::default(),
        accounts: HashMap::new(),
        _balance: HashMap::new(),
        storage: HashMap::new(),
        storage_dword: HashMap::new(),
        log_topics: Vec::new(),
        log_data: Vec::new(),
    };
    let raw_env: *mut ::libc::c_void = unsafe { ::std::mem::transmute(Box::new(&ext as &dyn Ext)) };
    instance.init(raw_env);
    println!("raw_ext = {:?}", raw_env);

    let code = vec![0x60, 0x01, 0x60, 0x02, 0x55];
    let res = instance.run(&code, &mut context.into());
    println!("TEST<fastvm_env>: res = {:?}", res);
    assert_eq!(
        ext.storage_at(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02u8].into()),
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01u8].into()
    );
}

#[test]
fn evm_mstore() {
    let context = FastVMTest::new();
    let mut instance = FastVM::new();
    let ext = TestEnv {
        env_info: &EnvInfo::default(),
        accounts: HashMap::new(),
        _balance: HashMap::new(),
        storage: HashMap::new(),
        storage_dword: HashMap::new(),
        log_topics: Vec::new(),
        log_data: Vec::new(),
    };
    let raw_env: *mut ::libc::c_void = unsafe { ::std::mem::transmute(Box::new(&ext as &dyn Ext)) };
    instance.init(raw_env);

    // first mstore, then mload
    let code = vec![
        0x60, 0x0f, 0x60, 0x02, 0x52, 0x60, 0x02, 0x51, 0x60, 0x10, 0x60, 0x02, 0xf3,
    ];
    let res = instance.run(&code, &mut context.clone().into());
    println!("UT: evm_log, topics = {:?}", ext.log_topics);
    assert_eq!(
        res.2,
        vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0f]
    );
}

#[test]
fn evm_log() {
    let context = FastVMTest::new();
    let mut instance = FastVM::new();
    let mut ext = TestEnv {
        env_info: &EnvInfo::default(),
        accounts: HashMap::new(),
        _balance: HashMap::new(),
        storage: HashMap::new(),
        storage_dword: HashMap::new(),
        log_topics: Vec::new(),
        log_data: Vec::new(),
    };
    let raw_env: *mut ::libc::c_void = unsafe { ::std::mem::transmute(Box::new(&ext as &dyn Ext)) };
    instance.init(raw_env);

    // LOG0
    let code = vec![0x60, 0x01, 0x60, 0x02, 0xa0];
    let _res = instance.run(&code, &mut context.clone().into());
    println!("UT: evm_log, topics = {:?}", ext.log_topics);
    assert_eq!(ext.log_topics.len(), 1);
    assert!(ext.log_topics[0].is_empty());

    ext.log_topics.clear();
    ext.log_data.clear();

    instance.init(raw_env);
    // LOG1
    // set M[0x02] = 0xaf
    let code = vec![
        0x60, 0xaf, 0x60, 0x02, 0x52, 0x60, 0x03, 0x60, 0x00, 0x60, 0x1, 0x60, 0x11, 0xa1,
    ];
    // let code = vec![0x60, 0x01, 0x60, 0x02, 0xa0];
    let _res = instance.run(&code, &mut context.clone().into());

    println!("topics = {:?}, data = {:?}", ext.log_topics, ext.log_data);
    assert_eq!(ext.log_topics.len(), 1);
    let expected_topic: H256 = [
        0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0x03,
    ]
        .into();
    assert_eq!(ext.log_topics[0].pop().unwrap(), expected_topic);
    assert_eq!(ext.log_data, [0xafu8]);
}

#[test]
fn blockhash() {
    let context = FastVMTest::new();
    let mut instance = FastVM::new();
    let ext = TestEnv {
        env_info: &EnvInfo::default(),
        accounts: HashMap::new(),
        _balance: HashMap::new(),
        storage: HashMap::new(),
        storage_dword: HashMap::new(),
        log_topics: Vec::new(),
        log_data: Vec::new(),
    };
    let raw_env: *mut ::libc::c_void = unsafe { ::std::mem::transmute(Box::new(&ext as &dyn Ext)) };
    instance.init(raw_env);

    // 0x40
    let code = vec![
        0x60, 0x01, 0x40, 0x60, 0x02, 0x52, 0x60, 0x12, 0x52, 0x60, 0x20, 0x60, 0x02, 0xf3,
    ];
    let res = instance.run(&code, &mut context.clone().into());
    assert_eq!(
        res.2,
        vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0xf,
        ]
    );
}

#[test]
fn sha3() {
    let context = FastVMTest::new();
    let mut instance = FastVM::new();
    let ext = TestEnv {
        env_info: &EnvInfo::default(),
        accounts: HashMap::new(),
        _balance: HashMap::new(),
        storage: HashMap::new(),
        storage_dword: HashMap::new(),
        log_topics: Vec::new(),
        log_data: Vec::new(),
    };
    let raw_env: *mut ::libc::c_void = unsafe { ::std::mem::transmute(Box::new(&ext as &dyn Ext)) };
    instance.init(raw_env);

    // 0x20: compute sha3(0xff)
    let code = vec![
        0x60, 0xff, 0x60, 0x00, 0x52, 0x60, 0x10, 0x60, 0x00, 0x20, 0x60, 0x10, 0x52, 0x60, 0x20,
        0x52, 0x60, 0x20, 0x60, 0x10, 0xf3,
    ];
    let res = instance.run(&code, &mut context.clone().into());
    assert_eq!(
        res.2,
        vec![
            131, 193, 186, 50, 43, 185, 25, 210, 12, 46, 9, 202, 112, 253, 39, 188, 36, 86, 23,
            169, 233, 171, 213, 49, 91, 138, 250, 235, 196, 19, 96, 68,
        ]
    );
}

use rustc_hex::FromHex;

#[test]
fn invalid_gas() {
    let mut context = FastVMTest::new();
    context.nrg_limit = 0; //9223372036854775808;
    let mut instance = FastVM::new();
    let ext = TestEnv {
        env_info: &EnvInfo::default(),
        accounts: HashMap::new(),
        _balance: HashMap::new(),
        storage: HashMap::new(),
        storage_dword: HashMap::new(),
        log_topics: Vec::new(),
        log_data: Vec::new(),
    };
    let raw_env: *mut ::libc::c_void = unsafe { ::std::mem::transmute(Box::new(&ext as &dyn Ext)) };
    instance.init(raw_env);
    //let code = "d0d1188a00000000000000000000000000000032".from_hex().unwrap();
    // let code = vec![0x60, 0x50];
    println!("{:?}-{:?}", i64::max_value(), i64::min_value());
    let code = "60506040523415600f5760006000fd5b5b5b600115601b576011565b5b6020565b603a80602d6000396000f30060506040526008565b60006000fd00a165627a7a72305820c39f9e61953f77cbe7316aee3ed72ba5914ee08019d883f16b87cff04a1c829d0029".from_hex().unwrap();
    println!("code = {:?}", code);
    let res = instance.run(&code, &mut context.clone().into());
    println!("{:?}", res);
}
