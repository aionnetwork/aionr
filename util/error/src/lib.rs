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

use db;

use std::fmt;
use rustc_hex::FromHexError;
use rlp::DecoderError;
use aion_types::H256;

#[derive(Debug)]
/// Error in database subsystem.
pub enum BaseDataError {
    /// An entry was removed more times than inserted.
    NegativelyReferencedHash(H256),
    /// A committed value was inserted more than once.
    AlreadyExists(H256),
}

impl fmt::Display for BaseDataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            BaseDataError::NegativelyReferencedHash(hash) => {
                write!(
                    f,
                    "Entry {} removed from database more times than it was added.",
                    hash
                )
            }
            BaseDataError::AlreadyExists(hash) => {
                write!(f, "Committed key already exists in database: {}", hash)
            }
        }
    }
}

impl std::error::Error for BaseDataError {
    fn description(&self) -> &str { "Error in database subsystem" }
}

/// Util Error
#[derive(Debug, derive_more::Display, derive_more::From)]
pub enum UtilError {
    Io(::std::io::Error),
    FromHex(FromHexError),
    Decoder(DecoderError),
    BaseData(BaseDataError),
    Db(db::Error),
}

impl std::error::Error for UtilError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            UtilError::Io(err) => Some(err),
            UtilError::FromHex(err) => Some(err),
            UtilError::Decoder(err) => Some(err),
            UtilError::BaseData(err) => Some(err),
            UtilError::Db(err) => Some(err),
        }
    }
}
