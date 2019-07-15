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

extern crate keychain;
extern crate tempdir;

use std::{env, fs};
use keychain::{KeyDirectory, RootDiskDirectory, VaultKey};
//use account::SafeAccount;
//use ethkey::Generator;
use self::tempdir::TempDir;

/*#[test]
fn should_create_new_account() {
    // given
    let mut dir = env::temp_dir();
    dir.push("ethstore_should_create_new_account");
    let keypair = Random.generate().unwrap();
    let password = "hello world";
    let directory = RootDiskDirectory::create(dir.clone()).unwrap();

    // when
    let account = SafeAccount::create_ed25519(
        &keypair,
        [0u8; 16],
        password,
        1024,
        "Test".to_owned(),
        "{}".to_owned(),
    );
    let res = directory.insert(account);

    // then
    assert!(res.is_ok(), "Should save account succesfuly.");
    assert!(
        res.unwrap().filename.is_some(),
        "Filename has been assigned."
    );

    // cleanup
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn should_handle_duplicate_filenames() {
    // given
    let mut dir = env::temp_dir();
    dir.push("ethstore_should_handle_duplicate_filenames");
    let keypair = Random.generate().unwrap();
    let password = "hello world";
    let directory = RootDiskDirectory::create(dir.clone()).unwrap();

    // when
    let account = SafeAccount::create_ed25519(
        &keypair,
        [0u8; 16],
        password,
        1024,
        "Test".to_owned(),
        "{}".to_owned(),
    );
    let filename = "test".to_string();
    let dedup = true;

    directory
        .insert_with_filename(account.clone(), "foo".to_string(), dedup)
        .unwrap();
    let file1 = directory
        .insert_with_filename(account.clone(), filename.clone(), dedup)
        .unwrap()
        .filename
        .unwrap();
    let file2 = directory
        .insert_with_filename(account.clone(), filename.clone(), dedup)
        .unwrap()
        .filename
        .unwrap();
    let file3 = directory
        .insert_with_filename(account.clone(), filename.clone(), dedup)
        .unwrap()
        .filename
        .unwrap();

    // then
    // the first file should have the original names
    assert_eq!(file1, filename);

    // the following duplicate files should have a suffix appended
    assert!(file2 != file3);
    assert_eq!(file2.len(), filename.len() + 5);
    assert_eq!(file3.len(), filename.len() + 5);

    // cleanup
    let _ = fs::remove_dir_all(dir);
}*/

#[test]
fn should_manage_vaults() {

    println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    // given
    let mut dir = env::temp_dir();
    dir.push("should_create_new_vault");
    let directory = RootDiskDirectory::create(dir.clone()).unwrap();
    let vault_name = "vault";
    let password = "password";

    // then
    assert!(directory.as_vault_provider().is_some());

    // and when
    let before_root_items_count = fs::read_dir(&dir).unwrap().count();
    let vault = directory
        .as_vault_provider()
        .unwrap()
        .create(vault_name, VaultKey::new(password, 1024));

    // then
    assert!(vault.is_ok());
    let after_root_items_count = fs::read_dir(&dir).unwrap().count();
    assert!(after_root_items_count > before_root_items_count);

    // and when
    let vault = directory
        .as_vault_provider()
        .unwrap()
        .open(vault_name, VaultKey::new(password, 1024));

    // then
    assert!(vault.is_ok());
    let after_root_items_count2 = fs::read_dir(&dir).unwrap().count();
    assert!(after_root_items_count == after_root_items_count2);

    // cleanup
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn should_list_vaults() {
    // given
    let temp_path = TempDir::new("").unwrap();
    let directory = RootDiskDirectory::create(&temp_path).unwrap();
    let vault_provider = directory.as_vault_provider().unwrap();
    vault_provider
        .create("vault1", VaultKey::new("password1", 1))
        .unwrap();
    vault_provider
        .create("vault2", VaultKey::new("password2", 1))
        .unwrap();

    // then
    let vaults = vault_provider.list_vaults().unwrap();
    assert_eq!(vaults.len(), 2);
    assert!(vaults.iter().any(|v| &*v == "vault1"));
    assert!(vaults.iter().any(|v| &*v == "vault2"));
}

/*
#[test]
fn hash_of_files() {
    let temp_path = TempDir::new("").unwrap();
    let directory = RootDiskDirectory::create(&temp_path).unwrap();

    let hash = directory
        .files_hash()
        .expect("Files hash should be calculated ok");
    assert_eq!(hash, 15130871412783076140);

    let keypair = Random.generate().unwrap();
    let password = "test pass";
    let account = SafeAccount::create(
        &keypair,
        [0u8; 16],
        password,
        1024,
        "Test".to_owned(),
        "{}".to_owned(),
    );
    directory
        .insert(account)
        .expect("Account should be inserted ok");

    let new_hash = directory
        .files_hash()
        .expect("New files hash should be calculated ok");

    assert!(
        new_hash != hash,
        "hash of the file list should change once directory content changed"
    );
}*/