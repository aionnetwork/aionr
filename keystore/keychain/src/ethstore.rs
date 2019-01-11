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

use std::collections::{BTreeMap, HashMap};
use std::mem;
use std::path::PathBuf;
use parking_lot::{Mutex, RwLock};
use std::time::{Instant, Duration};

use crypto::KEY_ITERATIONS;
use random::Random;
use ethkey::{Ed25519KeyPair, Ed25519Secret, Ed25519Signature, Address, Message};
use accounts_dir::{KeyDirectory, VaultKeyDirectory, VaultKey, SetKeyError};
use account::SafeAccount;
use json::{Uuid, OpaqueKeyFile};
use {Error, SimpleSecretStore, SecretStore, StoreAccountRef, OpaqueSecretEd25519};

/// Accounts store.
pub struct EthStore {
    store: EthMultiStore,
}

impl EthStore {
    /// Open a new accounts store with given key directory backend.
    pub fn open(directory: Box<KeyDirectory>) -> Result<Self, Error> {
        Self::open_with_iterations(directory, KEY_ITERATIONS as u32)
    }

    /// Open a new account store with given key directory backend and custom number of iterations.
    pub fn open_with_iterations(
        directory: Box<KeyDirectory>,
        iterations: u32,
    ) -> Result<Self, Error>
    {
        Ok(EthStore {
            store: EthMultiStore::open_with_iterations(directory, iterations)?,
        })
    }

    /// Modify account refresh timeout - how often they are re-read from `KeyDirectory`.
    ///
    /// Setting this to low values (or 0) will cause new accounts to be picked up quickly,
    /// although it may induce heavy disk reads and is not recommended if you manage many keys (say over 10k).
    ///
    /// By default refreshing is disabled, so only accounts created using this instance of `EthStore` are taken into account.
    pub fn set_refresh_time(&self, time: Duration) { self.store.set_refresh_time(time) }

    /// get account by address
    pub fn get(&self, account: &StoreAccountRef) -> Result<SafeAccount, Error> {
        let mut accounts = self.store.get_accounts(account)?.into_iter();
        accounts.next().ok_or(Error::InvalidAccount)
    }
}

impl SimpleSecretStore for EthStore {
    fn insert_account_ed25519(
        &self,
        secret: Ed25519Secret,
        password: &str,
    ) -> Result<StoreAccountRef, Error>
    {
        let keypair = Ed25519KeyPair::from_secret(secret).map_err(|_| Error::CreationFailed)?;
        let id: [u8; 16] = Random::random();
        let account = SafeAccount::create_ed25519(
            &keypair,
            id,
            password,
            self.store.iterations,
            "".to_owned(),
            "{}".to_owned(),
        );
        self.store.import(account)
    }

    fn account_ref(&self, address: &Address) -> Result<StoreAccountRef, Error> {
        self.store.account_ref(address)
    }

    fn accounts(&self) -> Result<Vec<StoreAccountRef>, Error> { self.store.accounts() }

    fn export_account(
        &self,
        account: &StoreAccountRef,
        password: &str,
    ) -> Result<OpaqueKeyFile, Error>
    {
        self.store.export_account(account, password)
    }

    fn remove_account(&self, account: &StoreAccountRef, password: &str) -> Result<(), Error> {
        self.store.remove_account(account, password)
    }

    fn sign_ed25519(
        &self,
        account: &StoreAccountRef,
        password: &str,
        message: &Message,
    ) -> Result<Ed25519Signature, Error>
    {
        self.get(account)?.sign(password, message)
    }

    fn create_vault(&self, name: &str, password: &str) -> Result<(), Error> {
        self.store.create_vault(name, password)
    }

    fn open_vault(&self, name: &str, password: &str) -> Result<(), Error> {
        self.store.open_vault(name, password)
    }

    fn close_vault(&self, name: &str) -> Result<(), Error> { self.store.close_vault(name) }

    fn list_vaults(&self) -> Result<Vec<String>, Error> { self.store.list_vaults() }

