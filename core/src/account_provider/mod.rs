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
use std::collections::{HashMap};
use std::time::{Instant, Duration};
use parking_lot::RwLock;
use keychain::{
    SimpleSecretStore, SecretStore, Error as SSError, EthStore, EthMultiStore, random_string,
    StoreAccountRef, OpaqueSecretEd25519,
};
use keychain::accounts_dir::MemoryDirectory;
use keychain::ethkey::{Address, Ed25519Secret, generate_keypair, Message, Ed25519Signature};
use ajson::misc::AccountMeta;
pub use keychain::{Derivation, IndexDerivation, KeyFile};

/// Type of unlock.
#[derive(Clone, PartialEq)]
enum Unlock {
    /// If account is unlocked temporarily, it should be locked after first usage.
    OneTime,
    /// Account unlocked permanently can always sign message.
    /// Use with caution.
    Perm,
    /// Account unlocked with a timeout
    Timed(Instant),
}

/// Data associated with account.
#[derive(Clone)]
struct AccountData {
    unlock: Unlock,
    password: String,
}

/// Signing error
#[derive(Debug)]
pub enum SignError {
    /// Account is not unlocked
    NotUnlocked,
    /// Account does not exist.
    NotFound,
    /// Low-level error from store
    SStore(SSError),
    /// Inappropriate chain
    InappropriateChain,
}

impl fmt::Display for SignError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            SignError::NotUnlocked => write!(f, "Account is locked"),
            SignError::NotFound => write!(f, "Account does not exist"),
            SignError::SStore(ref e) => write!(f, "{}", e),
            SignError::InappropriateChain => write!(f, "Inappropriate chain"),
        }
    }
}

impl From<SSError> for SignError {
    fn from(e: SSError) -> Self { SignError::SStore(e) }
}

/// `AccountProvider` errors.
pub type Error = SSError;

fn transient_sstore() -> EthMultiStore {
    EthMultiStore::open(Box::new(MemoryDirectory::default()))
        .expect("MemoryDirectory load always succeeds; qed")
}

type AccountToken = String;

/// Account management.
/// Responsible for unlocking accounts.
pub struct AccountProvider {
    unlocked_secrets: RwLock<HashMap<StoreAccountRef, OpaqueSecretEd25519>>,
    /// Unlocked account data.
    unlocked: RwLock<HashMap<StoreAccountRef, AccountData>>,
    /// Accounts on disk
    sstore: Box<SecretStore>,
    /// Accounts unlocked with rolling tokens
    transient_sstore: EthMultiStore,
    /// When unlocking account permanently we additionally keep a raw secret in memory
    /// to increase the performance of transaction signing.
    unlock_keep_secret: bool,
    /// Disallowed accounts.
    blacklisted_accounts: Vec<Address>,
}

/// Account management settings.
pub struct AccountProviderSettings {
    /// Store raw account secret when unlocking the account permanently.
    pub unlock_keep_secret: bool,
    /// Disallowed accounts.
    pub blacklisted_accounts: Vec<Address>,
}

impl Default for AccountProviderSettings {
    fn default() -> Self {
        AccountProviderSettings {
            unlock_keep_secret: false,
            blacklisted_accounts: vec![],
        }
    }
}

impl AccountProvider {
    /// Creates new account provider.
    pub fn new(sstore: Box<SecretStore>, settings: AccountProviderSettings) -> Self {
        if let Ok(accounts) = sstore.accounts() {
            for account in accounts
                .into_iter()
                .filter(|a| settings.blacklisted_accounts.contains(&a.address))
            {
                warn!(
                    target: "account",
                    "Local Account {} has a blacklisted (known to be weak) address and will be \
                     ignored",
                    account.address
                );
            }
        }

        AccountProvider {
            unlocked_secrets: RwLock::new(HashMap::new()),
            unlocked: RwLock::new(HashMap::new()),
            sstore: sstore,
            transient_sstore: transient_sstore(),
            unlock_keep_secret: settings.unlock_keep_secret,
            blacklisted_accounts: settings.blacklisted_accounts,
        }
    }

    /// Creates not disk backed provider.
    pub fn transient_provider() -> Self {
        AccountProvider {
            unlocked_secrets: RwLock::new(HashMap::new()),
            unlocked: RwLock::new(HashMap::new()),
            sstore: Box::new(
                EthStore::open(Box::new(MemoryDirectory::default()))
                    .expect("MemoryDirectory load always succeeds; qed"),
            ),
            transient_sstore: transient_sstore(),
            unlock_keep_secret: false,
            blacklisted_accounts: vec![],
        }
    }

