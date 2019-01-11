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

use std::str::FromStr;
use rustc_hex::ToHex;
use uuid::Uuid;
use ethkey::{Ed25519KeyPair, Ed25519Signature, Address, Message, sign_ed25519};
use {json, Error};
use account::Version;
use aion_types::H256;
use rlp::{self, RlpStream, UntrustedRlp, DecoderError};
use super::crypto::Crypto;

/// Account representation.
#[derive(Debug, PartialEq, Clone)]
pub struct SafeAccount {
    /// Account ID
    pub id: Uuid,
    /// Account version
    pub version: Version,
    /// Account address
    pub address: Address,
    /// Account private key derivation definition.
    pub crypto: Crypto,
    /// Account filename
    pub filename: Option<String>,
    /// Account name
    pub name: String,
    /// Account metadata
    pub meta: String,
}

impl Into<json::KeyFile> for SafeAccount {
    fn into(self) -> json::KeyFile {
        json::KeyFile {
            id: From::from(self.id.as_bytes().clone()),
            version: self.version.into(),
            address: self.address.into(),
            crypto: self.crypto.into(),
            name: Some(self.name.into()),
            meta: Some(self.meta.into()),
        }
    }
}

impl SafeAccount {
    /// Create a new account with ed25519
    pub fn create_ed25519(
        keypair: &Ed25519KeyPair,
        id: [u8; 16],
        password: &str,
        iterations: u32,
        name: String,
        meta: String,
    ) -> Self
    {
        SafeAccount {
            id: Uuid::from_random_bytes(id),
            version: Version::V3,
            crypto: Crypto::with_secret_ed25519(keypair.secret(), password, iterations),
            address: keypair.address(),
            filename: None,
            name: name,
            meta: meta,
        }
    }

    /// Create a new `SafeAccount` from the given `json`; if it was read from a
    /// file, the `filename` should be `Some` name. If it is as yet anonymous, then it
    /// can be left `None`.
    pub fn from_file(json: json::KeyFile, filename: Option<String>) -> Self {
        SafeAccount {
            id: Uuid::from_random_bytes(json.id.into()),
            version: json.version.into(),
            address: json.address.into(),
            crypto: json.crypto.into(),
            filename: filename,
            name: json.name.unwrap_or(String::new()),
            meta: json.meta.unwrap_or("{}".to_owned()),
        }
    }

    /// Create a new `SafeAccount` from the given vault `json`; if it was read from a
    /// file, the `filename` should be `Some` name. If it is as yet anonymous, then it
    /// can be left `None`.
    pub fn from_vault_file(
        password: &str,
        json: json::VaultKeyFile,
        filename: Option<String>,
    ) -> Result<Self, Error>
    {
        let meta_crypto: Crypto = json.metacrypto.into();
        let meta_plain = meta_crypto.decrypt(password)?;
        let meta_plain =
            json::VaultKeyMeta::load(&meta_plain).map_err(|e| Error::Custom(format!("{:?}", e)))?;

        Ok(SafeAccount::from_file(
            json::KeyFile {
                id: json.id,
                version: json.version,
                crypto: json.crypto,
                address: meta_plain.address,
                name: meta_plain.name,
                meta: meta_plain.meta,
            },
            filename,
        ))
    }

    /// Create a new `VaultKeyFile` from the given `self`
    pub fn into_vault_file(
        self,
        iterations: u32,
        password: &str,
    ) -> Result<json::VaultKeyFile, Error>
    {
        let meta_plain = json::VaultKeyMeta {
            address: self.address.into(),
            name: Some(self.name),
            meta: Some(self.meta),
        };
        let meta_plain = meta_plain
            .write()
            .map_err(|e| Error::Custom(format!("{:?}", e)))?;
        let meta_crypto = Crypto::with_plain(&meta_plain, password, iterations);

        Ok(json::VaultKeyFile {
            id: self.id.as_bytes().clone().into(),
            version: self.version.into(),
            crypto: self.crypto.into(),
            metacrypto: meta_crypto.into(),
        })
    }

    /// Sign a message.
    pub fn sign(&self, password: &str, message: &Message) -> Result<Ed25519Signature, Error> {
        let secret = self.crypto.secret_ed25519(password)?;
        sign_ed25519(&secret, message).map_err(From::from)
    }

    /// Check if password matches the account.
    pub fn check_password(&self, password: &str) -> bool {
        self.crypto.secret_ed25519(password).is_ok()
    }
}

impl rlp::Decodable for SafeAccount {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        Ok(SafeAccount {
            id: {
                let val: String = d.val_at(0)?;
                Uuid::parse_str(&val).map_err(|_| DecoderError::Custom("id parse error"))?
            },
            version: d.val_at(1)?,
            address: {
                let val: String = d.val_at(2)?;
                H256::from_str(&val).map_err(|_| DecoderError::Custom("address parse error"))?
            },
            crypto: d.val_at(3)?,
            filename: None,
            name: String::new(),
            meta: String::new(),
        })
    }
}

impl rlp::Encodable for SafeAccount {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&self.id.hyphenated().to_string());
        s.append(&self.version);
        s.append(&self.address.to_hex());
        s.append(&self.crypto);
    }
}