    fn list_opened_vaults(&self) -> Result<Vec<String>, Error> { self.store.list_opened_vaults() }

    fn change_vault_password(&self, name: &str, new_password: &str) -> Result<(), Error> {
        self.store.change_vault_password(name, new_password)
    }

    fn change_account_vault(&self, account: StoreAccountRef) -> Result<StoreAccountRef, Error> {
        self.store.change_account_vault(account)
    }

    fn get_vault_meta(&self, name: &str) -> Result<String, Error> {
        self.store.get_vault_meta(name)
    }

    fn set_vault_meta(&self, name: &str, meta: &str) -> Result<(), Error> {
        self.store.set_vault_meta(name, meta)
    }
}

impl SecretStore for EthStore {
    fn raw_secret(
        &self,
        account: &StoreAccountRef,
        password: &str,
    ) -> Result<OpaqueSecretEd25519, Error>
    {
        Ok(OpaqueSecretEd25519(
            self.get(account)?.crypto.secret_ed25519(password)?,
        ))
    }

    fn test_password(&self, account: &StoreAccountRef, password: &str) -> Result<bool, Error> {
        let account = self.get(account)?;
        Ok(account.check_password(password))
    }

    fn uuid(&self, account: &StoreAccountRef) -> Result<Uuid, Error> {
        let account = self.get(account)?;
        Ok(account.id.as_bytes().clone().into())
    }

    fn name(&self, account: &StoreAccountRef) -> Result<String, Error> {
        let account = self.get(account)?;
        Ok(account.name.clone())
    }

    fn meta(&self, account: &StoreAccountRef) -> Result<String, Error> {
        let account = self.get(account)?;
        Ok(account.meta.clone())
    }

    fn set_name(&self, account_ref: &StoreAccountRef, name: String) -> Result<(), Error> {
        let old = self.get(account_ref)?;
        let mut safe_account = old.clone();
        safe_account.name = name;

        // save to file
        self.store.update(account_ref, old, safe_account)
    }

    fn set_meta(&self, account_ref: &StoreAccountRef, meta: String) -> Result<(), Error> {
        let old = self.get(account_ref)?;
        let mut safe_account = old.clone();
        safe_account.meta = meta;

        // save to file
        self.store.update(account_ref, old, safe_account)
    }

    fn local_path(&self) -> PathBuf { self.store.dir.path().cloned().unwrap_or_else(PathBuf::new) }
}

/// Similar to `EthStore` but may store many accounts (with different passwords) for the same `Address`
pub struct EthMultiStore {
    dir: Box<KeyDirectory>,
    iterations: u32,
    // order lock: cache, then vaults
    cache: RwLock<BTreeMap<StoreAccountRef, Vec<SafeAccount>>>,
    vaults: Mutex<HashMap<String, Box<VaultKeyDirectory>>>,
    timestamp: Mutex<Timestamp>,
}

struct Timestamp {
    dir_hash: Option<u64>,
    last_checked: Instant,
    refresh_time: Duration,
}

impl EthMultiStore {
    /// Open new multi-accounts store with given key directory backend.
    pub fn open(directory: Box<KeyDirectory>) -> Result<Self, Error> {
        Self::open_with_iterations(directory, KEY_ITERATIONS as u32)
    }

    /// Open new multi-accounts store with given key directory backend and custom number of iterations for new keys.
    pub fn open_with_iterations(
        directory: Box<KeyDirectory>,
        iterations: u32,
    ) -> Result<Self, Error>
    {
        let store = EthMultiStore {
            dir: directory,
            vaults: Mutex::new(HashMap::new()),
            iterations,
            cache: Default::default(),
            timestamp: Mutex::new(Timestamp {
                dir_hash: None,
                last_checked: Instant::now(),
                // by default we never refresh accounts
                refresh_time: Duration::from_secs(u64::max_value()),
            }),
        };
        store.reload_accounts()?;
        Ok(store)
    }

