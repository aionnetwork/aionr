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

//! Evm factory.
//!
use aion_types::U256;

use fastvm::{FastVM, EvmStatusCode};
use fastvm::basetypes::DataWord;
use fastvm::context::{execution_kind, ExecutionContext, TransactionResult};

use fastvm::vm::{ExecutionResult, Ext, ActionParams, ActionValue, ReturnData, CallType};
use fastvm::basetypes::constants::GAS_CODE_DEPOSIT;
use std::sync::Arc;

use aion_types::{U128};

pub trait Factory {
    fn exec(&mut self, params: ActionParams, ext: &mut Ext) -> ExecutionResult;
}

#[derive(Clone)]
pub struct FastVMFactory {
    instance: FastVM,
}

impl FastVMFactory {
    /// Create new instance of FastVM factory, with a size in bytes
    /// for caching jump destinations.
    pub fn new() -> Self {
        FastVMFactory {
            instance: FastVM::new(),
        }
    }
}

impl Factory for FastVMFactory {
    fn exec(&mut self, params: ActionParams, ext: &mut Ext) -> ExecutionResult {
        assert!(
            params.gas <= U256::from(i64::max_value() as u64),
            "evmjit max gas is 2 ^ 63"
        );
        assert!(
            params.gas_price <= U256::from(i64::max_value() as u64),
            "evmjit max gas is 2 ^ 63"
        );

        let raw_code = Arc::into_raw(params.code.unwrap());
        let code: &Vec<u8> = unsafe { ::std::mem::transmute(raw_code) };

        let call_data = params.data.unwrap_or_else(Vec::new);

        if code.is_empty() {
            ext.set_special_empty_flag();
            return ExecutionResult {
                gas_left: params.gas,
                status_code: EvmStatusCode::Success,
                return_data: ReturnData::empty(),
                exception: String::default(),
            };
        }

        let gas = params.gas.low_u64();
        let gas_price = DataWord::new_with_long(params.gas_price.low_u64() as i64);
        let address = params.address;
        let caller = params.sender;
        let origin = params.origin;
        let transfer_value = match params.value {
            ActionValue::Transfer(val) => <[u8; 16]>::from(U128::from(val)),
            ActionValue::Apparent(val) => <[u8; 16]>::from(U128::from(val)),
        };
        let mut call_value = DataWord::new();
        call_value.data = transfer_value.to_vec();
        debug!(target: "vm", "call_data = {:?}", call_data);
        debug!(target: "vm", "gas limit = {:?}", gas);

        let author = ext.env_info().author.clone();
        let difficulty = <[u8; 16]>::from(U128::from(ext.env_info().difficulty));
        let mut block_difficulty = DataWord::new();
        block_difficulty.data = difficulty.to_vec();
        let gas_limit = ext.env_info().gas_limit.low_u64();
        let number = ext.env_info().number;
        // don't really know why jit timestamp is int..
        let timestamp = ext.env_info().timestamp as i64;
        // from fastvm, no use in aion
        let tx_hash = vec![0; 32];
        let tx_result = TransactionResult::new();
        let depth = ext.depth() as i32;
        let kind = match params.call_type {
            CallType::None => execution_kind::CREATE,
            CallType::Call => execution_kind::CALL,
            CallType::CallCode => execution_kind::CALLCODE,
            CallType::DelegateCall => execution_kind::DELEGATECALL,
            CallType::StaticCall => execution_kind::CALL,
        };
        let flags: i32 = params.static_flag.into();

        let mut ctx = ExecutionContext::new(
            tx_hash,
            address,
            origin,
            caller,
            gas_price,
            gas,
            call_value,
            call_data,
            depth,
            kind,
            flags,
            author,
            number,
            timestamp,
            gas_limit,
            block_difficulty,
            tx_result,
        );
        let inst = &mut self.instance;
        let ext_ptr: *mut ::libc::c_void = unsafe { ::std::mem::transmute(Box::new(ext)) };
        inst.init(ext_ptr);
        let res = inst.run(code, &mut ctx);

        let ext_post: &mut Box<Ext> = unsafe { ::std::mem::transmute(ext_ptr) };
        let mut status_code = res.0;
        let mut gas_left = U256::from(res.1);
        let return_data = res.2;
        let return_data_length = return_data.len();

        // Panic if internal error occurred (problem in local node client)
        if status_code == EvmStatusCode::InternalError {
            panic!("Internal error occurred");
        }

        // Try to save code data if creating new contracts
        if kind == execution_kind::CREATE
            && status_code == EvmStatusCode::Success
            && !return_data.is_empty()
        {
            if gas_left >= GAS_CODE_DEPOSIT || depth == 0 {
                ext_post.save_code(return_data.clone());
            } else {
                gas_left = U256::from(0);
                status_code = EvmStatusCode::Failure;
            }
        }

        ExecutionResult {
            gas_left: gas_left,
            status_code: status_code.clone(),
            return_data: ReturnData::new(return_data, 0, return_data_length),
            exception: match status_code {
                EvmStatusCode::Success => String::default(),
                code => code.to_string(),
            },
        }
    }
}

#[derive(Clone)]
pub struct AVMFactory {}

impl AVMFactory {
    pub fn new() -> Self { AVMFactory {} }
}

impl Factory for AVMFactory {
    fn exec(&mut self, _params: ActionParams, _ext: &mut Ext) -> ExecutionResult {
        unimplemented!()
    }
}

#[test]
fn test_create_fastvm() { let _vm = FastVMFactory::new(); }
