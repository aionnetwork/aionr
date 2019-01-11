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

#[derive(Debug)]
/// Crypto error
pub enum Error {
    /// Invalid secret key
    InvalidSecret,
    /// Invalid public key
    InvalidPublic,
    /// Invalid address
    InvalidAddress,
    /// Invalid EC signature
    InvalidSignature,
    /// Invalid AES message
    InvalidMessage,
    /// IO Error
    Io(::std::io::Error),
    /// Custom
    Custom(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match *self {
            Error::InvalidSecret => "Invalid secret".into(),
            Error::InvalidPublic => "Invalid public".into(),
            Error::InvalidAddress => "Invalid address".into(),
            Error::InvalidSignature => "Invalid EC signature".into(),
            Error::InvalidMessage => "Invalid AES message".into(),
            Error::Io(ref err) => format!("I/O error: {}", err),
            Error::Custom(ref s) => s.clone(),
        };

        f.write_fmt(format_args!("Crypto error ({})", msg))
    }
}

impl Into<String> for Error {
    fn into(self) -> String { format!("{}", self) }
}

impl From<::std::io::Error> for Error {
    fn from(err: ::std::io::Error) -> Error { Error::Io(err) }
}