    /// Creates new random account.
    pub fn new_account_ed25519(&self, password: &str) -> Result<Address, Error> {
        let key_pair = generate_keypair();
        let secret = key_pair.secret().clone();
        let account = self.sstore.insert_account_ed25519(secret, password)?;
        Ok(account.address)
    }

    /// Inserts new account into underlying store.
    /// Does not unlock account!
    pub fn insert_account_ed25519(
        &self,
        secret: Ed25519Secret,
        password: &str,
    ) -> Result<Address, Error>
    {
        let account = self.sstore.insert_account_ed25519(secret, password)?;
        if self.blacklisted_accounts.contains(&account.address) {
            self.sstore.remove_account(&account, password)?;
            return Err(SSError::InvalidAccount.into());
        }
        Ok(account.address)
    }

    /// Checks whether an account with a given address is present.
    pub fn has_account(&self, address: Address) -> Result<bool, Error> {
        Ok(self.sstore.account_ref(&address).is_ok()
            && !self.blacklisted_accounts.contains(&address))
    }

    /// Returns addresses of all accounts.
    pub fn accounts(&self) -> Result<Vec<Address>, Error> {
        let accounts = self.sstore.accounts()?;
        Ok(accounts
            .into_iter()
            .map(|a| a.address)
            .filter(|address| !self.blacklisted_accounts.contains(address))
            .collect())
    }

    /// Inserts given address as first in the vector, preventing duplicates.
    fn _insert_default(mut addresses: Vec<Address>, default: Address) -> Vec<Address> {
        if let Some(position) = addresses.iter().position(|address| address == &default) {
            addresses.swap(0, position);
        } else {
            addresses.insert(0, default);
        }

        addresses
    }

    /// Returns each account along with name and meta.
    pub fn accounts_info(&self) -> Result<HashMap<Address, AccountMeta>, Error> {
        let r = self
            .sstore
            .accounts()?
            .into_iter()
            .filter(|a| !self.blacklisted_accounts.contains(&a.address))
            .map(|a| {
                (
                    a.address.clone(),
                    self.account_meta(a.address).ok().unwrap_or_default(),
                )
            })
            .collect();
        Ok(r)
    }

    /// Returns each account along with name and meta.
    pub fn account_meta(&self, address: Address) -> Result<AccountMeta, Error> {
        let account = self.sstore.account_ref(&address)?;
        Ok(AccountMeta {
            name: self.sstore.name(&account)?,
            meta: self.sstore.meta(&account)?,
            uuid: self.sstore.uuid(&account).ok().map(Into::into), // allowed to not have a Uuid
        })
    }

    /// Returns each account along with name and meta.
    pub fn set_account_name(&self, address: Address, name: String) -> Result<(), Error> {
        self.sstore
            .set_name(&self.sstore.account_ref(&address)?, name)?;
        Ok(())
    }

    /// Returns each account along with name and meta.
    pub fn set_account_meta(&self, address: Address, meta: String) -> Result<(), Error> {
        self.sstore
            .set_meta(&self.sstore.account_ref(&address)?, meta)?;
        Ok(())
    }

    /// Returns `true` if the password for `account` is `password`. `false` if not.
    pub fn test_password(&self, address: &Address, password: &str) -> Result<bool, Error> {
        self.sstore
            .test_password(&self.sstore.account_ref(&address)?, password)
            .map_err(Into::into)
    }

    /// Permanently removes an account.
    pub fn kill_account(&self, address: &Address, password: &str) -> Result<(), Error> {
        self.sstore
            .remove_account(&self.sstore.account_ref(&address)?, &password)?;
        Ok(())
    }

    /// Exports an account for given address.
    pub fn export_account(&self, address: &Address, password: String) -> Result<KeyFile, Error> {
        self.sstore
            .export_account(&self.sstore.account_ref(address)?, &password)
    }

    /// lock a sepcific account.
    pub fn lock_account(&self, address: Address, password: String) -> Result<(), Error> {
        let account = self.sstore.account_ref(&address)?;

        // remove stored secret if any
        let mut unlocked_secrets = self.unlocked_secrets.write();
        if unlocked_secrets.contains_key(&account) {
            unlocked_secrets.remove(&account);
        }

        // check if account is already locked, if it is, do nothing
        let mut unlocked = self.unlocked.write();
        if let None = unlocked.get(&account) {
            return Ok(());
        }

        // verify password by signing dump message
        // result may be discarded
        let _ = self
            .sstore
            .sign_ed25519(&account, &password, &Default::default())?;

        unlocked
            .remove(&account)
            .expect("data exists: so key must exist: qed");
        Ok(())
    }

