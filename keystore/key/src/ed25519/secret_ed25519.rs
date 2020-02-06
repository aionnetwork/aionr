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

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use rustc_hex::ToHex;
use aion_types::H512;
use {Error};

#[derive(Clone, PartialEq, Eq)]
pub struct Ed25519Secret {
    inner: H512,
}

impl ToHex for Ed25519Secret {
    fn to_hex(&self) -> String { self.inner.to_hex() }
}

impl fmt::Debug for Ed25519Secret {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "Secret: 0x{:x}{:x}..{:x}{:x}",
            self.inner[0], self.inner[1], self.inner[62], self.inner[63]
        )
    }
}

impl Ed25519Secret {
    pub fn from_slice(key: &[u8]) -> Option<Self> {
        if key.len() != 64 {
            return None;
        }
        let mut h = H512::default();
        h.copy_from_slice(&key[0..64]);
        Some(Ed25519Secret {
            inner: h,
        })
    }
}

impl FromStr for Ed25519Secret {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(H512::from_str(s)
            .map_err(|e| Error::Custom(format!("{:?}", e)))?
            .into())
    }
}

impl From<H512> for Ed25519Secret {
    fn from(s: H512) -> Self { Ed25519Secret::from_slice(&s).unwrap() }
}

impl From<&'static str> for Ed25519Secret {
    fn from(s: &'static str) -> Self {
        s.parse().expect(&format!(
            "invalid string literal for {}: '{}'",
            stringify!(Self),
            s
        ))
    }
}

impl Deref for Ed25519Secret {
    type Target = H512;

    fn deref(&self) -> &Self::Target { &self.inner }
}
