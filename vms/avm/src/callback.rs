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
use std::{mem, ptr};
use std::convert::Into;
use core::fmt;
use core::slice;
use libc::c_void;
use num_bigint::BigUint;
use types::traits::Ext;
use types::avm::NativeDecoder;
use aion_types::{Address, U256, H256};
use hash::blake2b;
use crypto::{sha2::Sha256, digest::Digest, ed25519};
use tiny_keccak::keccak256;

const AVM_VERSION_MAGIC: &[u8] = b"avm-version-";
const AVM_V1: u8 = 1;

#[derive(Debug)]
#[repr(C)]
pub struct avm_address {
    pub bytes: [u8; 32],
}

#[derive(Debug)]
#[repr(C)]
pub struct avm_value {
    pub bytes: [u8; 32],
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct avm_bytes {
    pub length: u32,
    pub pointer: *mut u8,
}

impl Into<Vec<u8>> for avm_bytes {
    fn into(self) -> Vec<u8> {
        let ret = unsafe { slice::from_raw_parts(self.pointer, self.length as usize) };
        ret.into()
    }
}

#[repr(C)]
pub struct avm_callbacks {
    pub create_account: extern fn(handle: *const c_void, address: *const avm_address),
    pub has_account_state: extern fn(handle: *const c_void, address: *const avm_address) -> u32,
    pub put_code:
        extern fn(handle: *const c_void, address: *const avm_address, code: *const avm_bytes),
    pub get_code: extern fn(handle: *const c_void, address: *const avm_address) -> avm_bytes,
    pub put_storage: extern fn(
        handle: *const c_void,
        address: *const avm_address,
        key: *const avm_bytes,
        value: *const avm_bytes,
    ),
    pub get_storage:
        extern fn(handle: *const c_void, address: *const avm_address, key: *const avm_bytes)
            -> avm_bytes,
    pub delete_account: extern fn(handle: *const c_void, address: *const avm_address),
    pub get_balance: extern fn(handle: *const c_void, address: *const avm_address) -> avm_value,
    pub increase_balance:
        extern fn(handle: *const c_void, address: *const avm_address, value: *const avm_value),
    pub decrease_balance:
        extern fn(handle: *const c_void, address: *const avm_address, value: *const avm_value),
    pub get_nonce: extern fn(handle: *const c_void, address: *const avm_address) -> u64,
    pub increment_nonce: extern fn(handle: *const c_void, address: *const avm_address),
    pub touch_account: extern fn(handle: *const c_void, address: *const avm_address, idx: i32),
    pub send_signal: extern fn(handle: *const c_void, sig_num: i32) -> avm_bytes,
    pub contract_address:
        extern fn(sender: *const avm_address, nonce: *const avm_bytes) -> avm_bytes,
    pub add_log: extern fn(handle: *const c_void, logs: *const avm_bytes, idx: i32),
    pub get_transformed_code:
        extern fn(handle: *const c_void, addr: *const avm_address, version: u8) -> avm_bytes,
    pub put_transformed_code: extern fn(
        handle: *const c_void,
        addr: *const avm_address,
        code: *const avm_bytes,
        version: u8,
    ),
    pub get_objectgraph: extern fn(handle: *const c_void, addr: *const avm_address) -> avm_bytes,
    pub set_objectgraph:
        extern fn(handle: *const c_void, addr: *const avm_address, data: *const avm_bytes),
    pub get_blockhash: extern fn(handle: *const c_void, block_number: i64) -> avm_bytes,
    pub sha256: extern fn(input: *const avm_bytes) -> avm_bytes,
    pub blake2b: extern fn(input: *const avm_bytes) -> avm_bytes,
    pub keccak256: extern fn(input: *const avm_bytes) -> avm_bytes,
    pub verify_ed25519:
        extern fn(input: *const avm_bytes, input1: *const avm_bytes, input2: *const avm_bytes)
            -> bool,
    pub remove_storage:
        extern fn(handle: *const c_void, addr: *const avm_address, data: *const avm_bytes),
    pub has_storage: extern fn(handle: *const c_void, addr: *const avm_address) -> bool,
}

impl fmt::Display for avm_address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = ::rustc_hex::ToHex::to_hex(&self.bytes[..]);
        write!(f, "0x{}", s)
    }
}

