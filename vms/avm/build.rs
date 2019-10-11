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
        .arg("libs/avmjni/native")
        .arg(format!("{}={}", "OUTDIR", outdir))
        .arg(profile.clone())
        .status()
        .expect("failed to build jni library");

    if !status.success() {
        panic!("build native jni library failed");
    }

    println!("cargo:rustc-link-search=native={}", outdir);

    // NOTE: build jni jar package
    Command::new("ant")
        .arg("-f")
        .arg("libs/avmjni/build.xml")
        .status()
        .expect("failed to build jni jar");
    Command::new("ant")
        .arg("-f")
        .arg("libs/avmjni_v2/build.xml")
        .status()
        .expect("failed to build jni jar");

    //println!("cargo:rustc-link-lib=avmjni");

    // fetch jni jar
    // let mut jni_jar_path = env!("CARGO_MANIFEST_DIR").to_string();
    // jni_jar_path.extend("/libs/aion_vm/org-aion-avm-jni.jar".chars());
    // println!("{:?}", jni_jar_path);
    // Command::new("wget")
    //         .arg("https://github.com/aion-camus/rust_avm/releases/download/v0.5.0/org-aion-avm-jni.jar")
    //         .args(["-O", &jni_jar_path].iter())
    //         .status()
    //         .expect("fetch jni jar error");

    // // fetch avm libs
    // Command::new("wget")
    //         .arg("https://github.com/aionnetwork/AVM/archive/1.0.tar.gz")
    //         .args(["-O", "/tmp/avm.tar.gz"].iter())
    //         .status()
    //         .expect("fetch AVM error");

    // // unpack avm package and put the jars in libs dir
    // Command::new("tar")
    // .arg("-xvf")
    // .arg("/tmp/avm.tar.gz")
    // .args(["-C", "/tmp/"].iter());
}
