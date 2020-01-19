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

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::cmp::Ordering;
use key::{Address, Ed25519Signature, Ed25519Secret, Message, sign_ed25519};
use crate::Error;
use crate::json::{Uuid, OpaqueKeyFile};
use aion_types::H256;
use crate::OpaqueSecretEd25519;

/// Stored account reference
#[derive(Debug, Clone, PartialEq, Eq, Ord)]
pub struct StoreAccountRef {
    /// Account address
    pub address: Address,
}

impl PartialOrd for StoreAccountRef {
    fn partial_cmp(&self, other: &StoreAccountRef) -> Option<Ordering> {
        Some(self.address.cmp(&other.address))
    }
}

impl ::std::borrow::Borrow<Address> for StoreAccountRef {
    fn borrow(&self) -> &Address { &self.address }
}

/// Simple Secret Store API
pub trait SimpleSecretStore: Send + Sync {
    /// Inserts new accounts to the store (or vault) with given password.
    fn insert_account_ed25519(
        &self,
        secret: Ed25519Secret,
        password: &str,
    ) -> Result<StoreAccountRef, Error>;
    /// Exports key details for account.
    fn export_account(
        &self,
        account: &StoreAccountRef,
        password: &str,
    ) -> Result<OpaqueKeyFile, Error>;
    /// Entirely removes account from the store and underlying storage.
    fn remove_account(&self, account: &StoreAccountRef, password: &str) -> Result<(), Error>;
    /// Sign a message with given account.
    fn sign_ed25519(
        &self,
        _account: &StoreAccountRef,
        _password: &str,
        _message: &Message,
    ) -> Result<Ed25519Signature, Error>
    {
        unimplemented!()
    }
    /// Returns all accounts in this secret store.
    fn accounts(&self) -> Result<Vec<StoreAccountRef>, Error>;
    /// Get reference to some account with given address.
    /// This method could be removed if we will guarantee that there is max(1) account for given address.
    fn account_ref(&self, address: &Address) -> Result<StoreAccountRef, Error>;

    /// Create new vault with given password
    fn create_vault(&self, name: &str, password: &str) -> Result<(), Error>;
    /// Open vault with given password
    fn open_vault(&self, name: &str, password: &str) -> Result<(), Error>;
    /// Close vault
    fn close_vault(&self, name: &str) -> Result<(), Error>;
    /// List all vaults
    fn list_vaults(&self) -> Result<Vec<String>, Error>;
    /// List all currently opened vaults
    fn list_opened_vaults(&self) -> Result<Vec<String>, Error>;
    /// Change vault password
    fn change_vault_password(&self, name: &str, new_password: &str) -> Result<(), Error>;
    /// Cnage account' vault
    fn change_account_vault(&self, account: StoreAccountRef) -> Result<StoreAccountRef, Error>;
    /// Get vault metadata string.
    fn get_vault_meta(&self, name: &str) -> Result<String, Error>;
    /// Set vault metadata string.
    fn set_vault_meta(&self, name: &str, meta: &str) -> Result<(), Error>;
}

/// Secret Store API
pub trait SecretStore: SimpleSecretStore {
    /// Returns a raw opaque Secret that can be later used to sign a message.
    fn raw_secret(
        &self,
        account: &StoreAccountRef,
        password: &str,
    ) -> Result<OpaqueSecretEd25519, Error>;

    /// Signs a message with raw secret.
    fn sign_with_secret(
        &self,
        secret: &OpaqueSecretEd25519,
        message: &Message,
    ) -> Result<Ed25519Signature, Error>
    {
        Ok(sign_ed25519(&secret.0, message)?)
    }

    /// Checks if password matches given account.
    fn test_password(&self, account: &StoreAccountRef, password: &str) -> Result<bool, Error>;

    /// Returns uuid of an account.
    fn uuid(&self, account: &StoreAccountRef) -> Result<Uuid, Error>;
    /// Returns account's name.
    fn name(&self, account: &StoreAccountRef) -> Result<String, Error>;
    /// Returns account's metadata.
    fn meta(&self, account: &StoreAccountRef) -> Result<String, Error>;

    /// Modifies account metadata.
    fn set_name(&self, account: &StoreAccountRef, name: String) -> Result<(), Error>;
    /// Modifies account name.
    fn set_meta(&self, account: &StoreAccountRef, meta: String) -> Result<(), Error>;

    /// Returns local path of the store.
    fn local_path(&self) -> PathBuf;
}

impl StoreAccountRef {
    /// Create new account reference
    pub fn new(address: Address) -> Self {
        StoreAccountRef {
            address,
        }
    }
}

impl Hash for StoreAccountRef {
    fn hash<H: Hasher>(&self, state: &mut H) { self.address.hash(state); }
}

/// Node in hierarchical derivation.
pub struct IndexDerivation {
    /// Node is soft (allows proof of parent from parent node).
    pub soft: bool,
    /// Index sequence of the node.
    pub index: u32,
}

/// Derivation scheme for keys
pub enum Derivation {
    /// Hierarchical derivation
    Hierarchical(Vec<IndexDerivation>),
    /// Hash derivation, soft.
    SoftHash(H256),
    /// Hash derivation, hard.
    HardHash(H256),
}
