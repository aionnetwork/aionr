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

use json;
use rlp::{self, RlpStream, UntrustedRlp, DecoderError};

#[derive(Debug, PartialEq, Clone)]
pub enum Version {
    V3,
}

impl From<json::Version> for Version {
    fn from(json: json::Version) -> Self {
        match json {
            json::Version::V3 => Version::V3,
        }
    }
}

impl Into<json::Version> for Version {
    fn into(self) -> json::Version {
        match self {
            Version::V3 => json::Version::V3,
        }
    }
}

impl rlp::Decodable for Version {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        let value: u32 = d.as_val()?;
        match value {
            3 => Ok(Version::V3),
            _ => Err(DecoderError::Custom("Invalid version value.")),
        }
    }
}

impl rlp::Encodable for Version {
    fn rlp_append(&self, s: &mut RlpStream) { s.append_internal(&3_u8); }
}
