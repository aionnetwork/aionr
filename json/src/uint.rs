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

//! Lenient uint json deserialization for test json files.

use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Deserializer};
use serde::de::{Error, Visitor, Unexpected};
use aion_types::{U128, U256};

/// Lenient uint json deserialization for test json files.
#[derive(
    Default,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Clone,
    Copy
)]
pub struct Uint(pub U256);

impl Into<U256> for Uint {
    fn into(self) -> U256 { self.0 }
}

impl Into<U128> for Uint {
    fn into(self) -> U128 { self.0.into() }
}

impl Into<u64> for Uint {
    fn into(self) -> u64 { u64::from(self.0) }
}

impl Into<u32> for Uint {
    fn into(self) -> u32 { u64::from(self.0) as u32 }
}

impl Into<usize> for Uint {
    fn into(self) -> usize {
        // TODO: clean it after util conversions refactored.
        u64::from(self.0) as usize
    }
}
impl Into<u8> for Uint {
    fn into(self) -> u8 { u64::from(self.0) as u8 }
}

impl<'a> Deserialize<'a> for Uint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'a> {
        deserializer.deserialize_any(UintVisitor)
    }
}

struct UintVisitor;

impl<'a> Visitor<'a> for UintVisitor {
    type Value = Uint;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a hex encoded or decimal uint")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where E: Error {
        Ok(Uint(U256::from(value)))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where E: Error {
        let value = match value.len() {
            0 => U256::from(0),
            2 if value.starts_with("0x") => U256::from(0),
            _ if value.starts_with("0x") => {
                U256::from_str(&value[2..]).map_err(|e| {
                    Error::custom(format!("Invalid hex value {}: {}", value, e).as_str())
                })?
            }
            _ => {
                U256::from_dec_str(value).map_err(|e| {
                    Error::custom(format!("Invalid decimal value {}: {:?}", value, e).as_str())
                })?
            }
        };

        Ok(Uint(value))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where E: Error {
        self.visit_str(value.as_ref())
    }
}

pub fn validate_non_zero<'de, D>(d: D) -> Result<Uint, D::Error>
where D: Deserializer<'de> {
    let value = Uint::deserialize(d)?;

    if value == Uint(U256::from(0)) {
        return Err(Error::invalid_value(
            Unexpected::Unsigned(value.into()),
            &"a non-zero value",
        ));
    }

    Ok(value)
}

pub fn validate_optional_non_zero<'de, D>(d: D) -> Result<Option<Uint>, D::Error>
where D: Deserializer<'de> {
    let value: Option<Uint> = Option::deserialize(d)?;

    if let Some(value) = value {
        if value == Uint(U256::from(0)) {
            return Err(Error::invalid_value(
                Unexpected::Unsigned(value.into()),
                &"a non-zero value",
            ));
        }
    }

    Ok(value)
}
