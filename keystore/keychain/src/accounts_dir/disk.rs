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
use std::io::Write;
use std::path::{PathBuf, Path};
use std::collections::HashMap;
use time;
use {json, SafeAccount, Error};
use rlp::{self};
use super::{KeyDirectory, VaultKeyDirectory, VaultKeyDirectoryProvider, VaultKey};
use super::vault::{VAULT_FILE_NAME, VaultDiskDirectory};

const IGNORED_FILES: &'static [&'static str] = &[
    "thumbs.db",
    "address_book.json",
    "dapps_policy.json",
    "dapps_accounts.json",
    "dapps_history.json",
    "vault.json",
];

#[cfg(not(windows))]
fn restrict_permissions_to_owner(file_path: &Path) -> Result<(), i32> {
    use std::ffi;

    let cstr = ffi::CString::new(&*file_path.to_string_lossy()).map_err(|_| -1)?;
    match unsafe { libc::chmod(cstr.as_ptr(), libc::S_IWUSR | libc::S_IRUSR) } {
        0 => Ok(()),
        x => Err(x),
    }
}

#[cfg(windows)]
fn restrict_permissions_to_owner(_file_path: &Path) -> Result<(), i32> { Ok(()) }

/// Root keys directory implementation
pub type RootDiskDirectory = DiskDirectory<DiskKeyFileManager>;

/// Disk directory key file manager
pub trait KeyFileManager: Send + Sync {
    /// Read `SafeAccount` from given key file stream
    fn read<T>(&self, filename: Option<String>, reader: T) -> Result<SafeAccount, Error>
    where T: io::Read;
    /// Read `SafeAccount` from given encoded key file stream
    fn read_encoded<T>(&self, reader: &mut T) -> Result<SafeAccount, Error>
    where T: io::Read;
    /// Write `SafeAccount` to given key file stream
    fn write<T>(&self, account: SafeAccount, writer: &mut T) -> Result<(), Error>
    where T: io::Write;
    /// Write `SafeAccount` to given encoded key file stream
    fn write_encoded<T>(&self, account: SafeAccount, writer: &mut T) -> Result<(), Error>
    where T: io::Write;
}

/// Disk-based keys directory implementation
pub struct DiskDirectory<T>
where T: KeyFileManager
{
    path: PathBuf,
    key_manager: T,
}

/// Keys file manager for root keys directory
pub struct DiskKeyFileManager;

impl RootDiskDirectory {
    pub fn create<P>(path: P) -> Result<Self, Error>
    where P: AsRef<Path> {
        fs::create_dir_all(&path)?;
        Ok(Self::at(path))
    }

    pub fn at<P>(path: P) -> Self
    where P: AsRef<Path> {
        DiskDirectory::new(path, DiskKeyFileManager)
    }
}

impl<T> DiskDirectory<T>
where T: KeyFileManager
{
    /// Create new disk directory instance
    pub fn new<P>(path: P, key_manager: T) -> Self
    where P: AsRef<Path> {
        DiskDirectory {
            path: path.as_ref().to_path_buf(),
            key_manager,
        }
    }

    fn files(&self) -> Result<Vec<PathBuf>, Error> {
        Ok(fs::read_dir(&self.path)?
            .flat_map(Result::ok)
            .filter(|entry| {
                let metadata = entry.metadata().ok();
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                // filter directories
                metadata.map_or(false, |m| !m.is_dir()) &&
                    // hidden files
                    !name.starts_with(".") &&
                    // other ignored files
                    !IGNORED_FILES.contains(&&*name)
            })
            .map(|entry| entry.path())
            .collect::<Vec<PathBuf>>())
    }

    pub fn files_hash(&self) -> Result<u64, Error> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        let files = self.files()?;
        for file in files {
            hasher.write(file.to_str().unwrap_or("").as_bytes())
        }

        Ok(hasher.finish())
    }

    fn last_modification_date(&self) -> Result<u64, Error> {
        use std::time::{Duration, UNIX_EPOCH};
        let duration = fs::metadata(&self.path)?
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::default());
        let timestamp = duration.as_secs() ^ (duration.subsec_nanos() as u64);
        Ok(timestamp)
    }

    /// all accounts found in keys directory
    fn files_content(&self) -> Result<HashMap<PathBuf, SafeAccount>, Error> {
        // it's not done using one iterator cause
        // there is an issue with rustc and it takes tooo much time to compile
        let paths = self.files()?;
        Ok(paths
            .into_iter()
            .filter_map(|path| {
                fs::File::open(path.clone())
                    .map_err(Into::into)
                    .and_then(|mut file| self.key_manager.read_encoded(&mut file))
                    .map_err(|err| {
                        warn!(target: "disk","Invalid key file: {:?} ({})", path, err);
                        err
                    })
                    .map(|account| (path, account))
                    .ok()
            })
            .collect())
    }

    /// insert account with given filename. if the filename is a duplicate of any stored account and dedup is set to
    /// true, a random suffix is appended to the filename.
    pub fn insert_with_filename(
        &self,
        account: SafeAccount,
        mut filename: String,
        dedup: bool,
    ) -> Result<SafeAccount, Error>
    {
        // path to keyfile
        let mut keyfile_path = self.path.join(filename.as_str());

        // check for duplicate filename and append random suffix
        if dedup && keyfile_path.exists() {
            let suffix = ::random::random_string(4);
            filename.push_str(&format!("-{}", suffix));
            keyfile_path.set_file_name(&filename);
        }

        // update account filename
        let original_account = account.clone();
        let mut account = account;
        account.filename = Some(filename);

        {
            // save the file
            let mut file = fs::File::create(&keyfile_path)?;

            // write key content
            self.key_manager
                .write_encoded(original_account, &mut file)
                .map_err(|e| Error::Custom(format!("{:?}", e)))?;

            file.flush()?;

            if let Err(_) = restrict_permissions_to_owner(keyfile_path.as_path()) {
                return Err(Error::Io(io::Error::last_os_error()));
            }

            file.sync_all()?;
        }

        Ok(account)
    }

    /// Get key file manager referece
    pub fn key_manager(&self) -> &T { &self.key_manager }
}