    /// Modify account refresh timeout - how often they are re-read from `KeyDirectory`.
    ///
    /// Setting this to low values (or 0) will cause new accounts to be picked up quickly,
    /// although it may induce heavy disk reads and is not recommended if you manage many keys (say over 10k).
    ///
    /// By default refreshing is disabled, so only accounts created using this instance of `EthStore` are taken into account.
    pub fn set_refresh_time(&self, time: Duration) { self.timestamp.lock().refresh_time = time; }

    fn reload_if_changed(&self) -> Result<(), Error> {
        let mut last_timestamp = self.timestamp.lock();
        let now = Instant::now();
        if now - last_timestamp.last_checked > last_timestamp.refresh_time {
            let dir_hash = Some(self.dir.unique_repr()?);
            last_timestamp.last_checked = now;
            if last_timestamp.dir_hash == dir_hash {
                return Ok(());
            }
            self.reload_accounts()?;
            last_timestamp.dir_hash = dir_hash;
        }
        Ok(())
    }

    fn reload_accounts(&self) -> Result<(), Error> {
        let mut cache = self.cache.write();

        let mut new_accounts = BTreeMap::new();
        for account in self.dir.load()? {
            let account_ref = StoreAccountRef::new(account.address);
            new_accounts
                .entry(account_ref)
                .or_insert_with(Vec::new)
                .push(account);
        }
        mem::replace(&mut *cache, new_accounts);
        Ok(())
    }

    fn get_accounts(&self, account: &StoreAccountRef) -> Result<Vec<SafeAccount>, Error> {
        let from_cache = |account| {
            let cache = self.cache.read();
            if let Some(accounts) = cache.get(account) {
                if !accounts.is_empty() {
                    return Some(accounts.clone());
                }
            }

            None
        };

        match from_cache(account) {
            Some(accounts) => Ok(accounts),
            None => {
                self.reload_if_changed()?;
                from_cache(account).ok_or(Error::InvalidAccount)
            }
        }
    }

    fn get_matching(
        &self,
        account: &StoreAccountRef,
        password: &str,
    ) -> Result<Vec<SafeAccount>, Error>
    {
        let accounts = self.get_accounts(account)?;

        Ok(accounts
            .into_iter()
            .filter(|acc| acc.check_password(password))
            .collect())
    }

    fn import(&self, account: SafeAccount) -> Result<StoreAccountRef, Error> {
        // save to file
        let account = self.dir.insert(account)?;

        // update cache
        let account_ref = StoreAccountRef::new(account.address.clone());
        let mut cache = self.cache.write();
        cache
            .entry(account_ref.clone())
            .or_insert_with(Vec::new)
            .push(account);

        Ok(account_ref)
    }

    fn update(
        &self,
        account_ref: &StoreAccountRef,
        old: SafeAccount,
        new: SafeAccount,
    ) -> Result<(), Error>
    {
        // save to file
        let account = self.dir.update(new)?;

        // update cache
        let mut cache = self.cache.write();
        let accounts = cache.entry(account_ref.clone()).or_insert_with(Vec::new);
        // Remove old account
        accounts.retain(|acc| acc != &old);
        // And push updated to the end
        accounts.push(account);
        Ok(())
    }

    fn remove_safe_account(
        &self,
        account_ref: &StoreAccountRef,
        account: &SafeAccount,
    ) -> Result<(), Error>
    {
        // Remove from dir
        self.dir.remove(&account)?;

        // Remove from cache
        let mut cache = self.cache.write();
        let is_empty = {
            if let Some(accounts) = cache.get_mut(account_ref) {
                if let Some(position) = accounts.iter().position(|acc| acc == account) {
                    accounts.remove(position);
                }
                accounts.is_empty()
            } else {
                false
            }
        };

        if is_empty {
            cache.remove(account_ref);
        }

        return Ok(());
    }
}

impl SimpleSecretStore for EthMultiStore {
    fn insert_account_ed25519(
        &self,
        _secret: Ed25519Secret,
        _password: &str,
    ) -> Result<StoreAccountRef, Error>
    {
        unimplemented!()
    }

