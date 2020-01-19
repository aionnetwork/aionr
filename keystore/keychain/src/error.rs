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
use std::io::Error as IoError;
use crate::ethkey::Error as EthKeyError;
use crypto::Error as EthCryptoError;
//use crate::ethkey::DerivationError;

/// Account-related errors.
#[derive(Debug)]
pub enum Error {
    /// IO error
    Io(IoError),
    /// Invalid Password
    InvalidPassword,
    /// Account's secret is invalid.
    InvalidSecret,
    /// Invalid Vault Crypto meta.
    InvalidCryptoMeta,
    /// Invalid Account.
    InvalidAccount,
    /// Invalid Message.
    InvalidMessage,
    /// Invalid Key File
    InvalidKeyFile(String),
    /// Vaults are not supported.
    VaultsAreNotSupported,
    /// Unsupported vault
    UnsupportedVault,
    /// Invalid vault name
    InvalidVaultName,
    /// Vault not found
    VaultNotFound,
    /// Account creation failed.
    CreationFailed,
    /// `EthKey` error
    EthKey(EthKeyError),
    /// `EthCrypto` error
    EthCrypto(EthCryptoError),
    /// Custom error
    Custom(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let s = match *self {
            Error::Io(ref err) => err.to_string(),
            Error::InvalidPassword => "Invalid password".into(),
            Error::InvalidSecret => "Invalid secret".into(),
            Error::InvalidCryptoMeta => "Invalid crypted metadata".into(),
            Error::InvalidAccount => "Invalid account".into(),
            Error::InvalidMessage => "Invalid message".into(),
            Error::InvalidKeyFile(ref reason) => format!("Invalid key file: {}", reason),
            Error::VaultsAreNotSupported => "Vaults are not supported".into(),
            Error::UnsupportedVault => "Vault is not supported for this operation".into(),
            Error::InvalidVaultName => "Invalid vault name".into(),
            Error::VaultNotFound => "Vault not found".into(),
            Error::CreationFailed => "Account creation failed".into(),
            Error::EthKey(ref err) => err.to_string(),
            Error::EthCrypto(ref err) => err.to_string(),
            Error::Custom(ref s) => s.clone(),
        };

        write!(f, "{}", s)
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self { Error::Io(err) }
}

impl From<EthKeyError> for Error {
    fn from(err: EthKeyError) -> Self { Error::EthKey(err) }
}

impl From<EthCryptoError> for Error {
    fn from(err: EthCryptoError) -> Self { Error::EthCrypto(err) }
}
