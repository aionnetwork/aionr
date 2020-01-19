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

use std::{mem, slice};
use std::ops::Deref;
use std::convert::Into;
use std::clone::Clone;
use libc;
use basetypes::{EvmMessage, constants};
use ffi::{EvmResult, EvmStatusCode};
use types::CallType;
use types::traits::Ext;
use aion_types::{U128, H256, U256, Address};

#[derive(Debug)]
#[repr(C)]
pub struct EvmWord {
    bytes: [u8; 16],
}

#[derive(Debug)]
#[repr(C)]
pub struct HashValue {
    bytes: [u8; 32],
}

#[derive(Debug)]
#[repr(C)]
pub struct EvmAddress {
    bytes: [u8; 32],
}

impl Deref for EvmWord {
    type Target = [u8; 16];

    fn deref(&self) -> &Self::Target { &self.bytes }
}

impl Deref for EvmAddress {
    type Target = [u8; 32];

    fn deref(&self) -> &Self::Target { &self.bytes }
}

// definitions of callbacks used by fastvm.so; obj : &Callback
#[no_mangle]
// 1 - get block hash
pub extern fn get_blockhash(obj: *mut libc::c_void, number: u64) -> HashValue {
    debug!(target: "vm", "get_blockhash");
    let ext: &mut Box<dyn Ext> = unsafe { mem::transmute(obj) };
    debug!(target: "vm", "blockhash = {:?}, number = {:?}", ext.blockhash(&(number.into())), number);
    println!("blockhash = {:?}", ext.blockhash(&(number.into())));
    HashValue {
        bytes: ext.blockhash(&(number.into())).into(),
    }
}

#[no_mangle]
// 2 - get contract code
pub extern fn get_code(obj: *mut libc::c_void, code_info: *mut u8, address: EvmAddress) {
    debug!(target: "vm", "get_code");
    let ext: &mut Box<dyn Ext> = unsafe { mem::transmute(obj) };

    #[repr(C)]
    struct code_info {
        code_size: u32,
        code_ptr: *mut u8,
    }

    let info: &mut code_info = unsafe { mem::transmute(code_info) };
    let code = ext.extcode(&((*address).into()));
    info.code_size = code.len().clone() as u32;
    info.code_ptr = {
        if info.code_size <= 0 {
            ::std::ptr::null_mut()
        } else {
            unsafe { mem::transmute(&code.as_slice()[0]) }
        }
    };
}

#[no_mangle]
// 3 - get account balance
pub extern fn get_balance(obj: *mut libc::c_void, address: EvmAddress) -> EvmWord {
    debug!(target: "vm", "get_balance");
    let ext: &mut Box<dyn Ext> = unsafe { mem::transmute(obj) };
    let balance = ext.balance(&(address.bytes.into()));
    let evm_strg: [u8; 16] = U128::from(U256::from(balance)).into();
    EvmWord {
        bytes: evm_strg,
    }
}

#[no_mangle]
// 4 - check account exists
pub extern fn exists(obj: *mut libc::c_void, address: EvmAddress) -> i32 {
    debug!(target: "vm", "check exists");
    let ext: &mut Box<dyn Ext> = unsafe { mem::transmute(obj) };
    match ext.exists(&((*address).into())) {
        true => 1,
        false => 0,
    }
}

#[no_mangle]
// 5 - get storage
pub extern fn get_storage(obj: *mut libc::c_void, _address: EvmAddress, key: EvmWord) -> EvmWord {
    let ext: &mut Box<dyn Ext> = unsafe { mem::transmute(obj) };
    debug!(target: "vm", "ext<get_storage>: key = {:?}, raw_env = {:?}", key, obj);
    let storage = ext.storage_at(&(key.bytes).into());
    debug!(target: "vm",
        "FastVM CB: get_storage() key = {:?}, value = {:?}",
        key,
        storage
    );
    let evm_strg: [u8; 16] = storage.into();
    debug!(target: "vm", "callback.rs get_storage() storage: {:?}", evm_strg);
    EvmWord {
        bytes: evm_strg,
    }
}

#[no_mangle]
// 6 - put storage
pub extern fn put_storage(obj: *mut libc::c_void, _addr: EvmAddress, key: EvmWord, value: EvmWord) {
    let ext: &mut Box<dyn Ext> = unsafe { mem::transmute(obj) };

    debug!(target: "vm",
        "callback.rs put_storage() key: {:?}",
        U256::from(U128::from(key.bytes.clone()))
    );
    debug!(target: "vm", "callback.rs put_storage() value: {:?}", value);
    ext.set_storage(U128::from(*key).into(), U128::from(*value).into());
}

#[no_mangle]
// 7 - self destroy
pub extern fn selfdestruct(obj: *mut libc::c_void, _owner: EvmAddress, beneficiary: EvmAddress) {
    debug!(target: "vm", "selfdestruct");
    let ext: &mut Box<dyn Ext> = unsafe { mem::transmute(obj) };
    ext.suicide(&((*beneficiary).into()));
}

#[no_mangle]
// 8 - log
pub extern fn vm_log(
    obj: *mut libc::c_void,
    _addr: EvmAddress,
    data: *const u8,
    data_size: usize,
    topics: *const u8,
    topics_cnt: usize,
)
{
    debug!(target: "vm", "log");
    let ext: &mut Box<dyn Ext> = unsafe { mem::transmute(obj) };
    let data: &[u8] = unsafe { slice::from_raw_parts(data, data_size) };
    let mut new_topics: Vec<H256> = Vec::new();
    // topics count is calculated by 16 bytes in evm
    for idx in 0..topics_cnt / 2 {
        let topic: &[u8; 32] = unsafe { mem::transmute(topics as usize + idx * 32) };
        new_topics.push((*topic).into());
    }
    debug!(target: "vm", "vm topics = {:?}, data = {:?}", new_topics, data);
    ext.log(new_topics, data);
}

