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

extern crate fastvm;
extern crate libc;
#[macro_use]
extern crate log;
extern crate rustc_hex;

use fastvm::core::FastVM;
use rustc_hex::FromHex;
use fastvm::context::ExecutionContext;

extern {
    fn clock() -> ::libc::clock_t;
}

#[test]
fn create_evm() {
    let instance = FastVM::create();
    info!(target: "vm", "fastvm instance = {:?}", instance);
    assert_ne!(instance as i32, 0);
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