    fn account_ref(&self, address: &Address) -> Result<StoreAccountRef, Error> {
        let read_from_cache = |address: &Address| {
            use std::collections::Bound;
            let cache = self.cache.read();
            let mut r = cache.range((Bound::Included(*address), Bound::Included(*address)));
            r.next().map(|(k, _)| k.clone())
        };

        match read_from_cache(address) {
            Some(account) => Ok(account),
            None => {
                self.reload_if_changed()?;
                read_from_cache(address).ok_or(Error::InvalidAccount)
            }
        }
    }

    fn accounts(&self) -> Result<Vec<StoreAccountRef>, Error> {
        self.reload_if_changed()?;
        Ok(self.cache.read().keys().cloned().collect())
    }

    fn remove_account(&self, account_ref: &StoreAccountRef, password: &str) -> Result<(), Error> {
        let accounts = self.get_matching(account_ref, password)?;

        for account in accounts {
            return self.remove_safe_account(account_ref, &account);
        }

        Err(Error::InvalidPassword)
    }

    fn export_account(
        &self,
        account_ref: &StoreAccountRef,
        password: &str,
    ) -> Result<OpaqueKeyFile, Error>
    {
        self.get_matching(account_ref, password)?
            .into_iter()
            .nth(0)
            .map(Into::into)
            .ok_or(Error::InvalidPassword)
    }

    fn sign_ed25519(
        &self,
        account: &StoreAccountRef,
        password: &str,
        message: &Message,
    ) -> Result<Ed25519Signature, Error>
    {
        let accounts = self.get_matching(account, password)?;
        match accounts.first() {
            Some(ref account) => account.sign(password, message),
            None => Err(Error::InvalidPassword),
        }
    }

    fn create_vault(&self, name: &str, password: &str) -> Result<(), Error> {
        let is_vault_created = {
            // lock border
            let mut vaults = self.vaults.lock();
            if !vaults.contains_key(&name.to_owned()) {
                let vault_provider = self
                    .dir
                    .as_vault_provider()
                    .ok_or(Error::VaultsAreNotSupported)?;
                let vault =
                    vault_provider.create(name, VaultKey::new(password, self.iterations))?;
                vaults.insert(name.to_owned(), vault);
                true
            } else {
                false
            }
        };

        if is_vault_created {
            self.reload_accounts()?;
        }

        Ok(())
    }

    fn open_vault(&self, name: &str, password: &str) -> Result<(), Error> {
        let is_vault_opened = {
            // lock border
            let mut vaults = self.vaults.lock();
            if !vaults.contains_key(&name.to_owned()) {
                let vault_provider = self
                    .dir
                    .as_vault_provider()
                    .ok_or(Error::VaultsAreNotSupported)?;
                let vault = vault_provider.open(name, VaultKey::new(password, self.iterations))?;
                vaults.insert(name.to_owned(), vault);
                true
            } else {
                false
            }
        };

        if is_vault_opened {
            self.reload_accounts()?;
        }

        Ok(())
    }

    fn close_vault(&self, name: &str) -> Result<(), Error> {
        let is_vault_removed = self.vaults.lock().remove(&name.to_owned()).is_some();
        if is_vault_removed {
            self.reload_accounts()?;
        }
        Ok(())
    }

    fn list_vaults(&self) -> Result<Vec<String>, Error> {
        let vault_provider = self
            .dir
            .as_vault_provider()
            .ok_or(Error::VaultsAreNotSupported)?;
        vault_provider.list_vaults()
    }

    fn list_opened_vaults(&self) -> Result<Vec<String>, Error> {
        Ok(self.vaults.lock().keys().cloned().collect())
    }

    fn change_vault_password(&self, name: &str, new_password: &str) -> Result<(), Error> {
        let old_key = self
            .vaults
            .lock()
            .get(name)
            .map(|v| v.key())
            .ok_or(Error::VaultNotFound)?;
        let vault_provider = self
            .dir
            .as_vault_provider()
            .ok_or(Error::VaultsAreNotSupported)?;
        let vault = vault_provider.open(name, old_key)?;
        match vault.set_key(VaultKey::new(new_password, self.iterations)) {
            Ok(_) => {
                self.close_vault(name)
                    .and_then(|_| self.open_vault(name, new_password))
            }
            Err(SetKeyError::Fatal(err)) => {
                let _ = self.close_vault(name);
                Err(err)
            }
            Err(SetKeyError::NonFatalNew(err)) => {
                let _ = self
                    .close_vault(name)
                    .and_then(|_| self.open_vault(name, new_password));
                Err(err)
            }
            Err(SetKeyError::NonFatalOld(err)) => Err(err),
        }
    }