    /// Unlock a sepcific account.
    fn unlock_account(
        &self,
        address: Address,
        password: String,
        unlock: Unlock,
    ) -> Result<(), Error>
    {
        let account = self.sstore.account_ref(&address)?;

        // check if account is already unlocked pernamently, if it is, do nothing
        let mut unlocked = self.unlocked.write();
        if let Some(data) = unlocked.get(&account) {
            if let Unlock::Perm = data.unlock {
                return Ok(());
            }
        }

        if self.unlock_keep_secret && unlock == Unlock::Perm {
            // verify password and get the secret
            let secret = self.sstore.raw_secret(&account, &password)?;
            self.unlocked_secrets
                .write()
                .insert(account.clone(), secret);
        } else {
            // verify password by signing dump message
            // result may be discarded
            //            let _ = self.sstore.sign_ed25519(&account, &password, &Default::default())?;
            let _ = self
                .sstore
                .sign_ed25519(&account, &password, &Default::default())?;
        }

        let data = AccountData {
            unlock: unlock,
            password: password,
        };

        unlocked.insert(account, data);
        Ok(())
    }

    fn update_unlock_time(&self, account: &StoreAccountRef) {
        let mut unlocked = self.unlocked.write();
        let data = unlocked.get(account).cloned();
        if data.is_some() {
            if let Unlock::Timed(ref end) = data.unwrap().unlock {
                if Instant::now() > *end {
                    unlocked
                        .remove(account)
                        .expect("data exists: so key must exist: qed");
                }
            }
        }
    }

    fn password(&self, account: &StoreAccountRef) -> Result<String, SignError> {
        let mut unlocked = self.unlocked.write();
        let data = unlocked.get(account).ok_or(SignError::NotUnlocked)?.clone();
        if let Unlock::OneTime = data.unlock {
            unlocked
                .remove(account)
                .expect("data exists: so key must exist: qed");
        }
        if let Unlock::Timed(ref end) = data.unlock {
            if Instant::now() > *end {
                unlocked
                    .remove(account)
                    .expect("data exists: so key must exist: qed");
                return Err(SignError::NotUnlocked);
            }
        }
        Ok(data.password.clone())
    }

    /// Unlocks account permanently.
    pub fn unlock_account_permanently(
        &self,
        account: Address,
        password: String,
    ) -> Result<(), Error>
    {
        self.unlock_account(account, password, Unlock::Perm)
    }

    /// Unlocks account temporarily (for one signing).
    pub fn unlock_account_temporarily(
        &self,
        account: Address,
        password: String,
    ) -> Result<(), Error>
    {
        self.unlock_account(account, password, Unlock::OneTime)
    }

    /// Unlocks account temporarily with a timeout.
    pub fn unlock_account_timed(
        &self,
        account: Address,
        password: String,
        duration_ms: u64,
    ) -> Result<(), Error>
    {
        self.unlock_account(
            account,
            password,
            Unlock::Timed(Instant::now() + Duration::from_millis(duration_ms)),
        )
    }

    /// Checks if given account is unlocked
    pub fn is_unlocked_generic(&self, address: &Address) -> bool {
        self.sstore
            .account_ref(address)
            .map(|r| {
                self.update_unlock_time(&r);
                let unlocked = self.unlocked.read();
                let unlocked_secrets = self.unlocked_secrets.read();
                unlocked.get(&r).is_some() || unlocked_secrets.get(&r).is_some()
            })
            .unwrap_or(false)
    }

    /// Checks if given account is unlocked permanently
    pub fn is_unlocked_permanently(&self, address: &Address) -> bool {
        let unlocked = self.unlocked.read();
        self.sstore
            .account_ref(address)
            .map(|r| {
                unlocked
                    .get(&r)
                    .map_or(false, |account| account.unlock == Unlock::Perm)
            })
            .unwrap_or(false)
    }

    /// Signs the message. If password is not provided the account must be unlocked.
    pub fn sign(
        &self,
        address: Address,
        password: Option<String>,
        message: Message,
    ) -> Result<Ed25519Signature, SignError>
    {
        let account = self.sstore.account_ref(&address)?;
        match self.unlocked_secrets.read().get(&account) {
            Some(secret) => Ok(self.sstore.sign_with_secret(&secret, &message)?),
            None => {
                let password = password
                    .map(Ok)
                    .unwrap_or_else(|| self.password(&account))?;
                Ok(self.sstore.sign_ed25519(&account, &password, &message)?)
            }
        }
    }

