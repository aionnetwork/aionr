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

use aion_types::U128;
use aion_types::U256;
use fastvm::{EvmStatusCode, FastVM};
use fastvm::basetypes::{constants::GAS_CODE_DEPOSIT, DataWord};
use fastvm::context::{execution_kind, ExecutionContext, TransactionResult};
use fastvm::vm::{Ext, ActionParams, ActionValue};
use vm_common::{ExecutionResult, ExecStatus, CallType, ReturnData};
use std::sync::Arc;
use avm::{
    AVM,
    AVMExt,
    AVMActionParams
};
use avm::types::{TransactionContext as AVMTxContext, AvmStatusCode};

pub trait Factory {
    fn exec(&mut self, params: Vec<ActionParams>, ext: &mut Ext) -> Vec<ExecutionResult>;
    fn exec_v1(&mut self, params: Vec<AVMActionParams>, ext: &mut AVMExt) -> Vec<ExecutionResult>;
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
    fn exec(&mut self, fvm_params: Vec<ActionParams>, ext: &mut Ext) -> Vec<ExecutionResult> {
        assert!(fvm_params.len() == 1);
        let params = fvm_params[0].clone();
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
            return vec![ExecutionResult {
                gas_left: params.gas,
                status_code: ExecStatus::Success,
                return_data: ReturnData::empty(),
                exception: String::default(),
            }];
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

        vec![ExecutionResult {
            gas_left: gas_left,
            status_code: status_code.into(),
            return_data: ReturnData::new(return_data, 0, return_data_length),
            exception: match status_code {
                EvmStatusCode::Success => String::default(),
                code => code.to_string(),
            },
        }]
    }
    fn exec_v1(&mut self, params: Vec<AVMActionParams>, ext: &mut AVMExt) -> Vec<ExecutionResult> {
        unimplemented!()
    }
}

const AVM_CREATE: i32 = 2;
const AVM_CALL: i32 = 3;
const AVM_BALANCE_TRANSFER: i32 = 4;

#[derive(Clone)]
pub struct AVMFactory {
    instance: AVM,
}

impl AVMFactory {
    pub fn new() -> Self {
        AVMFactory {
            instance: AVM::new(),
        }
    }
}

impl Factory for AVMFactory {
    fn exec(&mut self, fvm_params: Vec<ActionParams>, ext: &mut Ext) -> Vec<ExecutionResult> {
        unimplemented!()
    }
    fn exec_v1(&mut self, avm_params: Vec<AVMActionParams>, ext: &mut AVMExt) -> Vec<ExecutionResult> {
        let mut avm_tx_contexts = Vec::new();

        for params in avm_params {
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

            let mut call_data = params.data.unwrap_or_else(Vec::new);

            let gas_limit = params.gas.low_u64();
            let gas_price = params.gas_price.low_u64();
            let address = params.address;
            let caller = params.sender;
            let origin = params.origin;
            let transfer_value: [u8; 32] = params.value.into();
            let call_value = transfer_value.to_vec();
            debug!(target: "vm", "call_data = {:?}", call_data);
            debug!(target: "vm", "gas limit = {:?}", gas_limit);

            let block_coinbase = ext.env_info().author.clone();
            let difficulty = <[u8; 16]>::from(U128::from(ext.env_info().difficulty));
            let block_difficulty = difficulty.to_vec();
            let block_gas_limit = ext.env_info().gas_limit.low_u64();
            let block_number = ext.env_info().number;
            // don't really know why jit timestamp is int..
            let block_timestamp = ext.env_info().timestamp as i64;
            let tx_hash = vec![0; 32];
            let depth = ext.depth() as i32;
            let kind = match params.call_type {
                CallType::None => AVM_CREATE,
                _ => AVM_CALL,
            };

            if kind == AVM_CREATE {
                call_data = code.clone();
            }
            let nonce = params.nonce;

            avm_tx_contexts.push(AVMTxContext::new(
                tx_hash,
                address,
                origin,
                caller,
                gas_price,
                gas_limit,
                call_value,
                call_data,
                depth,
                kind,
                block_coinbase,
                block_number,
                block_timestamp,
                block_gas_limit,
                block_difficulty,
                nonce,
            ))
        }

        let inst = &mut self.instance;
        let ext_ptr: *mut ::libc::c_void = unsafe { ::std::mem::transmute(Box::new(ext)) };
        //println!("ext ptr = {:?}, avm contexts = {:?}", ext_ptr, avm_tx_contexts);
        let mut res = inst.execute(ext_ptr as i64, &avm_tx_contexts);

        let ext_post: &mut Box<Ext> = unsafe { ::std::mem::transmute(ext_ptr) };
        let mut exec_results = Vec::new();

        if let Ok(ref mut tx_res) = res {
            assert!(
                tx_res.len() >= 1,
                "avm must return valid transaction result"
            );

            for index in 0..tx_res.len() {
                let result = tx_res[index].clone();
                let mut status_code: AvmStatusCode = (result.code as i32).into();
                let mut gas_left =
                    U256::from(avm_tx_contexts[index].energy_limit - result.energy_used);
                let return_data = result.return_data;
                let storage_root = result.storage_root_hash;
                println!("storage root = {}", storage_root);
                exec_results.push(ExecutionResult {
                    gas_left: gas_left.into(),
                    status_code: status_code.clone().into(),
                    return_data: ReturnData::new(return_data.clone(), 0, return_data.len()),
                    exception: match status_code.into() {
                        AvmStatusCode::Success => String::default(),
                        code => code.to_string(),
                    },
                })
            }
        } else {
            panic!("avm unexpected error");
        }

        return exec_results;
    }
}

#[cfg(test)]
mod tests {
    use super::AVMFactory;
    use super::FastVMFactory;

    #[test]
    fn test_create_fastvm() { let _vm = FastVMFactory::new(); }

    #[test]
    fn test_create_avm() { let _vm = AVMFactory::new(); }
}
