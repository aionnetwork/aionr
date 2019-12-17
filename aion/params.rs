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

use std::{str, fs, fmt};
use aion_types::{U256, Address};
use journaldb::Algorithm;
use acore::spec::{Spec};
use user_defaults::UserDefaults;

#[derive(Debug, PartialEq)]
pub enum SpecType {
    Default,
    Custom(String),
}

impl Default for SpecType {
    fn default() -> Self { SpecType::Default }
}

impl str::FromStr for SpecType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let spec = match s {
            "foundation" | "mainnet" => SpecType::Default,
            other => SpecType::Custom(other.into()),
        };
        Ok(spec)
    }
}

impl fmt::Display for SpecType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            SpecType::Default => "mainnet",
            SpecType::Custom(ref custom) => custom,
        })
    }
}

impl SpecType {
    pub fn spec<'a>(&self) -> Result<Spec, String> {
        let file;
        match *self {
            SpecType::Default => {
                return Ok(Spec::new_foundation());
            }
            SpecType::Custom(ref filename) => {
                file = fs::File::open(filename).map_err(|e| {
                    format!("Could not load specification file at {}: {}", filename, e)
                })?;
            }
        }
        Spec::load(file)
    }
}

#[derive(Debug, PartialEq)]
pub enum Pruning {
    Specific(Algorithm),
    Auto,
}

impl Default for Pruning {
    fn default() -> Self { Pruning::Specific(Algorithm::Archive) }
}

impl str::FromStr for Pruning {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Pruning::Auto),
            other => other.parse().map(Pruning::Specific),
        }
    }
}

impl Pruning {
    pub fn to_algorithm(&self, user_defaults: &UserDefaults) -> Algorithm {
        match *self {
            Pruning::Specific(algo) => algo,
            Pruning::Auto => user_defaults.pruning,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct AccountsConfig {
    pub iterations: u32,
    pub refresh_time: u64,
    pub password_files: Vec<String>,
    pub unlocked_accounts: Vec<Address>,
    pub enable_fast_signing: bool,
}

impl Default for AccountsConfig {
    fn default() -> Self {
        AccountsConfig {
            iterations: 10240,
            refresh_time: 5,
            password_files: Vec::new(),
            unlocked_accounts: Vec::new(),
            enable_fast_signing: false,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct StakeConfig {
    pub contract: Address,
}

impl Default for StakeConfig {
    fn default() -> Self {
        StakeConfig {
            contract: Address::default(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MinerExtras {
    pub author: Address,
    pub extra_data: Vec<u8>,
    pub gas_floor_target: U256,
    pub gas_ceil_target: U256,
}

impl Default for MinerExtras {
    fn default() -> Self {
        MinerExtras {
            author: Default::default(),
            extra_data: "AION".as_bytes().to_vec(),
            gas_floor_target: U256::from(15_000_000),
            gas_ceil_target: U256::from(20_000_000),
        }
    }
}

/// 3-value enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Switch {
    /// True.
    On,
    /// False.
    Off,
    /// Auto.
    Auto,
}

impl Default for Switch {
    fn default() -> Self { Switch::Auto }
}

impl str::FromStr for Switch {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "on" => Ok(Switch::On),
            "off" => Ok(Switch::Off),
            "auto" => Ok(Switch::Auto),
            other => Err(format!("Invalid switch value: {}", other)),
        }
    }
}

pub fn fatdb_switch_to_bool(
    switch: Switch,
    user_defaults: &UserDefaults,
    _algorithm: Algorithm,
) -> Result<bool, String>
{
    let result = match (user_defaults.is_first_launch, switch, user_defaults.fat_db) {
        (false, Switch::On, false) => Err("FatDB resync required".into()),
        (_, Switch::On, _) => Ok(true),
        (_, Switch::Off, _) => Ok(false),
        (_, Switch::Auto, def) => Ok(def),
    };
    result
}

#[cfg(test)]
mod tests {
    use journaldb::Algorithm;
    use super::{SpecType, Pruning, Switch};

    #[test]
    fn test_spec_type_parsing() {
        assert_eq!(SpecType::Default, "mainnet".parse().unwrap());
    }

    #[test]
    fn test_spec_type_default() {
        assert_eq!(SpecType::Default, SpecType::default());
    }

    #[test]
    fn test_spec_type_display() {
        assert_eq!(format!("{}", SpecType::Default), "mainnet");
        assert_eq!(format!("{}", SpecType::Custom("foo/bar".into())), "foo/bar");
    }

    #[test]
    fn test_pruning_parsing() {
        assert_eq!(Pruning::Auto, "auto".parse().unwrap());
        assert_eq!(
            Pruning::Specific(Algorithm::Archive),
            "archive".parse().unwrap()
        );
        assert_eq!(
            Pruning::Specific(Algorithm::OverlayRecent),
            "fast".parse().unwrap()
        );
    }

    #[test]
    fn test_pruning_default() {
        assert_eq!(Pruning::Specific(Algorithm::Archive), Pruning::default());
    }

    #[test]
    fn test_switch_parsing() {
        assert_eq!(Switch::On, "on".parse().unwrap());
        assert_eq!(Switch::Off, "off".parse().unwrap());
        assert_eq!(Switch::Auto, "auto".parse().unwrap());
    }

    #[test]
    fn test_switch_default() {
        assert_eq!(Switch::default(), Switch::Auto);
    }
}
