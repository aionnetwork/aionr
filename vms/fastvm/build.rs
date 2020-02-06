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

extern crate cmake;

#[cfg(target_os = "linux")]
fn main() {
    let mut config = cmake::Config::new("native/rust_evm_intf");

    let dst = config.build_target("fastvm").build();
    println!("cargo:rustc-link-search=native={}/build", dst.display());
    println!("cargo:rustc-link-lib=static=fastvm");
    println!("cargo:rustc-link-lib=LLVM-4.0");
}

#[cfg(target_os = "macos")]
pub fn main() {
    let mut config = cmake::Config::new("native/rust_evm_intf");

    let dst = config.build_target("fastvm").build();
    println!("cargo:rustc-link-search=native={}/build", dst.display());
    println!("cargo:rustc-link-lib=static=fastvm");
    println!("cargo:rustc-link-lib=LLVM");
}

#[cfg(target_os = "windows")]
pub fn main() {}
