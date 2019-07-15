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
extern crate key;
extern crate rustc_hex;

use rustc_hex::ToHex;
use key::generate_keypair;
use keychain::{Crypto, Error};

#[test]
fn crypto_with_secret_create() {
    let keypair = generate_keypair();
    let crypto = Crypto::with_secret_ed25519(keypair.secret(), "this is sparta", 10240);
    let secret = crypto.secret_ed25519("this is sparta").unwrap();
    assert_eq!(keypair.secret().to_hex(), secret.to_hex());
}

#[test]
fn crypto_with_secret_invalid_password() {
    let keypair = generate_keypair();
    let crypto = Crypto::with_secret_ed25519(keypair.secret(), "this is sparta", 10240);

    match crypto.secret_ed25519("this is sparta!") {
        Err(Error::InvalidPassword) => {
            assert!(true);
        }
        _ => {
            assert!(false);
        }
    }
}

#[test]
fn crypto_with_null_plain_data() {
    let original_data = b"";
    let crypto = Crypto::with_plain(&original_data[..], "this is sparta", 10240);
    let decrypted_data = crypto.decrypt("this is sparta").unwrap();
    assert_eq!(original_data[..], *decrypted_data);
}

#[test]
fn crypto_with_tiny_plain_data() {
    let original_data = b"{}";
    let crypto = Crypto::with_plain(&original_data[..], "this is sparta", 10240);
    let decrypted_data = crypto.decrypt("this is sparta").unwrap();
    assert_eq!(original_data[..], *decrypted_data);
}

#[test]
fn crypto_with_huge_plain_data() {
    let original_data: Vec<_> = (1..65536).map(|i| (i % 256) as u8).collect();
    let crypto = Crypto::with_plain(&original_data, "this is sparta", 10240);
    let decrypted_data = crypto.decrypt("this is sparta").unwrap();
    assert_eq!(&original_data, &decrypted_data);
}
