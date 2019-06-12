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
//! Path utilities
use std::path::Path;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
/// Get the config path for application `name`.
pub fn config_path(name: &str) -> PathBuf {
    let mut home = ::std::env::home_dir().expect("Failed to get home dir");
    home.push("Library");
    home.push(name);
    home
}

#[cfg(windows)]
/// Get the config path for application `name`.
pub fn config_path(name: &str) -> PathBuf {
    let mut home = ::std::env::home_dir().expect("Failed to get home dir");
    home.push("AppData");
    home.push("Roaming");
    home.push(name);
    home
}

#[cfg(not(any(target_os = "macos", windows)))]
/// Get the config path for application `name`.
pub fn config_path(name: &str) -> PathBuf {
    #![allow(warnings)]
    let mut home = ::std::env::home_dir().expect("Failed to get home dir");
    home.push(format!(".{}", name.to_lowercase()));
    home
}

/// Get the specific folder inside a config path.
pub fn config_path_with(name: &str, then: &str) -> PathBuf {
    let mut path = config_path(name);
    path.push(then);
    path
}

/// Restricts the permissions of given path only to the owner.
#[cfg(unix)]
pub fn restrict_permissions_owner(
    file_path: &Path,
    write: bool,
    executable: bool,
) -> Result<(), String>
{
    let perms = ::std::os::unix::fs::PermissionsExt::from_mode(
        0o400 + write as u32 * 0o200 + executable as u32 * 0o100,
    );
    ::std::fs::set_permissions(file_path, perms).map_err(|e| format!("{:?}", e))
}

/// Restricts the permissions of given path only to the owner.
#[cfg(not(unix))]
pub fn restrict_permissions_owner(
    _file_path: &Path,
    _write: bool,
    _executable: bool,
) -> Result<(), String>
{
    //TODO: implement me
    Ok(())
}
