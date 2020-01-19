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
use std::io::{Read, Write};
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::{Error, Visitor, MapAccess, DeserializeOwned};
use serde_json;
use super::{Uuid, Version, Crypto, H256};

/// Public opaque type representing serializable `KeyFile`.
#[derive(Debug, PartialEq)]
pub struct OpaqueKeyFile {
    key_file: KeyFile,
}

impl Serialize for OpaqueKeyFile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        self.key_file.serialize(serializer)
    }
}

impl<T> From<T> for OpaqueKeyFile
where T: Into<KeyFile>
{
    fn from(val: T) -> Self {
        OpaqueKeyFile {
            key_file: val.into(),
        }
    }
}

#[derive(Debug, PartialEq, Serialize)]
pub struct KeyFile {
    pub id: Uuid,
    pub version: Version,
    pub crypto: Crypto,
    pub address: H256,
    pub name: Option<String>,
    pub meta: Option<String>,
}

enum KeyFileField {
    Id,
    Version,
    Crypto,
    Address,
    Name,
    Meta,
}

impl<'a> Deserialize<'a> for KeyFileField {
    fn deserialize<D>(deserializer: D) -> Result<KeyFileField, D::Error>
    where D: Deserializer<'a> {
        deserializer.deserialize_any(KeyFileFieldVisitor)
    }
}

struct KeyFileFieldVisitor;

impl<'a> Visitor<'a> for KeyFileFieldVisitor {
    type Value = KeyFileField;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a valid key file field")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where E: Error {
        match value {
            "id" => Ok(KeyFileField::Id),
            "version" => Ok(KeyFileField::Version),
            "crypto" => Ok(KeyFileField::Crypto),
            "Crypto" => Ok(KeyFileField::Crypto),
            "address" => Ok(KeyFileField::Address),
            "name" => Ok(KeyFileField::Name),
            "meta" => Ok(KeyFileField::Meta),
            _ => Err(Error::custom(format!("Unknown field: '{}'", value))),
        }
    }
}

impl<'a> Deserialize<'a> for KeyFile {
    fn deserialize<D>(deserializer: D) -> Result<KeyFile, D::Error>
    where D: Deserializer<'a> {
        static FIELDS: &'static [&'static str] = &["id", "version", "crypto", "Crypto", "address"];
        deserializer.deserialize_struct("KeyFile", FIELDS, KeyFileVisitor)
    }
}

fn none_if_empty<'a, T>(v: Option<serde_json::Value>) -> Option<T>
where T: DeserializeOwned {
    v.and_then(|v| {
        if v.is_null() {
            None
        } else {
            serde_json::from_value(v).ok()
        }
    })
}

struct KeyFileVisitor;
impl<'a> Visitor<'a> for KeyFileVisitor {
    type Value = KeyFile;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a valid key object")
    }

    fn visit_map<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    where V: MapAccess<'a> {
        let mut id = None;
        let mut version = None;
        let mut crypto = None;
        let mut address = None;
        let mut name = None;
        let mut meta = None;

        loop {
            match visitor.next_key()? {
                Some(KeyFileField::Id) => {
                    id = Some(visitor.next_value()?);
                }
                Some(KeyFileField::Version) => {
                    version = Some(visitor.next_value()?);
                }
                Some(KeyFileField::Crypto) => {
                    crypto = Some(visitor.next_value()?);
                }
                Some(KeyFileField::Address) => {
                    address = Some(visitor.next_value()?);
                }
                Some(KeyFileField::Name) => name = none_if_empty(visitor.next_value().ok()),
                Some(KeyFileField::Meta) => meta = none_if_empty(visitor.next_value().ok()),
                None => break,
            }
        }

        let id = match id {
            Some(id) => id,
            None => return Err(V::Error::missing_field("id")),
        };

        let version = match version {
            Some(version) => version,
            None => return Err(V::Error::missing_field("version")),
        };

        let crypto = match crypto {
            Some(crypto) => crypto,
            None => return Err(V::Error::missing_field("crypto")),
        };

        let address = match address {
            Some(address) => address,
            None => return Err(V::Error::missing_field("address")),
        };

        let result = KeyFile {
            id: id,
            version: version,
            crypto: crypto,
            address: address,
            name: name,
            meta: meta,
        };

        Ok(result)
    }
}

