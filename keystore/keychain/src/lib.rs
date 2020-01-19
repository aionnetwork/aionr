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

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

pub mod accounts_dir;
pub mod ethkey;
pub mod secret_store;

use crypto;
mod account;
mod json;

mod error;
mod ethstore;
mod import;
mod random;

#[cfg(test)]
mod tests;

pub use self::account::{SafeAccount, Crypto};
pub use self::error::Error;
pub use self::ethstore::{EthStore, EthMultiStore};
pub use self::import::{import_account, import_accounts};
pub use self::json::OpaqueKeyFile as KeyFile;
pub use self::secret_store::{
    StoreAccountRef, SimpleSecretStore, SecretStore, Derivation, IndexDerivation,
};
pub use self::random::random_string;

/// An opaque wrapper for secret.
pub struct OpaqueSecretEd25519(ethkey::Ed25519Secret);
