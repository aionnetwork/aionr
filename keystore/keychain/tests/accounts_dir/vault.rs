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

extern crate tempdir;
extern crate keychain;

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempdir::TempDir;
use keychain::accounts_dir::VaultKey;

use super::{
    VAULT_FILE_NAME, check_vault_name, make_vault_dir_path, create_vault_file, read_vault_file,
    VaultDiskDirectory,
};
use self::tempdir::TempDir;

#[test]
fn check_vault_name_succeeds() {
    assert!(check_vault_name("vault"));
    assert!(check_vault_name("vault with spaces"));
    assert!(check_vault_name("vault    with    tabs"));
    assert!(check_vault_name("vault_with_underscores"));
    assert!(check_vault_name("vault-with-dashes"));
    assert!(check_vault_name("vault-with-digits-123"));
    assert!(check_vault_name("vault中文名字"));
}

#[test]
fn check_vault_name_fails() {
    assert!(!check_vault_name(""));
    assert!(!check_vault_name("."));
    assert!(!check_vault_name("*"));
    assert!(!check_vault_name("../.bash_history"));
    assert!(!check_vault_name("/etc/passwd"));
    assert!(!check_vault_name("c:\\windows"));
}

#[test]
fn make_vault_dir_path_succeeds() {
    assert_eq!(
        make_vault_dir_path("/home/user/parity", "vault", true)
            .unwrap()
            .to_str()
            .unwrap(),
        "/home/user/parity/vault"
    );
    assert_eq!(
        make_vault_dir_path("/home/user/parity", "*bad-name*", false)
            .unwrap()
            .to_str()
            .unwrap(),
        "/home/user/parity/*bad-name*"
    );
}

#[test]
fn make_vault_dir_path_fails() {
    assert!(make_vault_dir_path("/home/user/parity", "*bad-name*", true).is_err());
}

#[test]
fn create_vault_file_succeeds() {
    // given
    let temp_path = TempDir::new("").unwrap();
    let key = VaultKey::new("password", 1024);
    let mut vault_dir: PathBuf = temp_path.path().into();
    vault_dir.push("vault");
    fs::create_dir_all(&vault_dir).unwrap();

    // when
    let result = create_vault_file(&vault_dir, &key, "{}");

    // then
    assert!(result.is_ok());
    let mut vault_file_path = vault_dir.clone();
    vault_file_path.push(VAULT_FILE_NAME);
    assert!(vault_file_path.exists() && vault_file_path.is_file());
}

/*
#[test]
fn read_vault_file_succeeds() {
    // given
    let temp_path = TempDir::new("").unwrap();
    let key = VaultKey::new("password", 1024);
    let vault_file_contents = r#"{"crypto":{"cipher":"aes-128-ctr","cipherparams":{"iv":"758696c8dc6378ab9b25bb42790da2f5"},"ciphertext":"54eb50683717d41caaeb12ea969f2c159daada5907383f26f327606a37dc7168","kdf":"pbkdf2","kdfparams":{"c":1024,"dklen":32,"prf":"hmac-sha256","salt":"3c320fa566a1a7963ac8df68a19548d27c8f40bf92ef87c84594dcd5bbc402b6"},"mac":"9e5c2314c2a0781962db85611417c614bd6756666b6b1e93840f5b6ed895f003"}}"#;
    let dir: PathBuf = temp_path.path().into();
    let mut vault_file_path: PathBuf = dir.clone();
    vault_file_path.push(VAULT_FILE_NAME);
    {
        let mut vault_file = fs::File::create(vault_file_path).unwrap();
        vault_file
            .write_all(vault_file_contents.as_bytes())
            .unwrap();
    }

    // when
    let result = read_vault_file(&dir, Some(&key));

    // then
    assert!(result.is_ok());
}
*/
#[test]
fn read_vault_file_fails() {
    // given
    let temp_path = TempDir::new("").unwrap();
    let key = VaultKey::new("password1", 1024);
    let dir: PathBuf = temp_path.path().into();
    let mut vault_file_path: PathBuf = dir.clone();
    vault_file_path.push(VAULT_FILE_NAME);

    // when
    let result = read_vault_file(&dir, Some(&key));

    // then
    assert!(result.is_err());

    // and when given
    let vault_file_contents = r#"{"crypto":{"cipher":"aes-128-ctr","cipherparams":{"iv":"0155e3690be19fbfbecabcd440aa284b"},"ciphertext":"4d6938a1f49b7782","kdf":"pbkdf2","kdfparams":{"c":1024,"dklen":32,"prf":"hmac-sha256","salt":"b6a9338a7ccd39288a86dba73bfecd9101b4f3db9c9830e7c76afdbd4f6872e5"},"mac":"16381463ea11c6eb2239a9f339c2e780516d29d234ce30ac5f166f9080b5a262"}}"#;
    {
        let mut vault_file = fs::File::create(vault_file_path).unwrap();
        vault_file
            .write_all(vault_file_contents.as_bytes())
            .unwrap();
    }

    // when
    let result = read_vault_file(&dir, Some(&key));

    // then
    assert!(result.is_err());
}

#[test]
fn vault_directory_can_be_created() {
    // given
    let temp_path = TempDir::new("").unwrap();
    let key = VaultKey::new("password", 1024);
    let dir: PathBuf = temp_path.path().into();

    // when
    let vault = VaultDiskDirectory::create(&dir, "vault", key.clone());

    // then
    assert!(vault.is_ok());

    // and when
    let vault = VaultDiskDirectory::at(&dir, "vault", key);

    // then
    assert!(vault.is_ok());
}

#[test]
fn vault_directory_cannot_be_created_if_already_exists() {
    // given
    let temp_path = TempDir::new("").unwrap();
    let key = VaultKey::new("password", 1024);
    let dir: PathBuf = temp_path.path().into();
    let mut vault_dir = dir.clone();
    vault_dir.push("vault");
    fs::create_dir_all(&vault_dir).unwrap();

    // when
    let vault = VaultDiskDirectory::create(&dir, "vault", key);

    // then
    assert!(vault.is_err());
}

#[test]
fn vault_directory_cannot_be_opened_if_not_exists() {
    // given
    let temp_path = TempDir::new("").unwrap();
    let key = VaultKey::new("password", 1024);
    let dir: PathBuf = temp_path.path().into();

    // when
    let vault = VaultDiskDirectory::at(&dir, "vault", key);

    // then
    assert!(vault.is_err());
}