    fn change_account_vault(&self, account_ref: StoreAccountRef) -> Result<StoreAccountRef, Error> {
        Ok(account_ref)
    }

    fn get_vault_meta(&self, name: &str) -> Result<String, Error> {
        // vault meta contains password hint
        // => allow reading meta even if vault is not yet opened
        self.vaults
            .lock()
            .get(name)
            .and_then(|v| Some(v.meta()))
            .ok_or(Error::VaultNotFound)
            .or_else(|_| {
                let vault_provider = self
                    .dir
                    .as_vault_provider()
                    .ok_or(Error::VaultsAreNotSupported)?;
                vault_provider.vault_meta(name)
            })
    }

    fn set_vault_meta(&self, name: &str, meta: &str) -> Result<(), Error> {
        self.vaults
            .lock()
            .get(name)
            .ok_or(Error::VaultNotFound)
            .and_then(|v| v.set_meta(meta))
    }
}

#[cfg(test)]
mod tests {
    extern crate tempdir;
    use accounts_dir::{
        KeyDirectory,
        MemoryDirectory,
        RootDiskDirectory
};
    use key::{generate_keypair, Ed25519KeyPair};
    use secret_store::{
        SimpleSecretStore,
        SecretStore,
};
    use super::EthStore;
    use self::tempdir::TempDir;

    fn keypair() -> Ed25519KeyPair { generate_keypair() }

    fn store() -> EthStore {
        EthStore::open(Box::new(MemoryDirectory::default()))
            .expect("MemoryDirectory always load successfuly; qed")
    }

    struct RootDiskDirectoryGuard {
        pub key_dir: Option<Box<KeyDirectory>>,
        _path: TempDir,
    }

    impl RootDiskDirectoryGuard {
        pub fn new() -> Self {
            let temp_path = TempDir::new("").unwrap();
            let disk_dir = Box::new(RootDiskDirectory::create(temp_path.path()).unwrap());

            RootDiskDirectoryGuard {
                key_dir: Some(disk_dir),
                _path: temp_path,
            }
        }
    }

    #[test]
    fn should_insert_account_successfully() {
        // given
        let store = store();
        let keypair = keypair();

        // when
        let address = store
            .insert_account_ed25519(keypair.secret().clone(), "test")
            .unwrap();

        // then
        //assert_eq!(address, StoreAccountRef::root(keypair.address()));
        assert!(store.get(&address).is_ok(), "Should contain account.");
        assert_eq!(
            store.accounts().unwrap().len(),
            1,
            "Should have one account."
        );
    }

    #[test]
    fn should_update_meta_and_name() {
        // given
        let store = store();
        let keypair = keypair();
        let address = store
            .insert_account_ed25519(keypair.secret().clone(), "test")
            .unwrap();
        assert_eq!(&store.meta(&address).unwrap(), "{}");
        assert_eq!(&store.name(&address).unwrap(), "");

        // when
        store.set_meta(&address, "meta".into()).unwrap();
        store.set_name(&address, "name".into()).unwrap();

        // then
        assert_eq!(&store.meta(&address).unwrap(), "meta");
        assert_eq!(&store.name(&address).unwrap(), "name");
        assert_eq!(store.accounts().unwrap().len(), 1);
    }

    #[test]
    fn should_remove_account() {
        // given
        let store = store();
        let keypair = keypair();
        let address = store
            .insert_account_ed25519(keypair.secret().clone(), "test")
            .unwrap();

        // when
        store.remove_account(&address, "test").unwrap();

        // then
        assert_eq!(store.accounts().unwrap().len(), 0, "Should remove account.");
    }