impl fmt::Display for avm_value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = BigUint::from_bytes_be(&self.bytes);
        write!(f, "{}", s)
    }
}

impl fmt::Display for avm_bytes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = unsafe { slice::from_raw_parts(self.pointer as *mut u8, self.length as usize) };
        let s = ::rustc_hex::ToHex::to_hex(bytes);
        write!(f, "0x{}", s)
    }
}

#[link(name = "avmloader")]
extern {
    pub static mut callbacks: avm_callbacks;

    #[allow(dead_code)]
    pub fn is_null(bytes: *const avm_bytes) -> bool;

    #[allow(dead_code)]
    pub fn new_fixed_bytes(length: u32) -> avm_bytes;

    #[allow(dead_code)]
    pub fn new_null_bytes() -> avm_bytes;

    #[allow(dead_code)]
    pub fn release_bytes(bytes: *mut avm_bytes);
}

#[no_mangle]
pub extern fn avm_create_account(handle: *const c_void, address: *const avm_address) {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    debug!(target: "vm", "create new AVM account");
    let addr: &Address = unsafe { mem::transmute(address) };
    debug!(target: "vm", "create new AVM account with address: {:?}", addr);
    ext.create_account(addr);
}

#[no_mangle]
pub extern fn avm_has_account_state(handle: *const c_void, address: *const avm_address) -> u32 {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };
    ext.exists(addr) as u32
}

#[no_mangle]
pub extern fn avm_put_code(
    handle: *const c_void,
    address: *const avm_address,
    code: *const avm_bytes,
)
{
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };
    let code: &avm_bytes = unsafe { mem::transmute(code) };
    debug!(target: "vm", "avm_put_code at: {:?}", addr);
    let ext_code: &[u8] =
        unsafe { ::std::slice::from_raw_parts(code.pointer, code.length as usize) };
    ext.save_code_at(addr, ext_code.to_vec());
}

#[no_mangle]
pub extern fn avm_get_code(handle: *const c_void, address: *const avm_address) -> avm_bytes {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };

    debug!(target: "vm", "avm_get_code: 0x{:?}", addr);

    match ext.code(addr) {
        None => unsafe { new_null_bytes() },
        Some(code) => {
            if code.len() == 0 {
                unsafe { new_null_bytes() }
            } else {
                unsafe {
                    let ret = new_fixed_bytes(code.len() as u32);
                    ptr::copy(&code.as_slice()[0], ret.pointer, code.len());
                    ret
                }
            }
        }
    }
}

#[no_mangle]
pub extern fn avm_put_storage(
    handle: *const c_void,
    address: *const avm_address,
    key: *const avm_bytes,
    value: *const avm_bytes,
)
{
    //raw debug info
    debug!(target: "vm", "handler = {:?}, address ptr: {:?}; origin key = {:?}, origin value = {:?}",
        handle,
        address,
        unsafe {(*key)},
        unsafe {(*value)}
    );
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };

    let key: &[u8] = unsafe { slice::from_raw_parts((*key).pointer, (*key).length as usize) };
    let value: &[u8] = unsafe { slice::from_raw_parts((*value).pointer, (*value).length as usize) };

    debug!(target: "vm", "avm_put_storage: addr = 0x{:?}, key = {:?}, value = {:?}", addr, key, value);

    ext.sstore(addr, key.into(), value.into());
}

#[no_mangle]
pub extern fn avm_get_storage(
    handle: *const c_void,
    address: *const avm_address,
    key: *const avm_bytes,
) -> avm_bytes
{
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr = unsafe { &(*address).bytes.into() };
    let key: &[u8] = unsafe { slice::from_raw_parts((*key).pointer, (*key).length as usize) };

    debug!(target: "vm", "avm_get_storage: addr = 0x{:?}, key = {:?}", addr, key);

    match ext.sload(addr, &key.into()) {
        Some(v) => {
            if v.len() > 0 {
                debug!("storage value = {:?}", v);
                unsafe {
                    let mut ret = new_fixed_bytes(v.len() as u32);
                    ptr::copy(&v.as_slice()[0], ret.pointer, v.len());
                    ret
                }
            } else {
                // is there a way to get zero-length array's header pointer
                unsafe { new_fixed_bytes(0) }
                // unsafe {new_null_bytes()}
            }
        }
        None => {
            debug!(target: "vm", "value is None");
            unsafe { new_null_bytes() }
        }
    }
}

