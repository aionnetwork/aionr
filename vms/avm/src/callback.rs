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

#[derive(Debug)]
#[repr(C)]
pub struct avm_bytes {
    pub length: u32,
    pub pointer: *mut u8,
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
    unsafe {
        let result: u32 = 1;
        println!(
            "Callback: avm_has_account_state({:?}, {}) => {}",
            handle, *address, result
        );
        result
    }
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
    ext.save_code(addr, ext_code.to_vec());
}

#[no_mangle]
pub extern fn avm_get_code(handle: *const c_void, address: *const avm_address) -> avm_bytes {
    let ext: &mut Box<AVMExt> = unsafe { mem::transmute(handle) };
    let addr: &Address = unsafe { mem::transmute(address) };
    match ext.get_code(addr) {
        None => {
            avm_bytes {
                length: 0,
                pointer: ptr::null_mut(),
            }
        }
        Some(code) => {
            avm_bytes {
                length: code.len() as u32,
                pointer: unsafe { mem::transmute(&code.as_slice()[0]) },
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
    unsafe {
        println!(
            "Callback: avm_put_storage({:?}, {}, {}, {})",
            handle, *address, *key, *value
        );
    }
}

#[no_mangle]
pub extern fn avm_get_storage(
    handle: *const c_void,
    address: *const avm_address,
    key: *const avm_bytes,
) -> avm_bytes
{
    unsafe {
        let value = new_null_bytes();
        println!(
            "Callback: avm_get_storage({:?}, {}, {}) => {}",
            handle, *address, *key, value
        );
        value
    }
}

#[no_mangle]
pub extern fn avm_delete_account(handle: *const c_void, address: *const avm_address) {
    unsafe {
        println!("Callback: avm_delete_account({:?}, {})", handle, *address);
    }
}

#[no_mangle]
pub extern fn avm_get_balance(handle: *const c_void, address: *const avm_address) -> avm_value {
    // let ext: &mut Box<AVMExt> = unsafe { mem::transmute(handle) };
    unsafe {
        let balance = avm_value {
            bytes: [1u8; 32],
        };
        println!(
            "Callback: avm_get_balance({:?}, {}) => {}",
            handle, *address, balance
        );
        balance
    }
}

#[no_mangle]
pub extern fn avm_increase_balance(
    handle: *const c_void,
    address: *const avm_address,
    value: *const avm_value,
)
{
    unsafe {
        println!(
            "Callback: avm_increase_balance({:?}, {}, {})",
            handle, *address, *value
        );
    }
}

#[no_mangle]
pub extern fn avm_decrease_balance(
    handle: *const c_void,
    address: *const avm_address,
    value: *const avm_value,
)
{
    unsafe {
        println!(
            "Callback: avm_decrease_balance({:?}, {}, {})",
            handle, *address, *value
        );
    }
}

#[no_mangle]
pub extern fn avm_get_nonce(handle: *const c_void, address: *const avm_address) -> u64 {
    unsafe {
        let nonce = 0;
        println!(
            "Callback: avm_get_nonce({:?}, {}) => {}",
            handle, *address, nonce
        );
        nonce
    }
}

#[no_mangle]
pub extern fn avm_increment_nonce(handle: *const c_void, address: *const avm_address) {
    unsafe {
        println!("Callback: avm_increment_nonce({:?}, {})", handle, *address);
    }
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