impl<T> KeyDirectory for DiskDirectory<T>
where T: KeyFileManager
{
    fn load(&self) -> Result<Vec<SafeAccount>, Error> {
        let accounts = self
            .files_content()?
            .into_iter()
            .map(|(_, account)| account)
            .collect();
        Ok(accounts)
    }

    fn update(&self, account: SafeAccount) -> Result<SafeAccount, Error> {
        // Disk store handles updates correctly iff filename is the same
        let filename = account_filename(&account);
        self.insert_with_filename(account, filename, false)
    }

    fn insert(&self, account: SafeAccount) -> Result<SafeAccount, Error> {
        let filename = account_filename(&account);
        self.insert_with_filename(account, filename, true)
    }

    fn remove(&self, account: &SafeAccount) -> Result<(), Error> {
        // enumerate all entries in keystore
        // and find entry with given address
        let to_remove = self
            .files_content()?
            .into_iter()
            .find(|&(_, ref acc)| acc.id == account.id && acc.address == account.address);

        // remove it
        match to_remove {
            None => Err(Error::InvalidAccount),
            Some((path, _)) => fs::remove_file(path).map_err(From::from),
        }
    }

    fn path(&self) -> Option<&PathBuf> { Some(&self.path) }

    fn as_vault_provider(&self) -> Option<&dyn VaultKeyDirectoryProvider> { Some(self) }

    fn unique_repr(&self) -> Result<u64, Error> { self.last_modification_date() }
}

impl<T> VaultKeyDirectoryProvider for DiskDirectory<T>
where T: KeyFileManager
{
    fn create(&self, name: &str, key: VaultKey) -> Result<Box<dyn VaultKeyDirectory>, Error> {
        let vault_dir = VaultDiskDirectory::create(&self.path, name, key)?;
        Ok(Box::new(vault_dir))
    }

    fn open(&self, name: &str, key: VaultKey) -> Result<Box<dyn VaultKeyDirectory>, Error> {
        let vault_dir = VaultDiskDirectory::at(&self.path, name, key)?;
        Ok(Box::new(vault_dir))
    }

    fn list_vaults(&self) -> Result<Vec<String>, Error> {
        Ok(fs::read_dir(&self.path)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter_map(|path| {
                let mut vault_file_path = path.clone();
                vault_file_path.push(VAULT_FILE_NAME);
                if vault_file_path.is_file() {
                    path.file_name()
                        .and_then(|f| f.to_str())
                        .map(|f| f.to_owned())
                } else {
                    None
                }
            })
            .collect())
    }

    fn vault_meta(&self, name: &str) -> Result<String, Error> {
        VaultDiskDirectory::meta_at(&self.path, name)
    }
}

impl KeyFileManager for DiskKeyFileManager {
    fn read<T>(&self, filename: Option<String>, reader: T) -> Result<SafeAccount, Error>
    where T: io::Read {
        let key_file =
            json::KeyFile::load(reader).map_err(|e| Error::Custom(format!("{:?}", e)))?;
        Ok(SafeAccount::from_file(key_file, filename))
    }

    fn write<T>(&self, mut account: SafeAccount, writer: &mut T) -> Result<(), Error>
    where T: io::Write {
        // when account is moved back to root directory from vault
        // => remove vault field from meta
        account.meta = json::remove_vault_name_from_json_meta(&account.meta)
            .map_err(|err| Error::Custom(format!("{:?}", err)))?;

        let key_file: json::KeyFile = account.into();
        key_file
            .write(writer)
            .map_err(|e| Error::Custom(format!("{:?}", e)))
    }

    fn read_encoded<T>(&self, reader: &mut T) -> Result<SafeAccount, Error>
    where T: io::Read {
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        Ok(rlp::decode(&buffer))
    }

    fn write_encoded<T>(&self, account: SafeAccount, writer: &mut T) -> Result<(), Error>
    where T: io::Write {
        let rlp = rlp::encode(&account);
        writer.write_all(&rlp)?;
        Ok(())
    }
}

fn account_filename(account: &SafeAccount) -> String {
    // build file path
    account.filename.clone().unwrap_or_else(|| {
        let tm = time::now_utc();
        let timestamp =
            time::strftime("%Y-%m-%dT%H:%M:%S", &tm).expect("Time-format string is valid.");
        format!(
            "UTC--{}.{}Z--{:?}",
            timestamp,
            tm.tm_nsec as i64 / 1000 / 1000,
            account.address
        )
    })
}
