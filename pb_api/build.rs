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

extern crate protoc_rust;

use protoc_rust::Customize;
use std::path::Path;

fn main() {
    // check if message.rs is already generated.
    if !Path::new("src/message.rs").exists() {
        // generate protobuf message file.
        protoc_rust::run(protoc_rust::Args {
            out_dir: "src",
            input: &["protos/message.proto"],
            includes: &["protos"],
            customize: Customize {
                ..Default::default()
            },
        })
        .expect("please install google protobuf.");
        println!("protobuf file: message.proto is generated.");
    }
}
