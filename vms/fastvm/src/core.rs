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

#![allow(dead_code)]

use context::ExecutionContext;
use callback::register_cbs;
use std::mem;
use std::ptr;
use bincode::{serialize};
use ffi::*;

const REVISION_AION: i32 = 5;

impl EvmJit<u8> for EvmResult {
    fn to_evm_jit(&mut self) -> *mut u8 { unsafe { mem::transmute(self) } }

    fn from_evm_jit(input: *const u8) -> *const Self { unsafe { mem::transmute(input) } }
}

impl EvmResult {
    pub fn new() -> Self {
        EvmResult {
            status_code: EvmStatusCode::Success,
            gas_left: 0,
            //output_data: unsafe {mem::transmute(&tmp_slice[0])},
            output_data: alloc_default_buf(),
            output_size: 3,
        }
    }

    fn release(&self) {
        let out_ptr: *mut ::libc::c_void = unsafe { mem::transmute(self.output_data) };
        if !out_ptr.is_null() {
            unsafe { ::libc::free(out_ptr) };
        }
    }
}

/// The kind of call-like instruction.
enum EvmCallKind {
    EvmCall = 0,         //< Request Call.
    EvmDelegateCall = 1, //< Request DELEGATECALL. The value param ignored.
    EvmCallCode = 2,     //< Request CALLCODE.
    EvmCreate = 3,       //< Request CREATE. Semantic of some params changes.
}

/*
 * The FastVM implementation. It calls into the jit library via ffi.
 *
 */
#[derive(Debug, Clone)]
pub struct FastVM {
    revision: i32,
    flag_static: i32,
    vm_instance: i64,
}

impl FastVM {
    pub fn new() -> FastVM {
        FastVM {
            revision: REVISION_AION,
            flag_static: 1,
            vm_instance: 0,
        }
    }

    pub fn init(&mut self, raw_env: *mut ::libc::c_void) {
        let instance: i64 = FastVM::create();
        debug!(target: "vm", "new vm instance = {:?}", instance);
        self.vm_instance = instance;
        debug!(target: "vm", "cbs raw env = {:?}", raw_env);
        let ret = unsafe { env_init(raw_env) };
        if ret != 0 {
            panic!("fatal error during env init");
        }
        register_cbs();
    }

    fn create() -> i64 { unsafe { fastvm_create() } }

