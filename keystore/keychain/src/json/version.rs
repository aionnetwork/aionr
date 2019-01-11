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

use std::fmt;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::{Error as SerdeError, Visitor};
use super::Error;

#[derive(Debug, PartialEq)]
pub enum Version {
    V3,
}

impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        match *self {
            Version::V3 => serializer.serialize_u64(3),
        }
    }
}

impl<'a> Deserialize<'a> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Version, D::Error>
    where D: Deserializer<'a> {
        deserializer.deserialize_any(VersionVisitor)
    }
}

struct VersionVisitor;

impl<'a> Visitor<'a> for VersionVisitor {
    type Value = Version;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a valid key version identifier")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where E: SerdeError {
        match value {
            3 => Ok(Version::V3),
            _ => Err(SerdeError::custom(Error::UnsupportedVersion)),
        }
    }
}