impl KeyFile {
    pub fn load<R>(reader: R) -> Result<Self, serde_json::Error>
    where R: Read {
        serde_json::from_reader(reader)
    }

    pub fn write<W>(&self, writer: &mut W) -> Result<(), serde_json::Error>
    where W: Write {
        serde_json::to_writer(writer, self)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use serde_json;
    use crate::json::{KeyFile, Uuid, Version, Crypto, Cipher, Aes128Ctr, Kdf, Pbkdf2, Prf};

    #[test]
    fn basic_keyfile() {
        let json = r#"
        {
            "address": "a09886911d84b089a0bb1ffa5e532a62d8b1ebc6f4eef8ed3d67cc48df9e46a3",
            "crypto": {
                "cipher": "aes-128-ctr",
                "ciphertext": "e52b0df3ad6da9f643e889ec796212e83e383ebe48a13fbbb1055127382415d2e0629c0a9623abbe660eac5cda8b08a7d0197fbc65ddf416f1156178dd9494ca",
                "cipherparams": {
                    "iv": "fa2aede80ca3e8ff0e2f2073054ab18e"
                },
                "kdf": "pbkdf2",
                "kdfparams": {
                    "c":10240,
                    "dklen": 32,
                    "prf":"hmac-sha256",
                    "salt":"13e96cae54f9ca959fa4764cf9becdafed71af9c2ce55fd2c6894071f15c26f6"
                },
                "mac": "a3a272cf26c5803c3d1acb9e766bbdcda3d7a8380bfc399320f72b085b8b959a"
            },
            "id": "4b497bc4-8617-973d-80ca-4e47c0c9fdf4",
            "version": 3,
            "name": "",
            "meta": "{}"
        }"#;

        let expected = KeyFile {
            id: Uuid::from_str("4b497bc4-8617-973d-80ca-4e47c0c9fdf4").unwrap(),
            version: Version::V3,
            address: "a09886911d84b089a0bb1ffa5e532a62d8b1ebc6f4eef8ed3d67cc48df9e46a3".into(),
            crypto: Crypto {
                cipher: Cipher::Aes128Ctr(Aes128Ctr {
                    iv: "fa2aede80ca3e8ff0e2f2073054ab18e".into(),
                }),
                ciphertext: "e52b0df3ad6da9f643e889ec796212e83e383ebe48a13fbbb1055127382415d2e0629c0a9623abbe660eac5cda8b08a7d0197fbc65ddf416f1156178dd9494ca"
                    .into(),
                kdf: Kdf::Pbkdf2(Pbkdf2 {
                    c: 10240,
                    dklen: 32,
                    prf: Prf::HmacSha256,
                    salt: "13e96cae54f9ca959fa4764cf9becdafed71af9c2ce55fd2c6894071f15c26f6".into(),
                }),
                mac: "a3a272cf26c5803c3d1acb9e766bbdcda3d7a8380bfc399320f72b085b8b959a".into(),
            },
            name: Some("".to_owned()),
            meta: Some("{}".to_owned()),
        };

        let keyfile: KeyFile = serde_json::from_str(json).unwrap();
        assert_eq!(keyfile, expected);
    }

    #[test]
    fn capital_crypto_keyfile() {
        let json = r#"
        {
            "address": "a09886911d84b089a0bb1ffa5e532a62d8b1ebc6f4eef8ed3d67cc48df9e46a3",
            "crypto": {
                "cipher": "aes-128-ctr",
                "ciphertext": "e52b0df3ad6da9f643e889ec796212e83e383ebe48a13fbbb1055127382415d2e0629c0a9623abbe660eac5cda8b08a7d0197fbc65ddf416f1156178dd9494ca",
                "cipherparams": {
                    "iv": "fa2aede80ca3e8ff0e2f2073054ab18e"
                },
                "kdf": "pbkdf2",
                "kdfparams": {
                    "c":10240,
                    "dklen": 32,
                    "prf":"hmac-sha256",
                    "salt":"13e96cae54f9ca959fa4764cf9becdafed71af9c2ce55fd2c6894071f15c26f6"
                },
                "mac": "a3a272cf26c5803c3d1acb9e766bbdcda3d7a8380bfc399320f72b085b8b959a"
            },
            "id": "4b497bc4-8617-973d-80ca-4e47c0c9fdf4",
            "version": 3,
            "name": "",
            "meta": "{}"
        }"#;

        let expected = KeyFile {
            id: Uuid::from_str("4b497bc4-8617-973d-80ca-4e47c0c9fdf4").unwrap(),
            version: Version::V3,
            address: "a09886911d84b089a0bb1ffa5e532a62d8b1ebc6f4eef8ed3d67cc48df9e46a3".into(),
            crypto: Crypto {
                cipher: Cipher::Aes128Ctr(Aes128Ctr {
                    iv: "fa2aede80ca3e8ff0e2f2073054ab18e".into(),
                }),
                ciphertext: "e52b0df3ad6da9f643e889ec796212e83e383ebe48a13fbbb1055127382415d2e0629c0a9623abbe660eac5cda8b08a7d0197fbc65ddf416f1156178dd9494ca"
                    .into(),
                kdf: Kdf::Pbkdf2(Pbkdf2 {
                    c: 10240,
                    dklen: 32,
                    prf: Prf::HmacSha256,
                    salt: "13e96cae54f9ca959fa4764cf9becdafed71af9c2ce55fd2c6894071f15c26f6".into(),
                }),
                mac: "a3a272cf26c5803c3d1acb9e766bbdcda3d7a8380bfc399320f72b085b8b959a".into(),
            },
            name: Some("".to_owned()),
            meta: Some("{}".to_owned()),
        };

        let keyfile: KeyFile = serde_json::from_str(json).unwrap();
        assert_eq!(keyfile, expected);
    }

    #[test]
    fn to_and_from_json() {
        let file = KeyFile {
            id: Uuid::from_str("4b497bc4-8617-973d-80ca-4e47c0c9fdf4").unwrap(),
            version: Version::V3,
            address: "a09886911d84b089a0bb1ffa5e532a62d8b1ebc6f4eef8ed3d67cc48df9e46a3".into(),
            crypto: Crypto {
                cipher: Cipher::Aes128Ctr(Aes128Ctr {
                    iv: "fa2aede80ca3e8ff0e2f2073054ab18e".into(),
                }),
                ciphertext: "e52b0df3ad6da9f643e889ec796212e83e383ebe48a13fbbb1055127382415d2e0629c0a9623abbe660eac5cda8b08a7d0197fbc65ddf416f1156178dd9494ca"
                    .into(),
                kdf: Kdf::Pbkdf2(Pbkdf2 {
                    c: 10240,
                    dklen: 32,
                    prf: Prf::HmacSha256,
                    salt: "13e96cae54f9ca959fa4764cf9becdafed71af9c2ce55fd2c6894071f15c26f6".into(),
                }),
                mac: "a3a272cf26c5803c3d1acb9e766bbdcda3d7a8380bfc399320f72b085b8b959a".into(),
            },
            name: Some("".to_owned()),
            meta: Some("{}".to_owned()),
        };

        let serialized = serde_json::to_string(&file).unwrap();
        println!("{}", serialized);
        let deserialized = serde_json::from_str(&serialized).unwrap();

        assert_eq!(file, deserialized);
    }
}
