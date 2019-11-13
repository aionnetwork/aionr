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

use std::env;
use std::process::Command;

#[cfg(target_os = "linux")]
fn main() {
    let outdir: String = env::var("OUT_DIR").unwrap();
    // check llvm devel package
    let llvm_installed = Command::new("dpkg")
        .arg("-l")
        .arg("llvm-4.0-dev")
        .output()
        .unwrap();
    if llvm_installed.stdout.is_empty() {
        panic!("No llvm found: pls install llvm-4.0-dev(sudo apt install llvm-4.0-dev .eg)");
    }
    let plat_info = Command::new("lsb_release")
        .arg("-i")
        .output()
        .unwrap()
        .stdout;
    let (_, plat_name) = plat_info.split_at("Distributor ID:	".len());

    /*match b"Ubuntu" == &plat_name[0..6] {
        false => panic!("Unsupported on Non-Ubuntu platform"),
        true => {
            let sys_version = Command::new("lsb_release")
                .arg("--release")
                .output()
                .unwrap();
            let (_, version) = sys_version.stdout.split_at("Release:	".len());
            match &version[0..5] {
                b"18.04" => println!("found 18.04"),
                b"16.04" => println!("found 16.04"),
                _ => panic!("Unsupported version, needs 18.04/16.04"),
            }
        }
    }*/

    // rebuild fastvm library
    let status = Command::new("make")
        .arg("-C")
        .arg("native/rust_evm_intf")
        .arg(format!("{}={}", "OUTDIR", outdir))
        .status()
        .expect("failed to build fastvm");
    if status.success() {
        println!("cargo:rustc-link-search=native={}/dist", outdir);
        println!("cargo:rustc-link-lib=static=fastvm");
        println!("cargo:rustc-link-lib=LLVM-4.0");
    } else {
        panic!("build fastvm failed");
    }
}

#[cfg(target_os = "macos")]
pub fn main() {
    // TODO: check llvm installation
    // TODO: check mac os version
    // rebuild fastvm library
    let outdir: String = env::var("OUT_DIR").unwrap();
    let status = Command::new("make")
        .arg("-C")
        .arg("native/rust_evm_intf")
        .arg(format!("{}={}", "OUTDIR", outdir))
        .status()
        .expect("failed to build fastvm");
    if status.success() {
        println!("cargo:rustc-link-search=native={}/dist", outdir);
        println!("cargo:rustc-link-lib=static=fastvm");
        println!("cargo:rustc-link-lib=LLVM");
    } else {
        panic!("build fastvm failed");
    }
}

#[cfg(target_os = "windows")]
pub fn main() {}