#[no_mangle]
pub extern fn avm_delete_account(handle: *const c_void, address: *const avm_address) {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };

    debug!(target: "vm", "avm_selfdestruct: {:?}", addr);

    ext.kill_account(addr);
}

#[no_mangle]
pub extern fn avm_get_balance(handle: *const c_void, address: *const avm_address) -> avm_value {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };

    let balance = avm_value {
        bytes: ext.balance(addr).into(),
    };

    debug!(target: "vm", "avm_get_balance: 0x{:?} = {:?}", addr, balance);

    balance
}

#[no_mangle]
pub extern fn avm_increase_balance(
    handle: *const c_void,
    address: *const avm_address,
    value: *const avm_value,
)
{
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };
    let value: &avm_value = unsafe {
        {
            mem::transmute(value)
        }
    };

    debug!(target: "vm", "avm_inc_balance: 0x{:?} += {:?}", addr, value);

    ext.inc_balance(addr, &value.bytes.into());
}

#[no_mangle]
pub extern fn avm_decrease_balance(
    handle: *const c_void,
    address: *const avm_address,
    value: *const avm_value,
)
{
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };
    let value: &avm_value = unsafe {
        {
            mem::transmute(value)
        }
    };

    debug!(target: "vm", "avm_inc_balance: 0x{:?} -= {:?}", addr, value);

    ext.dec_balance(addr, &value.bytes.into());
}

#[no_mangle]
pub extern fn avm_get_nonce(handle: *const c_void, address: *const avm_address) -> u64 {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };
    let nonce = ext.nonce(addr);

    debug!(target: "vm", "avm_get_nonce: 0x{:?} = {:?}", addr, nonce);
    return nonce;
}

#[no_mangle]
pub extern fn avm_increment_nonce(handle: *const c_void, address: *const avm_address) {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };

    debug!(target: "vm", "avm_inc_nonce: 0x{:?}", addr);

    ext.inc_nonce(addr);
}

#[no_mangle]
pub extern fn avm_touch_account(handle: *const c_void, address: *const avm_address, index: i32) {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };

    debug!(target: "vm", "touch account: {:?} - {:?}", addr, index);

    ext.touch_account(addr, index);
}

#[no_mangle]
pub extern fn avm_send_signal(handle: *const c_void, sig_num: i32) -> avm_bytes {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    ext.send_signal(sig_num);
    match sig_num {
        0 => {
            ext.commit();
            let root = ext.root();
            debug!(target: "vm", "state root = {:?}", root);
            unsafe {
                let ret = new_fixed_bytes(32);
                ptr::copy(&root[0], ret.pointer, 32);
                ret
            }
        }

        1 => {
            // just get current root
            let root = ext.root();
            unsafe {
                let ret = new_fixed_bytes(32);
                ptr::copy(&root[0], ret.pointer, 32);
                ret
            }
        }
        _ => unsafe {
            let ret = new_fixed_bytes(32);
            ret
        },
    }
}

fn contract_address(sender: &Address, nonce: &U256) -> (Address, Option<H256>) {
    use rlp::RlpStream;
    let mut stream = RlpStream::new_list(2);
    stream.append(sender);
    stream.append(nonce);
    let origin: [u8; 32] = blake2b(stream.as_raw()).into();
    let mut buffer = [0xa0u8; 32];
    &mut buffer[1..].copy_from_slice(&origin[1..]);
    (buffer.into(), None)
}

