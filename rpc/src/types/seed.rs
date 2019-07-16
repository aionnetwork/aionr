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

/// Seed struct used for stratum pos
use std::fmt;
use rustc_hex::{ToHex, FromHex};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{Error, Visitor};

const LEN: usize = 96;

pub const BLANK_SEED: [u8; LEN] = [
    0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
    0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
    0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
    0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
    0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
    0u8,
];

pub struct Seed(pub [u8; LEN]);

impl Seed {
    pub fn new(bytes: [u8; LEN]) -> Seed { Seed(bytes) }
}

impl Serialize for Seed {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        let mut serialized = "0x".to_owned();
        serialized.push_str(self.0.to_hex().as_ref());
        serializer.serialize_str(serialized.as_ref())
    }
}

impl<'a> Deserialize<'a> for Seed {
    fn deserialize<D>(deserializer: D) -> Result<Seed, D::Error>
    where D: Deserializer<'a> {
        deserializer.deserialize_any(SeedVisitor)
    }
}

struct SeedVisitor;

impl<'a> Visitor<'a> for SeedVisitor {
    type Value = Seed;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a 0x-prefixed, hex-encoded vector of bytes")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where E: Error {
        if value.len() == (LEN + 1) * 2 && &value[0..2] == "0x" {
            let data = FromHex::from_hex(&value[2..]).unwrap();
            let mut res: [u8; LEN] = BLANK_SEED;
            for i in 0..LEN {
                res[i] = data[i];
            }
            Ok(Seed::new(res))
        } else {
            Err(Error::custom(
                "Invalid seed format. Expected a 0x-prefixed hex string with total 192 characters \
                 in len",
            ))
        }
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where E: Error {
        self.visit_str(value.as_ref())
    }
}

#[cfg(test)]
mod tests {
    //    use super::Seed;
    //    use serde_json;
    //    use rustc_hex::FromHex;

    #[test]
    fn test() {
        // TODO: test invalid length of &str input
    }
}
