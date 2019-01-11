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

use std::fmt::{Display, Formatter, Error as FmtError};
use util_error::UtilError;
use kvdb;
use trie::TrieError;

/// Client configuration errors.
#[derive(Debug)]
pub enum Error {
    /// TrieDB-related error.
    Trie(TrieError),
    /// Database error
    Database(kvdb::Error),
    /// Util error
    Util(UtilError),
}

impl From<TrieError> for Error {
    fn from(err: TrieError) -> Self { Error::Trie(err) }
}

impl From<UtilError> for Error {
    fn from(err: UtilError) -> Self { Error::Util(err) }
}

impl<E> From<Box<E>> for Error
where Error: From<E>
{
    fn from(err: Box<E>) -> Self { Error::from(*err) }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match *self {
            Error::Trie(ref err) => write!(f, "{}", err),
            Error::Util(ref err) => write!(f, "{}", err),
            Error::Database(ref s) => write!(f, "Database error: {}", s),
        }
    }
}