    // return: status code, gas left, output data
    pub fn run(
        &mut self,
        code: &Vec<u8>,
        ctx: &mut ExecutionContext,
    ) -> (EvmStatusCode, i64, Vec<u8>)
    {
        let vm_inst = self.vm_instance;
        let buffer = code.as_slice();
        let vm_code: *const u8 = match code.len() > 0 {
            true => unsafe { mem::transmute(&buffer[0]) },
            false => ptr::null(),
        };

        let mut byte_ctx = Vec::<u8>::new();
        // wrap bytearray
        byte_ctx.extend(ctx.address.iter().cloned());
        byte_ctx.extend(ctx.origin.iter().cloned());
        byte_ctx.extend(ctx.caller.iter().cloned());
        byte_ctx.extend(ctx.nrg_price.data.iter().cloned());
        byte_ctx.extend(serialize(&ctx.nrg_limit).unwrap().iter().cloned());
        byte_ctx.extend(ctx.call_value.data.iter().cloned());
        byte_ctx.extend(
            serialize(&(ctx.call_data.len() as u32))
                .unwrap()
                .iter()
                .cloned(),
        );
        byte_ctx.extend(ctx.call_data.iter().cloned());
        byte_ctx.extend(serialize(&ctx.depth).unwrap().iter().cloned());
        byte_ctx.extend(serialize(&ctx.kind).unwrap().iter().cloned());
        byte_ctx.extend(serialize(&ctx.flags).unwrap().iter().cloned());
        byte_ctx.extend(ctx.block_coinbase.iter().cloned());
        byte_ctx.extend(serialize(&ctx.block_number).unwrap().iter().cloned());
        byte_ctx.extend(serialize(&ctx.block_timestamp).unwrap().iter().cloned());
        byte_ctx.extend(serialize(&ctx.block_nrglimit).unwrap().iter().cloned());
        byte_ctx.extend(ctx.block_difficulty.data.iter().cloned());

        let ctx_buffer = byte_ctx.as_slice();
        //let vm_ctx: *mut ::libc::wchar_t = unsafe { mem::transmute(&ctx_buffer[0]) };
        let vm_ctx = get_libc_pointer_of_bytes(&ctx_buffer);

        let vm_rev: i32 = self.revision.clone() as i32;
        let mut result = EvmResult::new();

        match result.output_data.is_null() {
            true => (EvmStatusCode::InternalError, 0, Vec::new()), // internal error triggers shutdown
            _ => {
                let result_p = result.to_evm_jit();
                unsafe {
                    fastvm_run(
                        vm_inst,
                        vm_code,
                        code.len() as u32,
                        vm_ctx,
                        vm_rev,
                        result_p,
                    )
                };

                let result = unsafe { EvmResult::from_evm_jit(result_p).as_ref().unwrap() };
                let output_data: &[u8] = unsafe {
                    ::std::slice::from_raw_parts(result.output_data, result.output_size as usize)
                };
                debug!(target: "vm", "vm exec result = {:?}", result);
                debug!(target: "vm", "vm exec output data = {:?}", output_data);

                let status_code = result.status_code.clone();
                let gas_left = result.gas_left;
                let mut output = Vec::<u8>::new();
                output.extend_from_slice(&output_data);

                // release malloc data
                result.release();

                debug!(target: "vm", "fastvm status code = {:?}", status_code);
                (status_code, gas_left, output)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate libc;
    use rustc_hex::FromHex;

    extern {
        fn clock() -> ::libc::clock_t;
    }

    #[test]
    fn create_evm() {
        let instance = FastVM::create();
        info!(target: "vm", "fastvm instance = {:?}", instance);
        assert!(instance as i32 != 0);
    }

    #[test]
    fn test_context_create() {
        let mut fvm_hdl = FastVM::new();
        fvm_hdl.init(::std::ptr::null_mut());
        // let code = vec![
        //     0x60, 0x00, 0x5b, 0x80,
        //     0x61, 0x04, 0x00, 0x10,
        //     0x60, 0x19, 0x57, 0x80,
        //     0x60, 0xE0, 0x51, 0x01,
        //     0x60, 0xE0, 0x52, 0x60,
        //     0x01, 0x01, 0x60, 0x02,
        //     0x56, 0x5b, 0x60, 0x10,
        //     0x60, 0xE0, 0xF3
        // ];
        let code = vec![0xff];
        let mut ctx = ExecutionContext::default();
        let res = fvm_hdl.run(&code, &mut ctx);
        println!("res = {:?}", res);

        let start = unsafe { clock() };
        for _i in 0..1000 {
            fvm_hdl.run(&code, &mut ctx);
        }
        let end = unsafe { clock() };
        println!("per execution duration = {:?}", (end - start) / 1000);
    }

    #[test]
    fn dump_llvm_ir() {
        let mut fvm_hdl = FastVM::new();
        fvm_hdl.init(::std::ptr::null_mut());
        let code = "605060405234156100105760006000fd5b610015565b60a6806100236000396000f30060506040526000356c01000000000000000000000000900463ffffffff16806332e7c5bf14603b578063f446c1d014604e576035565b60006000fd5b341560465760006000fd5b604c6061565b005b341560595760006000fd5b605f6070565b005b606d607063ffffffff16565b5b565b6000600a90505b505600a165627a7a72305820d905a1a748748eb4f0eea38afb9afed03907754600579b04adaabcc99df26a070029".from_hex().unwrap();
        let mut ctx = ExecutionContext::default();
        let res = fvm_hdl.run(&code, &mut ctx);
        println!("res = {:?}", res);
    }
}
