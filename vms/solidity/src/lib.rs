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

use libc;

use std::ffi::CStr;

pub struct CompileResult {
    pub stdout: String,
    pub stderr: String,
}

#[link(name = "solc", kind = "static")]
extern {
    fn solc_compile(sol: *const libc::c_uchar) -> *const libc::c_char;
}

pub fn compile(sol: &[u8]) -> Result<CompileResult, &str> {
    unsafe {
        let mut contract_data = Vec::with_capacity(sol.len() + 1);
        contract_data.extend_from_slice(sol);
        contract_data.extend_from_slice(&[0x00u8]);
        let result_ptr = solc_compile(contract_data.as_ptr());
        let result = CStr::from_ptr(result_ptr);
        let out = String::from(result.to_str().map_err(|_| "compile Error")?);

        Ok(CompileResult {
            stdout: String::from(out),
            stderr: String::from(""),
        })
    }
}