#[no_mangle]
// 9 - call
pub extern fn call(obj: *mut libc::c_void, info: *mut u8, msg: *const u8) -> *const u8 {
    debug!(target: "vm", "enter vm call");
    let ext: &mut Box<dyn Ext> = unsafe { mem::transmute(obj) };
    let result_info: &mut EvmResult = unsafe { mem::transmute(info) };
    let evm_msg: &EvmMessage = unsafe { mem::transmute(msg) };
    let call_data_size = evm_msg.input_size.clone();
    let data: &[u8] = unsafe { slice::from_raw_parts(evm_msg.input as *const u8, call_data_size) };
    let call_type: CallType = match evm_msg.kind {
        0 => CallType::Call,
        1 => CallType::DelegateCall,
        2 => CallType::CallCode,
        3 => CallType::None,
        4 => CallType::StaticCall,
        _ => panic!("Call type does not exist"),
    };

    let static_flag = evm_msg.flags == 1 || evm_msg.kind == 4;

    // Address in different call types are handled in VM
    let sender_address: Address = U256::from(&evm_msg.caller).into();
    let receive_address: Address = U256::from(&evm_msg.recv_addr).into();
    let code_address: Address = U256::from(&evm_msg.address).into();

    debug!(target: "vm", "sender address = {:?}", evm_msg.caller);
    debug!(target: "vm", "receive address = {:?}", evm_msg.recv_addr);
    debug!(target: "vm", "code address = {:?}", evm_msg.address);

    // Failure if exceed maximum call depth
    if evm_msg.depth >= constants::MAX_CALL_DEPTH {
        result_info.status_code = EvmStatusCode::Failure;
        result_info.gas_left = 0;
        return ::std::ptr::null();
    }

    let result = match call_type {
        CallType::None => {
            ext.create(
                &(evm_msg.gas.into()),
                &evm_msg.value.to_vec().as_slice().into(),
                data,
            )
        }
        _ => {
            ext.call(
                &(evm_msg.gas.into()),
                &sender_address,
                &receive_address,
                Some(evm_msg.value.to_vec().as_slice().into()),
                data,
                &code_address,
                call_type,
                static_flag,
            )
        }
    };

    result_info.gas_left = result.gas_left.low_u64() as i64;
    result_info.status_code = result.status_code.into();
    result_info.output_size = result.return_data.len();
    debug!(target: "vm", "output_data: {:?}", result.return_data);

    result_info.output_data = unsafe { vm_alloc_data(result_info.output_size as i32) };
    for idx in 0..result_info.output_size {
        unsafe {
            *result_info.output_data.add(idx) = result.return_data[idx];
        }
    }
    result_info.output_data
}

#[no_mangle]
// 10 - get tx context
pub extern fn get_tx_context(_obj: *mut libc::c_void, _result: *mut u8) {
    debug!(target: "vm", "I'm get_tx_context foo");
}

#[no_mangle]
pub extern fn test_fn() {
    info!(target: "vm", "I'm the callback test foo");
}

#[link(name = "fastvm")]
extern {
    pub fn vm_alloc_data(size: i32) -> *mut u8;
    // below two are reserved for `cargo rum --example callback`
    pub fn register_callback(func: extern fn());
    // use single resiter func, since each func type differs
    // it is ugly!!!
    pub fn register_call_fn(
        func: extern fn(obj: *mut libc::c_void, result: *mut u8, msg: *const u8) -> *const u8,
    );
    pub fn register_log_fn(
        func: extern fn(
            obj: *mut libc::c_void,
            address: EvmAddress,
            data: *const u8,
            data_size: usize,
            topics: *const u8,
            topics_cnt: usize,
        ),
    );
    pub fn register_get_code_fn(
        func: extern fn(obj: *mut libc::c_void, code_info: *mut u8, address: EvmAddress),
    );
    pub fn register_get_storage_fn(
        func: extern fn(obj: *mut libc::c_void, address: EvmAddress, key: EvmWord) -> EvmWord,
    );
    pub fn register_put_storage_fn(
        func: extern fn(obj: *mut libc::c_void, address: EvmAddress, key: EvmWord, value: EvmWord),
    );
    pub fn register_exists_fn(func: extern fn(obj: *mut libc::c_void, address: EvmAddress) -> i32);
    pub fn register_get_balance_fn(
        func: extern fn(obj: *mut libc::c_void, address: EvmAddress) -> EvmWord,
    );
    pub fn register_selfdestruct_fn(
        func: extern fn(obj: *mut libc::c_void, address: EvmAddress, beneficiary: EvmAddress),
    );
    pub fn register_get_tx_context_fn(func: extern fn(obj: *mut libc::c_void, result: *mut u8));
    pub fn register_get_blockhash_fn(
        func: extern fn(obj: *mut libc::c_void, number: u64) -> HashValue,
    );
}

pub fn register_cbs() {
    // register call_fn
    debug!(target: "vm", "register evm callbacks");
    unsafe {
        register_callback(test_fn);
        register_exists_fn(exists);
        register_get_storage_fn(get_storage);
        register_put_storage_fn(put_storage);
        register_get_balance_fn(get_balance);
        register_get_code_fn(get_code);
        register_selfdestruct_fn(selfdestruct);
        register_call_fn(call);
        register_get_tx_context_fn(get_tx_context);
        register_get_blockhash_fn(get_blockhash);
        register_log_fn(vm_log);
    };
}
