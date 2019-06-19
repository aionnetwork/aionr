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

#![allow(warnings)]

//! Directory helper functions
use std::env;

/// Replaces `$HOME` str with home directory path.
pub fn replace_home(base: &str, arg: &str) -> String {
    // the $HOME directory on mac os should be `~/Library` or `~/Library/Application Support`

    let r = arg.replace(
        "$HOME",
        env::home_dir()
            .expect("$HOME isn't defined")
            .to_str()
            .expect("$HOME parse error"),
    );

    let r = r.replace("$BASE", base);
    r.replace("/", &::std::path::MAIN_SEPARATOR.to_string())
}

/// Replaces `$HOME` str with home directory path and `$LOCAL` with local path.
pub fn replace_home_and_local(base: &str, local: &str, arg: &str) -> String {
    let r = replace_home(base, arg);
    r.replace("$LOCAL", local)
}

pub fn absolute(path: String) -> String {
    if path.find("/") != Some(0) {
        format!(
            "{}/{}",
            match env::current_dir() {
                Ok(ref path) => path.to_string_lossy(),
                Err(e) => {
                    error!(target: "run","Cannot get current dir path!! err:{}", e);
                    return path;
                }
            },
            path
        )
    } else {
        path
    }
}
