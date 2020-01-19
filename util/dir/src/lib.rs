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

//! Dir utilities for platform-specific operations

#[macro_use]
extern crate log;

pub mod helpers;
use std::{env, fs};
use std::path::{PathBuf, Path};
use aion_types::{H64, H256};
use journaldb::Algorithm;
use crate::helpers::{replace_home, replace_home_and_local};
// re-export platform-specific functions
use platform::*;

/// Platform-specific chains path - Windows only
#[cfg(target_os = "windows")]
pub const CHAINS_PATH: &'static str = "$LOCAL/chains";
/// Platform-specific chains path
#[cfg(not(target_os = "windows"))]
pub const CHAINS_PATH: &'static str = "$BASE/chains";

/// Platform-specific cache path - Windows only
#[cfg(target_os = "windows")]
pub const CACHE_PATH: &'static str = "$LOCAL/cache";
/// Platform-specific cache path
#[cfg(not(target_os = "windows"))]
pub const CACHE_PATH: &'static str = "$BASE/cache";

/// Platform-specific keys path - Windows only
#[cfg(target_os = "windows")]
pub const KEYS_PATH: &'static str = "$LOCAL/keys";
/// Platform-specific cache path
#[cfg(not(target_os = "windows"))]
pub const KEYS_PATH: &'static str = "$BASE/keys";

/// Platform-specific zmq path - Windows only
#[cfg(target_os = "windows")]
pub const CONFIG_PATH: &'static str = "$LOCAL/config.toml";
/// Platform-specific cache path
#[cfg(not(target_os = "windows"))]
pub const CONFIG_PATH: &'static str = "$BASE/config.toml";

#[derive(Debug, PartialEq)]
/// Aion local data directories
pub struct Directories {
    /// Base dir
    pub base: String,
    /// Database dir
    pub db: String,
    /// Cache dir
    pub cache: String,
    /// Dir to store keys
    pub keys: String,
    /// config dir
    pub config: Option<String>,
}

impl Default for Directories {
    fn default() -> Self {
        let data_dir = default_data_path();
        let local_dir = default_local_path();
        Directories {
            base: replace_home(&data_dir, "$BASE"),
            db: replace_home_and_local(&data_dir, &local_dir, CHAINS_PATH),
            cache: replace_home_and_local(&data_dir, &local_dir, CACHE_PATH),
            keys: replace_home_and_local(&data_dir, &local_dir, KEYS_PATH),
            config: Some(replace_home_and_local(&data_dir, &local_dir, CONFIG_PATH)),
        }
    }
}

impl Directories {
    /// Create local directories
    pub fn create_dirs(&self) -> Result<(), String> {
        fs::create_dir_all(&self.base).map_err(|e| e.to_string())?;
        fs::create_dir_all(&self.db).map_err(|e| e.to_string())?;
        fs::create_dir_all(&self.cache).map_err(|e| e.to_string())?;
        fs::create_dir_all(&self.keys).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Database paths.
    pub fn database(&self, genesis_hash: H256, spec_name: String) -> DatabaseDirectories {
        DatabaseDirectories {
            path: self.db.clone(),
            genesis_hash: genesis_hash,
            spec_name: spec_name,
        }
    }

    /// Get the ipc sockets path
    pub fn ipc_path(&self) -> PathBuf {
        let mut dir = Path::new(&self.base).to_path_buf();
        dir.push("ipc");
        dir
    }

    /// Get the keys path
    pub fn keys_path(&self, spec_name: &str) -> PathBuf {
        let mut dir = PathBuf::from(&self.keys);
        dir.push(spec_name);
        dir
    }
}

#[derive(Debug, PartialEq)]
/// Database directories for the given fork.
pub struct DatabaseDirectories {
    /// Base path
    pub path: String,
    /// Genesis hash
    pub genesis_hash: H256,
    /// Name of current spec
    pub spec_name: String,
}

impl DatabaseDirectories {
    /// Spec root directory for the given fork.
    pub fn spec_root_path(&self) -> PathBuf { Path::new(&self.path).join(&self.spec_name) }

    /// Generic client path
    pub fn client_path(&self, pruning: Algorithm) -> PathBuf {
        self.db_root_path()
            .join(pruning.as_internal_name_str())
            .join("db")
    }

    /// DB root path, named after genesis hash
    pub fn db_root_path(&self) -> PathBuf {
        self.spec_root_path()
            .join("db")
            .join(format!("{:x}", H64::from(self.genesis_hash)))
    }

    /// DB path
    pub fn db_path(&self, pruning: Algorithm) -> PathBuf {
        self.db_root_path().join(pruning.as_internal_name_str())
    }

    /// Get user defauls path
    pub fn user_defaults_path(&self) -> PathBuf { self.spec_root_path().join("user_defaults") }
}

/// Default data path
pub fn default_data_path() -> String {
    let mut home = home();
    home.push(".aion");
    home.to_string_lossy().into()
}

/// Default local path
pub fn default_local_path() -> String {
    let mut home = home();
    home.push(".aion");
    home.to_string_lossy().into()
}

/// Get home directory.
fn home() -> PathBuf {
    #![allow(warnings)]
    env::home_dir().expect("Failed to get home dir")
}

/// Aion path for specific chain
pub fn aion(chain: &str) -> PathBuf {
    let mut base = aion_base();
    base.push(chain);
    base
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::PathBuf;

    pub fn aion_base() -> PathBuf {
        let mut home = super::home();
        home.push("Library");
        home.push("Application Support");
        home.push("Aion");
        home.push("keys");
        home
    }

}

#[cfg(windows)]
mod platform {
    use std::path::PathBuf;

    pub fn aion_base() -> PathBuf {
        let mut home = super::home();
        home.push("AppData");
        home.push("Roaming");
        home.push("Aion");
        home.push("keys");
        home
    }

}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    use std::path::PathBuf;

    pub fn aion_base() -> PathBuf {
        let mut home = super::home();
        home.push(".aion");
        home.push("keys");
        home
    }

}

#[cfg(test)]
mod tests {
    use super::Directories;
    use crate::helpers::{replace_home, replace_home_and_local};

    #[test]
    fn test_default_directories() {
        let data_dir = super::default_data_path();
        let local_dir = super::default_local_path();
        let expected = Directories {
            base: replace_home(&data_dir, "$BASE"),
            db: replace_home_and_local(
                &data_dir,
                &local_dir,
                if cfg!(target_os = "windows") {
                    "$LOCAL/chains"
                } else {
                    "$BASE/chains"
                },
            ),
            cache: replace_home_and_local(
                &data_dir,
                &local_dir,
                if cfg!(target_os = "windows") {
                    "$LOCAL/cache"
                } else {
                    "$BASE/cache"
                },
            ),
            keys: replace_home(&data_dir, "$BASE/keys"),
            config: Some(replace_home(&data_dir, "$BASE/config.toml")),
        };
        assert_eq!(expected, Directories::default());
    }
}
