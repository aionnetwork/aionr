/*******************************************************************************
 * Copyright (c) 2015-2018 Parity Technologies (UK) Ltd.
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

extern crate rustc_version;
extern crate toml;
extern crate vergen;

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use vergen::{vergen, OutputFns};

const ERROR_MSG: &'static str = "Failed to generate metadata files";

fn main() {
    vergen(OutputFns::all()).expect(ERROR_MSG);

    let version = rustc_version::version().expect(ERROR_MSG);
    create_file(
        "meta.rs",
        format!(
            "
            /// Returns compiler version.
            pub fn rustc_version() -> \
             &'static str {{
                \"{version}\"
            }}
        ",
            version = version,
        ),
    );

    let aion_version = env::var("CARGO_PKG_VERSION").expect(ERROR_MSG);
    let current_dir = env::var("CARGO_MANIFEST_DIR").expect(ERROR_MSG);
    create_package_version(
        format!(
            "{}/../../release", current_dir
        ).as_str(),
        aion_version
    );
}

fn create_package_version(filename: &str, data: String) {
    let dest_path = Path::new(filename);
    let mut f = File::create(&dest_path).expect(ERROR_MSG);
    f.write_all(data.as_bytes()).expect(ERROR_MSG);
}

fn create_file(filename: &str, data: String) {
    let out_dir = env::var("OUT_DIR").expect(ERROR_MSG);
    let dest_path = Path::new(&out_dir).join(filename);
    let mut f = File::create(&dest_path).expect(ERROR_MSG);
    f.write_all(data.as_bytes()).expect(ERROR_MSG);
}