    #[test]
    fn should_return_true_if_password_is_correct() {
        // given
        let store = store();
        let keypair = keypair();
        let address = store
            .insert_account_ed25519(keypair.secret().clone(), "test")
            .unwrap();

        // when
        let res1 = store.test_password(&address, "x").unwrap();
        let res2 = store.test_password(&address, "test").unwrap();

        assert!(!res1, "First password should be invalid.");
        assert!(res2, "Second password should be correct.");
    }

    #[test]
    fn should_not_remove_account_when_moving_to_self() {
        // given
        let mut dir = RootDiskDirectoryGuard::new();
        let store = EthStore::open(dir.key_dir.take().unwrap()).unwrap();
        let password1 = "password1";
        let keypair1 = keypair();

        // when
        let account1 = store
            .insert_account_ed25519(keypair1.secret().clone(), password1)
            .unwrap();
        store.change_account_vault(account1).unwrap();

        // then
        let accounts = store.accounts().unwrap();
        assert_eq!(accounts.len(), 1);
    }

    #[test]
    fn should_list_opened_vaults() {
        // given
        let mut dir = RootDiskDirectoryGuard::new();
        let store = EthStore::open(dir.key_dir.take().unwrap()).unwrap();
        let name1 = "vault1";
        let password1 = "password1";
        let name2 = "vault2";
        let password2 = "password2";
        let name3 = "vault3";
        let password3 = "password3";

        // when
        store.create_vault(name1, password1).unwrap();
        store.create_vault(name2, password2).unwrap();
        store.create_vault(name3, password3).unwrap();
        store.close_vault(name2).unwrap();

        // then
        let opened_vaults = store.list_opened_vaults().unwrap();
        assert_eq!(opened_vaults.len(), 2);
        assert!(opened_vaults.iter().any(|v| &*v == name1));
        assert!(opened_vaults.iter().any(|v| &*v == name3));
    }

    #[test]
    fn should_manage_vaults_meta() {
        // given
        let mut dir = RootDiskDirectoryGuard::new();
        let store = EthStore::open(dir.key_dir.take().unwrap()).unwrap();
        let name1 = "vault1";
        let password1 = "password1";

        // when
        store.create_vault(name1, password1).unwrap();

        // then
        assert_eq!(store.get_vault_meta(name1).unwrap(), "{}".to_owned());
        assert!(store.set_vault_meta(name1, "Hello, world!!!").is_ok());
        assert_eq!(
            store.get_vault_meta(name1).unwrap(),
            "Hello, world!!!".to_owned()
        );

        // and when
        store.close_vault(name1).unwrap();
        store.open_vault(name1, password1).unwrap();

        // then
        assert_eq!(
            store.get_vault_meta(name1).unwrap(),
            "Hello, world!!!".to_owned()
        );

        // and when
        store.close_vault(name1).unwrap();

        // then
        assert_eq!(
            store.get_vault_meta(name1).unwrap(),
            "Hello, world!!!".to_owned()
        );
        assert!(store.get_vault_meta("vault2").is_err());
    }

    #[test]
    fn should_save_meta_when_setting_before_password() {
        // given
        let mut dir = RootDiskDirectoryGuard::new();
        let store = EthStore::open(dir.key_dir.take().unwrap()).unwrap();
        let name = "vault";
        let password = "password1";
        let new_password = "password2";

        // when
        store.create_vault(name, password).unwrap();
        store.set_vault_meta(name, "OldMeta").unwrap();
        store.change_vault_password(name, new_password).unwrap();

        // then
        assert_eq!(store.get_vault_meta(name).unwrap(), "OldMeta".to_owned());
    }

    #[test]
    fn should_export_account() {
        // given
        let store = store();
        let keypair = generate_keypair();
        let address = store
            .insert_account_ed25519(keypair.secret().clone(), "test")
            .unwrap();

        // when
        let exported = store.export_account(&address, "test");

        // then
        assert!(
            exported.is_ok(),
            "Should export single account: {:?}",
            exported
        );
    }
}