#[no_mangle]
pub extern fn avm_contract_address(
    sender: *const avm_address,
    nonce: *const avm_bytes,
) -> avm_bytes
{
    let addr: &Address = unsafe { mem::transmute(sender) };
    let n = unsafe { slice::from_raw_parts((*nonce).pointer, (*nonce).length as usize) };

    debug!(target: "vm", "avm new contract: sender = {:?}, nonce = {:?}", addr, n);

    let (new_contract, _) = contract_address(addr, &n.into());

    unsafe {
        let ret = new_fixed_bytes(32);
        ptr::copy(&new_contract[0], ret.pointer, 32);
        ret
    }
}

#[no_mangle]
pub extern fn avm_add_log(handle: *const c_void, avm_log: *const avm_bytes, index: i32) {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let log_data = unsafe { slice::from_raw_parts((*avm_log).pointer, (*avm_log).length as usize) };
    let mut decoder = NativeDecoder::new(&log_data.to_vec());
    let address: Address = decoder.decode_bytes().unwrap()[..].into();
    let topic_num = decoder.decode_int().unwrap();
    let mut topics: Vec<H256> = Vec::new();
    for _ in 0..topic_num {
        topics.push(decoder.decode_bytes().unwrap().as_slice().into());
    }

    let data = decoder.decode_bytes().unwrap();

    ext.avm_log(&address, topics, data, index);
}

#[no_mangle]
pub extern fn avm_get_transformed_code(
    handle: *const c_void,
    address: *const avm_address,
    version: u8,
) -> avm_bytes
{
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };

    debug!(target: "vm", "avm_get_transformed_code: 0x{:?}, v{:?}", addr, version);

    match ext.get_transformed_code(addr) {
        None => unsafe { new_null_bytes() },
        Some(code) => {
            if code.len() == 0 {
                unsafe { new_null_bytes() }
            } else {
                match (version, code.as_slice().starts_with(AVM_VERSION_MAGIC)) {
                    (expected_version, true) => {
                        if code[AVM_VERSION_MAGIC.len()] != expected_version {
                            unsafe { new_null_bytes() }
                        } else {
                            unsafe {
                                let ret = new_fixed_bytes(
                                    (code.len() - AVM_VERSION_MAGIC.len() - 1) as u32,
                                );
                                ptr::copy(
                                    &code.as_slice()[AVM_VERSION_MAGIC.len() + 1],
                                    ret.pointer,
                                    code.len(),
                                );
                                ret
                            }
                        }
                    }
                    (AVM_V1, false) => unsafe {
                        let ret = new_fixed_bytes(code.len() as u32);
                        ptr::copy(&code.as_slice()[0], ret.pointer, code.len());
                        ret
                    },
                    (_, false) => unsafe { new_null_bytes() },
                }
            }
        }
    }
}

#[no_mangle]
pub extern fn avm_put_transformed_code(
    handle: *const c_void,
    address: *const avm_address,
    code: *const avm_bytes,
    version: u8,
)
{
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };
    let code: &avm_bytes = unsafe { mem::transmute(code) };
    debug!(target: "vm", "avm_put_transformed_code at: {:?}, v{:?}", addr, version);
    let ext_code: &[u8] =
        unsafe { ::std::slice::from_raw_parts(code.pointer, code.length as usize) };
    let mut transformed_code = Vec::new();
    transformed_code.extend_from_slice(AVM_VERSION_MAGIC);
    transformed_code.push(version);
    transformed_code.extend_from_slice(ext_code);
    ext.save_transformed_code(addr, ext_code.to_vec());
}

#[no_mangle]
pub extern fn avm_get_objectgraph(handle: *const c_void, address: *const avm_address) -> avm_bytes {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };

    debug!(target: "vm", "avm_get_objectgraph: 0x{:?}", addr);

    match ext.get_objectgraph(addr) {
        None => unsafe { new_null_bytes() },
        Some(graph) => {
            if graph.len() == 0 {
                unsafe { new_null_bytes() }
            } else {
                unsafe {
                    let ret = new_fixed_bytes(graph.len() as u32);
                    ptr::copy(&graph.as_slice()[0], ret.pointer, graph.len());
                    ret
                }
            }
        }
    }
}

