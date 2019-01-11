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

use {Hash, Token, Bytes};

/// Ethereum log.
#[derive(Debug, PartialEq)]
pub struct RawLog {
    /// Indexed event params are represented as log topics.
    pub topics: Vec<Hash>,
    /// Others are just plain data.
    pub data: Bytes,
}

impl From<(Vec<Hash>, Bytes)> for RawLog {
    fn from(raw: (Vec<Hash>, Bytes)) -> Self {
        RawLog {
            topics: raw.0,
            data: raw.1,
        }
    }
}

/// Decoded log param.
#[derive(Debug, PartialEq)]
pub struct LogParam {
    /// Decoded log name.
    pub name: String,
    /// Decoded log value.
    pub value: Token,
}

/// Decoded log.
#[derive(Debug, PartialEq)]
pub struct Log {
    /// Log params.
    pub params: Vec<LogParam>,
}
