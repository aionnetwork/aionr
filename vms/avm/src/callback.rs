extern crate rustc_hex;
extern crate libc;
extern crate num_bigint;

use core::fmt;
use core::slice;
use libc::c_void;
use std::{mem, ptr};
use num_bigint::BigUint;
use avm::AVMExt;
use aion_types::Address;

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
        unsafe {
            Vec::from_raw_parts(self.pointer, self.length as usize, self.length as usize)
        }
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

#[link(name = "avmjni")]
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
    let ext: &mut Box<AVMExt> = unsafe { mem::transmute(handle) };
}

#[no_mangle]
pub extern fn avm_has_account_state(handle: *const c_void, address: *const avm_address) -> u32 {
    let ext: &mut Box<AVMExt> = unsafe {mem::transmute(handle)};
    let addr: &Address = unsafe { mem::transmute(address) };
    ext.account_exists(addr) as u32
}

#[no_mangle]
pub extern fn avm_put_code(
    handle: *const c_void,
    address: *const avm_address,
    code: *const avm_bytes,
)
{
    let ext: &mut Box<AVMExt> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };
    let code: &avm_bytes = unsafe { mem::transmute(code) };
    println!("avm_put_code, ext ptr = {:?}", handle);
    let ext_code: &[u8] =
        unsafe { ::std::slice::from_raw_parts(code.pointer, code.length as usize) };
    println!("code = {:?}", ext_code);
    ext.save_code(addr, ext_code.to_vec());
}

#[no_mangle]
pub extern fn avm_get_code(handle: *const c_void, address: *const avm_address) -> avm_bytes {
    let ext: &mut Box<AVMExt> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };

    println!("avm_get_code: 0x{:?}", addr);

    match ext.get_code(addr) {
        None => {
            unsafe {new_null_bytes()}
        }
        Some(code) => {
            if code.len() == 0 {
                unsafe {new_null_bytes()}
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
    println!("handler = {:?}, address ptr: {:?}; origin key = {:?}, origin value = {:?}",
        handle,
        address,
        unsafe {(*key)},
        unsafe {(*value)}
    );
    let ext: &mut Box<AVMExt> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };

    let key: &[u8] = unsafe {slice::from_raw_parts((*key).pointer, (*key).length as usize)};
    let value: &[u8] = unsafe {slice::from_raw_parts((*value).pointer, (*value).length as usize)};

    println!("avm_put_storage: addr = 0x{:?}, key = {:?}, value = {:?}", addr, key, value);

    ext.sstore(addr, key.into(), value.into());
}

#[no_mangle]
pub extern fn avm_get_storage(
    handle: *const c_void,
    address: *const avm_address,
    key: *const avm_bytes,
) -> avm_bytes
{
    let ext: &mut Box<AVMExt> = unsafe { mem::transmute(handle) };
    let addr = unsafe {&(*address).bytes.into()};
    let key: &[u8] = unsafe {slice::from_raw_parts((*key).pointer, (*key).length as usize)};

    println!("avm_get_storage: addr = 0x{:?}, key = {:?}", addr, key);

    match ext.sload(addr, &key.into()) {
        Some(v) => {
           if v.len() > 0 {
               println!("storage value = {:?}", v);
               unsafe {
                   let mut ret = new_fixed_bytes(v.len() as u32);
                   ptr::copy(&v.as_slice()[0], ret.pointer, v.len());
                   ret
                }
           } else {
               unsafe {new_null_bytes()}
           }
        },
        None => {
            println!("value is None");
            unsafe {new_null_bytes()}
        }
    }
}

#[no_mangle]
pub extern fn avm_delete_account(handle: *const c_void, address: *const avm_address) {
    let ext: &mut Box<AVMExt> = unsafe {mem::transmute(handle)};
    let addr: &Address = unsafe {mem::transmute(address)};

    ext.remove_account(addr);
}

#[no_mangle]
pub extern fn avm_get_balance(handle: *const c_void, address: *const avm_address) -> avm_value {
    let ext: &mut Box<AVMExt> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe {mem::transmute(address)};

    let balance = avm_value {
        bytes: ext.avm_balance(addr).into(),
    };

     println!("avm_get_balance: 0x{:?} = {:?}", addr, balance);

     balance
}

#[no_mangle]
pub extern fn avm_increase_balance(
    handle: *const c_void,
    address: *const avm_address,
    value: *const avm_value,
)
{
    let ext: &mut Box<AVMExt> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe {mem::transmute(address)};
    let value: &avm_value = unsafe {{mem::transmute(value)}};

    println!("avm_inc_balance: 0x{:?} += {:?}", addr, value);

    ext.inc_balance(addr, &value.bytes.into());
}

#[no_mangle]
pub extern fn avm_decrease_balance(
    handle: *const c_void,
    address: *const avm_address,
    value: *const avm_value,
)
{
    let ext: &mut Box<AVMExt> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe {mem::transmute(address)};
    let value: &avm_value = unsafe {{mem::transmute(value)}};

    println!("avm_inc_balance: 0x{:?} -= {:?}", addr, value);

    ext.dec_balance(addr, &value.bytes.into());
}

#[no_mangle]
pub extern fn avm_get_nonce(handle: *const c_void, address: *const avm_address) -> u64 {
    let ext: &mut Box<AVMExt> = unsafe {mem::transmute(handle)};
    let addr: &Address = unsafe {mem::transmute(address)};
    let nonce = ext.get_nonce(addr);

    println!("avm_get_nonce: 0x{:?} = {:?}", addr, nonce);
    return nonce;
}

#[no_mangle]
pub extern fn avm_increment_nonce(handle: *const c_void, address: *const avm_address) {
    let ext: &mut Box<AVMExt> = unsafe {mem::transmute(handle)};
    let addr: &Address = unsafe {mem::transmute(address)};

    println!("avm_inc_nonce: 0x{:?}", addr);

    ext.inc_nonce(addr);
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
    }
}

#[cfg(test)]
mod tests {
    use rand;
    use std::ptr;
    use hash::BLAKE2B_EMPTY;

    use super::*;

    #[test]
    fn test_set_storage() {
        //debug key/value
        let mut key: Vec<u8> = (1..10).map(|_| {
            rand::random()
        }).collect();
        let mut value = vec![5,6,7,8];

        println!("address = {:?}, key = {:?}, value = {:?}", BLAKE2B_EMPTY, key, value);

        let handle: *mut libc::c_void = ptr::null_mut();
        let address = avm_address {
            bytes: BLAKE2B_EMPTY.into(),
        };
        let key = avm_bytes {
            length: key.len() as u32,
            pointer: &mut key.as_mut_slice()[0],
        };
        let value = avm_bytes {
            length: value.len() as u32,
            pointer: &mut value.as_mut_slice()[0],
        };
        avm_put_storage(handle, &address, &key, &value);
    }
}