#[no_mangle]
pub extern fn avm_set_objectgraph(
    handle: *const c_void,
    address: *const avm_address,
    data: *const avm_bytes,
)
{
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };
    let graph: &avm_bytes = unsafe { mem::transmute(data) };
    debug!(target: "vm", "avm_set_objectgraph at: {:?}", addr);
    let ext_graph: &[u8] =
        unsafe { ::std::slice::from_raw_parts(graph.pointer, graph.length as usize) };
    ext.set_objectgraph(addr, ext_graph.to_vec());
}

#[no_mangle]
pub extern fn avm_get_blockhash(handle: *const c_void, block_number: i64) -> avm_bytes {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let blockhash = ext.blockhash(&block_number.into());
    unsafe {
        let ret = new_fixed_bytes(32);
        ptr::copy(&blockhash[..][0], ret.pointer, 32);
        ret
    }
}

#[no_mangle]
pub extern fn avm_sha256(input: *const avm_bytes) -> avm_bytes {
    let data = unsafe { slice::from_raw_parts((*input).pointer, (*input).length as usize) };
    let mut sh = Sha256::new();
    sh.input(data);
    let mut ans = vec![0u8; 32];
    sh.result(&mut ans);
    unsafe {
        let ret = new_fixed_bytes(32);
        ptr::copy(&ans.as_slice()[0], ret.pointer, ans.len());
        ret
    }
}

#[no_mangle]
pub extern fn avm_blake2b(input: *const avm_bytes) -> avm_bytes {
    let data: Vec<u8> = unsafe { (*input).into() };
    let ans = blake2b(data);
    unsafe {
        let ret = new_fixed_bytes(32);
        ptr::copy(&ans[0], ret.pointer, ans.len());
        ret
    }
}

#[cfg(test)]
pub fn avm_blake2b_test(input: *const avm_bytes) -> Vec<u8> {
    unsafe {
        println!("input = {:?}", *input);
    }
    let data: Vec<u8> = unsafe { (*input).into() };
    blake2b(data)[..].into()
}

#[no_mangle]
pub extern fn avm_keccak256(input: *const avm_bytes) -> avm_bytes {
    let data: Vec<u8> = unsafe { (*input).into() };
    let ans = keccak256(&data);
    unsafe {
        let ret = new_fixed_bytes(32);
        ptr::copy(&ans[0], ret.pointer, ans.len());
        ret
    }
}

#[no_mangle]
pub extern fn avm_vediryEdDSA(
    input: *const avm_bytes,
    input1: *const avm_bytes,
    input2: *const avm_bytes,
) -> bool
{
    let data: Vec<u8> = unsafe { (*input).into() };
    let data1: Vec<u8> = unsafe { (*input1).into() };
    let data2: Vec<u8> = unsafe { (*input2).into() };
    return ed25519::verify(&data, &data1, &data2);
}

#[no_mangle]
pub extern fn avm_remove_storage(
    handle: *const c_void,
    address: *const avm_address,
    data: *const avm_bytes,
)
{
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };
    let key: Vec<u8> = unsafe { (*data).into() };
    debug!(target: "vm", "avm remove storage at: {:?}", key);
    ext.remove_storage(addr, key);
}

#[no_mangle]
pub extern fn avm_has_storage(handle: *const c_void, address: *const avm_address) -> bool {
    let ext: &mut Box<Ext> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };
    debug!(target: "vm", "avm has storage at address: {:?}", addr);
    ext.has_storage(addr)
}

pub fn register_callbacks() {
    unsafe {
        callbacks.create_account = avm_create_account;
        callbacks.has_account_state = avm_has_account_state;
        callbacks.get_balance = avm_get_balance;
        callbacks.put_code = avm_put_code;
        callbacks.get_code = avm_get_code;
        callbacks.put_storage = avm_put_storage;
        callbacks.get_storage = avm_get_storage;
        callbacks.delete_account = avm_delete_account;
        callbacks.increase_balance = avm_increase_balance;
        callbacks.decrease_balance = avm_decrease_balance;
        callbacks.get_nonce = avm_get_nonce;
        callbacks.increment_nonce = avm_increment_nonce;
        callbacks.touch_account = avm_touch_account;
        callbacks.send_signal = avm_send_signal;
        callbacks.contract_address = avm_contract_address;
        callbacks.add_log = avm_add_log;
        callbacks.get_transformed_code = avm_get_transformed_code;
        callbacks.put_transformed_code = avm_put_transformed_code;
        callbacks.get_objectgraph = avm_get_objectgraph;
        callbacks.set_objectgraph = avm_set_objectgraph;
        callbacks.get_blockhash = avm_get_blockhash;
        callbacks.sha256 = avm_sha256;
        callbacks.blake2b = avm_blake2b;
        callbacks.keccak256 = avm_keccak256;
        callbacks.verify_ed25519 = avm_vediryEdDSA;
        callbacks.remove_storage = avm_remove_storage;
        callbacks.has_storage = avm_has_storage;
    }
}

