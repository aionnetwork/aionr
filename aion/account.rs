/*******************************************************************************
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

use std::path::PathBuf;
use keychain::{EthStore, StoreAccountRef, import_account, import_accounts};
use keychain::accounts_dir::RootDiskDirectory;
use key::Ed25519KeyPair;
use acore::account_provider::{AccountProvider, AccountProviderSettings};
use helpers::{password_prompt, password_from_file, password_once};
use params::SpecType;
use aion_types::clean_0x;
use rustc_hex::ToHex;
#[derive(Debug, PartialEq)]
pub enum AccountCmd {
    New(NewAccount),
    List(ListAccounts),
    Import(ImportAccounts),
    ImportByPrivkey(ImportAccount),
    ExportToProvkey(ExportAccount),
}

#[derive(Debug, PartialEq)]
pub struct ListAccounts {
    pub path: String,
    pub spec: SpecType,
}

#[derive(Debug, PartialEq)]
pub struct NewAccount {
    pub iterations: u32,
    pub path: String,
    pub spec: SpecType,
    pub password_file: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct ImportAccounts {
    pub from: Vec<String>,
    pub to: String,
    pub spec: SpecType,
}

#[derive(Debug, PartialEq)]
pub struct ImportAccount {
    pub path: String,
    pub spec: SpecType,
    pub iterations: u32,
    pub pri_keys: Option<String>,
}
#[derive(Debug, PartialEq)]
pub struct ExportAccount {
    pub path: String,
    pub spec: SpecType,
    pub iterations: u32,
    pub address: Option<String>,
}

pub fn execute(cmd: AccountCmd) -> Result<String, String> {
    match cmd {
        AccountCmd::New(new_cmd) => new(new_cmd),
        AccountCmd::List(list_cmd) => list(list_cmd),
        AccountCmd::Import(import_cmd) => import(import_cmd),
        AccountCmd::ImportByPrivkey(import_cmd) => import_by_private_key(import_cmd),
        AccountCmd::ExportToProvkey(export_cmd) => export_to_private_key(export_cmd),
    }
}

fn keys_dir(path: String, spec: SpecType) -> Result<RootDiskDirectory, String> {
    let spec = spec.spec()?;
    let mut path = PathBuf::from(&path);
    path.push(spec.data_dir);
    RootDiskDirectory::create(path).map_err(|e| format!("Could not open keys directory: {}", e))
}

fn secret_store(dir: Box<RootDiskDirectory>, iterations: Option<u32>) -> Result<EthStore, String> {
    match iterations {
        Some(i) => EthStore::open_with_iterations(dir, i),
        _ => EthStore::open(dir),
    }
    .map_err(|e| format!("Could not open keys store: {}", e))
}

fn new(n: NewAccount) -> Result<String, String> {
    let password: String = match n.password_file {
        Some(file) => password_from_file(file)?,
        None => password_prompt()?,
    };
    let dir = Box::new(keys_dir(n.path, n.spec)?);
    let secret_store = Box::new(secret_store(dir, Some(n.iterations))?);
    let acc_provider = AccountProvider::new(secret_store, AccountProviderSettings::default());
    let new_account = acc_provider
        .new_account_ed25519(&password)
        .map_err(|e| format!("Could not create new account: {}", e))?;
    Ok(format!("0x{:?}", new_account))
}

fn list(list_cmd: ListAccounts) -> Result<String, String> {
    let dir = Box::new(keys_dir(list_cmd.path, list_cmd.spec)?);
    let secret_store = Box::new(secret_store(dir, None)?);
    let acc_provider = AccountProvider::new(secret_store, AccountProviderSettings::default());
    let accounts = acc_provider.accounts().map_err(|e| format!("{}", e))?;
    let result = accounts
        .into_iter()
        .map(|a| format!("0x{:?}", a))
        .collect::<Vec<String>>()
        .join("\n");

    Ok(result)
}

fn import(i: ImportAccounts) -> Result<String, String> {
    let to = keys_dir(i.to, i.spec)?;
    let mut imported = 0;

    for path in &i.from {
        let path = PathBuf::from(path);
        if path.is_dir() {
            let from = RootDiskDirectory::at(&path);
            imported += import_accounts(&from, &to)
                .map_err(|e| format!("Importing accounts from {:?} failed: {}", path, e))?
                .len();
        } else if path.is_file() {
            import_account(&path, &to)
                .map_err(|e| format!("Importing account from {:?} failed: {}", path, e))?;
            imported += 1;
        }
    }

    Ok(format!("{} account(s) imported", imported))
}

/// Import account by private key
fn import_by_private_key(i: ImportAccount) -> Result<String, String> {
    let dir = Box::new(keys_dir(i.path, i.spec)?);
    let secret_store = Box::new(secret_store(dir, Some(i.iterations))?);
    let acc_provider = AccountProvider::new(secret_store, AccountProviderSettings::default());
    if i.pri_keys.is_none() {
        return Ok(" ".into());
    }
    let key = i.pri_keys.clone().expect("private key is none");
    let key_secret = clean_0x(&key)
        .parse()
        .map_err(|_| "Invalid private key!!".to_owned())?;
    let key_pair =
        Ed25519KeyPair::from_secret(key_secret).map_err(|_| "Invalid private key!!".to_owned())?;
    let password = password_prompt()?;
    let address = key_pair.address();
    if acc_provider
        .has_account(&address)
        .map_err(|_| "other error!!".to_owned())?
    {
        return Err("Failed to import the private key. Already exists?".to_owned());
    }
    let address = acc_provider
        .insert_account_ed25519(key_pair.secret().clone(), &password)
        .map_err(|_| "invalid account".to_owned())?;
    Ok(format!("A new account has been created: 0x{:?}", address))
}

/// export account to private key
fn export_to_private_key(e: ExportAccount) -> Result<String, String> {
    let dir = Box::new(keys_dir(e.path, e.spec)?);
    let secret_store = secret_store(dir, Some(e.iterations))?;
    if e.address.is_none() {
        return Ok("".into());
    }
    let address = e.address.clone().expect("address is nonce");
    let address = clean_0x(&address)
        .parse()
        .map_err(|_| "The account dose not exit!".to_owned())?;
    let accountref = StoreAccountRef::new(address);
    let account = secret_store
        .get(&accountref)
        .map_err(|_| "The account dose not exit!".to_owned())?;
    let password = password_once()?;
    let key_secret = account
        .crypto
        .secret_ed25519(&password)
        .map_err(|e| format!("Failed to unlock the account. {:?}", e))?;
    Ok(format!("Your private key is: 0x{}", key_secret.to_hex()))
}
