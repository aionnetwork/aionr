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

use context::ExecutionContext;
use callback::register_cbs;
use std::mem;
use std::ptr;
use bincode::{serialize};
use ffi::*;

const REVISION_AION_V1: i32 = 7;

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
            revision: REVISION_AION_V1,
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

    pub fn create() -> i64 { unsafe { fastvm_create() } }

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