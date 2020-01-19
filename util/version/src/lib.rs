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

#![warn(unused_extern_crates)]
//! Aion version specific information.

use target_info::Target;

mod vergen {
    #![allow(unused)]
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

mod generated {
    include!(concat!(env!("OUT_DIR"), "/meta.rs"));
}

/// Get the platform identifier.
pub fn platform() -> String {
    let env = Target::env();
    let env_dash = if env.is_empty() { "" } else { "-" };
    format!("{}-{}{}{}", Target::arch(), Target::os(), env_dash, env)
}

/// Get the standard version string for this software.
pub fn version() -> String {
    format!(
        "Aion(R)/v{}/{}/rustc-{}",
        short_version(),
        platform(),
        generated::rustc_version()
    )
}

pub fn short_version() -> String {
    let sha3 = vergen::short_sha();
    let sha3_dot = if sha3.is_empty() { "" } else { "." };
    format!("{}{}{}", env!("CARGO_PKG_VERSION"), sha3_dot, sha3)
}
