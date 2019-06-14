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

use std::{fs, io};
use std::path::{PathBuf, Path};
use parking_lot::Mutex;
use {json, SafeAccount, Error};
use blake2b::blake2b;
use super::super::account::Crypto;
use super::{KeyDirectory, VaultKeyDirectory, VaultKey, SetKeyError};
use super::disk::{DiskDirectory, KeyFileManager};

/// Name of vault metadata file
pub const VAULT_FILE_NAME: &'static str = "vault.json";
/// Name of temporary vault metadata file
pub const VAULT_TEMP_FILE_NAME: &'static str = "vault_temp.json";

/// Vault directory implementation
pub type VaultDiskDirectory = DiskDirectory<VaultKeyFileManager>;

/// Vault key file manager
pub struct VaultKeyFileManager {
    name: String,
    key: VaultKey,
    meta: Mutex<String>,
}

impl VaultDiskDirectory {
    /// Create new vault directory with given key
    pub fn create<P>(root: P, name: &str, key: VaultKey) -> Result<Self, Error>
    where P: AsRef<Path> {
        // check that vault directory does not exists
        let vault_dir_path = make_vault_dir_path(root, name, true)?;
        if vault_dir_path.exists() {
            return Err(Error::CreationFailed);
        }

        // create vault && vault file
        let vault_meta = "{}";
        fs::create_dir_all(&vault_dir_path)?;
        if let Err(err) = create_vault_file(&vault_dir_path, &key, vault_meta) {
            let _ = fs::remove_dir_all(&vault_dir_path); // can't do anything with this
            return Err(err);
        }

        Ok(DiskDirectory::new(
            vault_dir_path,
            VaultKeyFileManager::new(name, key, vault_meta),
        ))
    }

    /// Open existing vault directory with given key
    pub fn at<P>(root: P, name: &str, key: VaultKey) -> Result<Self, Error>
    where P: AsRef<Path> {
        // check that vault directory exists
        let vault_dir_path = make_vault_dir_path(root, name, true)?;
        if !vault_dir_path.is_dir() {
            return Err(Error::CreationFailed);
        }

        // check that passed key matches vault file
        let meta = read_vault_file(&vault_dir_path, Some(&key))?;

        Ok(DiskDirectory::new(
            vault_dir_path,
            VaultKeyFileManager::new(name, key, &meta),
        ))
    }

    /// Read vault meta without actually opening the vault
    pub fn meta_at<P>(root: P, name: &str) -> Result<String, Error>
    where P: AsRef<Path> {
        // check that vault directory exists
        let vault_dir_path = make_vault_dir_path(root, name, true)?;
        if !vault_dir_path.is_dir() {
            return Err(Error::VaultNotFound);
        }

        // check that passed key matches vault file
        read_vault_file(&vault_dir_path, None)
    }

    fn create_temp_vault(&self, key: VaultKey) -> Result<VaultDiskDirectory, Error> {
        let original_path = self
            .path()
            .expect("self is instance of DiskDirectory; DiskDirectory always returns path; qed");
        let mut path: PathBuf = original_path.clone();
        let name = self.name();

        path.push(name); // to jump to the next level

        let mut index = 0;
        loop {
            let name = format!("{}_temp_{}", name, index);
            path.set_file_name(&name);
            if !path.exists() {
                return VaultDiskDirectory::create(original_path, &name, key);
            }

            index += 1;
        }
    }

    fn copy_to_vault(&self, vault: &VaultDiskDirectory) -> Result<(), Error> {
        for account in self.load()? {
            let filename = account.filename.clone().expect(
                "self is instance of DiskDirectory; DiskDirectory fills filename in load; qed",
            );
            vault.insert_with_filename(account, filename, true)?;
        }

        Ok(())
    }

    fn delete(&self) -> Result<(), Error> {
        let path = self
            .path()
            .expect("self is instance of DiskDirectory; DiskDirectory always returns path; qed");
        fs::remove_dir_all(path).map_err(Into::into)
    }
}

impl VaultKeyDirectory for VaultDiskDirectory {
    fn as_key_directory(&self) -> &KeyDirectory { self }

    fn name(&self) -> &str { &self.key_manager().name }

    fn key(&self) -> VaultKey { self.key_manager().key.clone() }

    fn set_key(&self, new_key: VaultKey) -> Result<(), SetKeyError> {
        let temp_vault = VaultDiskDirectory::create_temp_vault(self, new_key.clone())
            .map_err(|err| SetKeyError::NonFatalOld(err))?;
        let mut source_path = temp_vault
            .path()
            .expect(
                "temp_vault is instance of DiskDirectory; DiskDirectory always returns path; qed",
            )
            .clone();
        let mut target_path = self
            .path()
            .expect("self is instance of DiskDirectory; DiskDirectory always returns path; qed")
            .clone();

        // preserve meta
        temp_vault
            .set_meta(&self.meta())
            .map_err(SetKeyError::NonFatalOld)?;

        // jump to next fs level
        source_path.push("next");
        target_path.push("next");

        let temp_accounts = self
            .copy_to_vault(&temp_vault)
            .and_then(|_| temp_vault.load())
            .map_err(|err| {
                // ignore error, as we already processing error
                let _ = temp_vault.delete();
                SetKeyError::NonFatalOld(err)
            })?;

        // we can't just delete temp vault until all files moved, because
        // original vault content has already been partially replaced
        // => when error or crash happens here, we can't do anything
        for temp_account in temp_accounts {
            let filename = temp_account.filename.expect(
                "self is instance of DiskDirectory; DiskDirectory fills filename in load; qed",
            );
            source_path.set_file_name(&filename);
            target_path.set_file_name(&filename);
            fs::rename(&source_path, &target_path).map_err(|err| SetKeyError::Fatal(err.into()))?;
        }
        source_path.set_file_name(VAULT_FILE_NAME);
        target_path.set_file_name(VAULT_FILE_NAME);
        fs::rename(source_path, target_path).map_err(|err| SetKeyError::Fatal(err.into()))?;

        temp_vault
            .delete()
            .map_err(|err| SetKeyError::NonFatalNew(err))
    }