#[cfg(test)]
mod tests {
    // use rand;
    // use std::ptr;
    // use hash::BLAKE2B_EMPTY;

    use super::*;

    #[test]
    fn set_storage() {
        //NOTE: moved to executive
        //debug key/value
        //     let mut key: Vec<u8> = (1..10).map(|_| rand::random()).collect();
        //     let mut value = vec![5, 6, 7, 8];

        //     println!(
        //         "address = {:?}, key = {:?}, value = {:?}",
        //         BLAKE2B_EMPTY, key, value
        //     );

        //     let handle: *mut libc::c_void = ptr::null_mut();
        //     let address = avm_address {
        //         bytes: BLAKE2B_EMPTY.into(),
        //     };
        //     let key = avm_bytes {
        //         length: key.len() as u32,
        //         pointer: &mut key.as_mut_slice()[0],
        //     };
        //     let value = avm_bytes {
        //         length: value.len() as u32,
        //         pointer: &mut value.as_mut_slice()[0],
        //     };
        //     avm_put_storage(handle, &address, &key, &value);
    }

    use std::mem;

    #[test]
    fn test_sha256() {
        let data = [1u8; 10];
        let input: *mut u8 = unsafe { mem::transmute(&data[0]) };
        let bytes = avm_bytes {
            length: data.len() as u32,
            pointer: input,
        };
        let avm_data: *const avm_bytes = unsafe { mem::transmute(&bytes) };
        let output: Vec<u8> = avm_sha256(avm_data).into();
        assert_eq!(
            output,
            vec![
                255, 173, 248, 216, 157, 55, 179, 181, 95, 225, 132, 123, 81, 60, 249, 46, 59, 232,
                126, 76, 22, 135, 8, 199, 133, 24, 69, 223, 150, 251, 54, 190
            ]
        );
    }

    #[test]
    fn test_blake2b() {
        let data = [1u8; 10];
        let input: *mut u8 = unsafe { mem::transmute(&data[0]) };
        let bytes = avm_bytes {
            length: data.len() as u32,
            pointer: input,
        };
        let avm_data: *const avm_bytes = unsafe { mem::transmute(&bytes) };
        // unsafe {println!("raw = {:?}-{:?}", *avm_data, data.len());}
        let output: Vec<u8> = avm_blake2b(avm_data).into();
        assert_eq!(
            output,
            vec![
                197, 207, 121, 92, 239, 143, 82, 117, 31, 217, 228, 129, 246, 136, 196, 67, 212,
                134, 89, 233, 238, 217, 168, 82, 23, 144, 41, 19, 132, 88, 201, 90
            ]
        );
    }

    #[test]
    fn test_keccak256() {
        let data = [1u8; 10];
        let input: *mut u8 = unsafe { mem::transmute(&data[0]) };
        let bytes = avm_bytes {
            length: data.len() as u32,
            pointer: input,
        };
        let avm_data: *const avm_bytes = unsafe { mem::transmute(&bytes) };
        let output: Vec<u8> = avm_keccak256(avm_data).into();
        assert_eq!(
            output,
            vec![
                227, 244, 47, 121, 192, 107, 198, 141, 238, 101, 169, 101, 242, 107, 156, 26, 29,
                64, 211, 25, 95, 36, 52, 17, 39, 21, 15, 114, 66, 151, 151, 9
            ]
        );
    }
}