    /// Signs given message with supplied token. Returns a token to use in next signing within this session.
    pub fn sign_with_token(
        &self,
        address: Address,
        _token: AccountToken,
        message: Message,
    ) -> Result<(Ed25519Signature, AccountToken), SignError>
    {
        let account = self.sstore.account_ref(&address)?;
        let new_token = random_string(16);

        // and sign
        let signature = self
            .transient_sstore
            .sign_ed25519(&account, &new_token, &message)?;

        Ok((signature, new_token))
    }

    /// Create new vault.
    pub fn create_vault(&self, name: &str, password: &str) -> Result<(), Error> {
        self.sstore.create_vault(name, password).map_err(Into::into)
    }

    /// Open existing vault.
    pub fn open_vault(&self, name: &str, password: &str) -> Result<(), Error> {
        self.sstore.open_vault(name, password).map_err(Into::into)
    }

    /// Close previously opened vault.
    pub fn close_vault(&self, name: &str) -> Result<(), Error> {
        self.sstore.close_vault(name).map_err(Into::into)
    }

    /// List all vaults
    pub fn list_vaults(&self) -> Result<Vec<String>, Error> {
        self.sstore.list_vaults().map_err(Into::into)
    }

    /// List all currently opened vaults
    pub fn list_opened_vaults(&self) -> Result<Vec<String>, Error> {
        self.sstore.list_opened_vaults().map_err(Into::into)
    }

    /// Change vault password.
    pub fn change_vault_password(&self, name: &str, new_password: &str) -> Result<(), Error> {
        self.sstore
            .change_vault_password(name, new_password)
            .map_err(Into::into)
    }

    /// Get vault metadata string.
    pub fn get_vault_meta(&self, name: &str) -> Result<String, Error> {
        self.sstore.get_vault_meta(name).map_err(Into::into)
    }

    /// Set vault metadata string.
    pub fn set_vault_meta(&self, name: &str, meta: &str) -> Result<(), Error> {
        self.sstore.set_vault_meta(name, meta).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::{AccountProvider, Unlock};
    use std::time::Instant;
    use keychain::ethkey::{generate_keypair, Address};
    use keychain::StoreAccountRef;

    #[test]
    fn unlock_account_temp() {
        let kp = generate_keypair();
        let ap = AccountProvider::transient_provider();
        assert!(
            ap.insert_account_ed25519(kp.secret().clone(), "test")
                .is_ok()
        );
        assert!(
            ap.unlock_account_temporarily(kp.address(), "test1".into())
                .is_err()
        );
        assert!(
            ap.unlock_account_temporarily(kp.address(), "test".into())
                .is_ok()
        );
        assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
        assert!(ap.sign(kp.address(), None, Default::default()).is_err());
    }

    #[test]
    fn unlock_account_perm() {
        let kp = generate_keypair();
        let ap = AccountProvider::transient_provider();
        assert!(
            ap.insert_account_ed25519(kp.secret().clone(), "test")
                .is_ok()
        );
        assert!(
            ap.unlock_account_permanently(kp.address(), "test1".into())
                .is_err()
        );
        assert!(
            ap.unlock_account_permanently(kp.address(), "test".into())
                .is_ok()
        );
        assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
        assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
        assert!(
            ap.unlock_account_temporarily(kp.address(), "test".into())
                .is_ok()
        );
        assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
        assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
    }

    #[test]
    fn unlock_account_timer() {
        let kp = generate_keypair();
        let ap = AccountProvider::transient_provider();
        assert!(
            ap.insert_account_ed25519(kp.secret().clone(), "test")
                .is_ok()
        );
        assert!(
            ap.unlock_account_timed(kp.address(), "test1".into(), 60000)
                .is_err()
        );
        assert!(
            ap.unlock_account_timed(kp.address(), "test".into(), 60000)
                .is_ok()
        );
        assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
        ap.unlocked
            .write()
            .get_mut(&StoreAccountRef::new(kp.address()))
            .unwrap()
            .unlock = Unlock::Timed(Instant::now());
        assert!(ap.sign(kp.address(), None, Default::default()).is_err());
    }

    #[test]
    fn should_not_return_blacklisted_account() {
        // given
        let mut ap = AccountProvider::transient_provider();
        let acc = ap.new_account_ed25519("test").unwrap();
        ap.blacklisted_accounts = vec![acc];

        // then
        assert_eq!(
            ap.accounts_info()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<Address>>(),
            vec![]
        );
        assert_eq!(ap.accounts().unwrap(), vec![]);
    }
}
