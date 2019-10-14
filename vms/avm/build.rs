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

fn main() {
    let outdir: String = env::var("OUT_DIR").unwrap();
    let profile = env::var("PROFILE").unwrap();
    // build avm library
    let status = Command::new("make")
        .arg("-C")
        .arg("libs/native_loader/native")
        .arg(format!("{}={}", "OUTDIR", outdir))
        .arg(profile.clone())
        .status()
        .expect("failed to build native loader library");

    if !status.success() {
        panic!("build native library failed");
    }

    if !Command::new("ant")
        .arg("-f")
        .arg("libs/native_loader/build.xml")
        .status()
        .expect("failed to build native loader")
        .success()
    {
        panic!("failed to build avm native loader");
    }

    if !Command::new("ant")
        .arg("-f")
        .arg("libs/version/build.xml")
        .status()
        .expect("failed to build avm version provider")
        .success()
    {
        panic!("failed to build avm version provider");
    }

    println!("cargo:rustc-link-search=native={}", outdir);

    // NOTE: build jni jar package
    if !Command::new("ant")
        .arg("-f")
        .arg("libs/avmjni_v1/build.xml")
        .status()
        .expect("failed to build jni v1 jar")
        .success()
    {
        panic!("build jni v1 failed");
    }
    if !Command::new("ant")
        .arg("-f")
        .arg("libs/avmjni_v2/build.xml")
        .status()
        .expect("failed to build jni v2 jar")
        .success()
    {
        panic!("build jni v2 failed");
    }
}