    fn meta(&self) -> String { self.key_manager().meta.lock().clone() }

    fn set_meta(&self, meta: &str) -> Result<(), Error> {
        let key_manager = self.key_manager();
        let vault_path = self
            .path()
            .expect("self is instance of DiskDirectory; DiskDirectory always returns path; qed");
        create_vault_file(vault_path, &key_manager.key, meta)?;
        *key_manager.meta.lock() = meta.to_owned();
        Ok(())
    }
}

impl VaultKeyFileManager {
    pub fn new(name: &str, key: VaultKey, meta: &str) -> Self {
        VaultKeyFileManager {
            name: name.into(),
            key: key,
            meta: Mutex::new(meta.to_owned()),
        }
    }
}

impl KeyFileManager for VaultKeyFileManager {
    fn read<T>(&self, filename: Option<String>, reader: T) -> Result<SafeAccount, Error>
    where T: io::Read {
        let vault_file =
            json::VaultKeyFile::load(reader).map_err(|e| Error::Custom(format!("{:?}", e)))?;
        let mut safe_account =
            SafeAccount::from_vault_file(&self.key.password, vault_file, filename.clone())?;

        safe_account.meta = json::insert_vault_name_to_json_meta(&safe_account.meta, &self.name)
            .map_err(|err| Error::Custom(format!("{:?}", err)))?;
        Ok(safe_account)
    }

    fn write<T>(&self, mut account: SafeAccount, writer: &mut T) -> Result<(), Error>
    where T: io::Write {
        account.meta = json::remove_vault_name_from_json_meta(&account.meta)
            .map_err(|err| Error::Custom(format!("{:?}", err)))?;

        let vault_file: json::VaultKeyFile =
            account.into_vault_file(self.key.iterations, &self.key.password)?;
        vault_file
            .write(writer)
            .map_err(|e| Error::Custom(format!("{:?}", e)))
    }

    fn read_encoded<T>(&self, _reader: &mut T) -> Result<SafeAccount, Error>
    where T: io::Read {
        unimplemented!()
    }

    fn write_encoded<T>(&self, _account: SafeAccount, _writer: &mut T) -> Result<(), Error>
    where T: io::Write {
        unimplemented!()
    }
}

/// Makes path to vault directory, checking that vault name is appropriate
fn make_vault_dir_path<P>(root: P, name: &str, check_name: bool) -> Result<PathBuf, Error>
where P: AsRef<Path> {
    // check vault name
    if check_name && !check_vault_name(name) {
        return Err(Error::InvalidVaultName);
    }

    let mut vault_dir_path: PathBuf = root.as_ref().into();
    vault_dir_path.push(name);
    Ok(vault_dir_path)
}

/// Every vault must have unique name => we rely on filesystem to check this
/// => vault name must not contain any fs-special characters to avoid directory traversal
/// => we only allow alphanumeric + separator characters in vault name.
fn check_vault_name(name: &str) -> bool {
    !name.is_empty() && name
        .chars()
        .all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '-' || c == '_')
}

/// Vault can be empty, but still must be pluggable => we store vault password in separate file
fn create_vault_file<P>(vault_dir_path: P, key: &VaultKey, meta: &str) -> Result<(), Error>
where P: AsRef<Path> {
    let password_hash = blake2b(&key.password);
    let crypto = Crypto::with_plain(&password_hash, &key.password, key.iterations);

    let mut vault_file_path: PathBuf = vault_dir_path.as_ref().into();
    vault_file_path.push(VAULT_FILE_NAME);
    let mut temp_vault_file_path: PathBuf = vault_dir_path.as_ref().into();
    temp_vault_file_path.push(VAULT_TEMP_FILE_NAME);

    // this method is used to rewrite existing vault file
    // => write to temporary file first, then rename temporary file to vault file
    let mut vault_file = fs::File::create(&temp_vault_file_path)?;
    let vault_file_contents = json::VaultFile {
        crypto: crypto.into(),
        meta: Some(meta.to_owned()),
    };
    vault_file_contents
        .write(&mut vault_file)
        .map_err(|e| Error::Custom(format!("{:?}", e)))?;
    drop(vault_file);
    fs::rename(&temp_vault_file_path, &vault_file_path)?;

    Ok(())
}

/// When vault is opened => we must check that password matches && read metadata
fn read_vault_file<P>(vault_dir_path: P, key: Option<&VaultKey>) -> Result<String, Error>
where P: AsRef<Path> {
    let mut vault_file_path: PathBuf = vault_dir_path.as_ref().into();
    vault_file_path.push(VAULT_FILE_NAME);

    let vault_file = fs::File::open(vault_file_path)?;
    let vault_file_contents =
        json::VaultFile::load(vault_file).map_err(|e| Error::Custom(format!("{:?}", e)))?;
    let vault_file_meta = vault_file_contents.meta.unwrap_or("{}".to_owned());
    let vault_file_crypto: Crypto = vault_file_contents.crypto.into();

    if let Some(key) = key {
        let password_bytes = vault_file_crypto.decrypt(&key.password)?;
        let password_hash = blake2b(&key.password);
        if password_hash.0 != password_bytes.as_slice() {
            return Err(Error::InvalidPassword);
        }
    }

    Ok(vault_file_meta)
}