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

use json;
use rustc_hex::{FromHex, ToHex};
use rlp::{self, RlpStream, UntrustedRlp, DecoderError};

#[derive(Debug, PartialEq, Clone)]
pub enum Prf {
    HmacSha256,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Pbkdf2 {
    pub c: u32,
    pub dklen: u32,
    pub prf: Prf,
    pub salt: [u8; 32],
}

#[derive(Debug, PartialEq, Clone)]
pub struct Scrypt {
    pub dklen: u32,
    pub p: u32,
    pub n: u32,
    pub r: u32,
    pub salt: [u8; 32],
}

#[derive(Debug, PartialEq, Clone)]
pub enum Kdf {
    Pbkdf2(Pbkdf2),
    Scrypt(Scrypt),
}

impl From<json::Prf> for Prf {
    fn from(json: json::Prf) -> Self {
        match json {
            json::Prf::HmacSha256 => Prf::HmacSha256,
        }
    }
}

impl Into<json::Prf> for Prf {
    fn into(self) -> json::Prf {
        match self {
            Prf::HmacSha256 => json::Prf::HmacSha256,
        }
    }
}

impl From<json::Pbkdf2> for Pbkdf2 {
    fn from(json: json::Pbkdf2) -> Self {
        Pbkdf2 {
            c: json.c,
            dklen: json.dklen,
            prf: From::from(json.prf),
            salt: json.salt.into(),
        }
    }
}

impl Into<json::Pbkdf2> for Pbkdf2 {
    fn into(self) -> json::Pbkdf2 {
        json::Pbkdf2 {
            c: self.c,
            dklen: self.dklen,
            prf: self.prf.into(),
            salt: From::from(self.salt),
        }
    }
}

impl From<json::Scrypt> for Scrypt {
    fn from(json: json::Scrypt) -> Self {
        Scrypt {
            dklen: json.dklen,
            p: json.p,
            n: json.n,
            r: json.r,
            salt: json.salt.into(),
        }
    }
}

impl Into<json::Scrypt> for Scrypt {
    fn into(self) -> json::Scrypt {
        json::Scrypt {
            dklen: self.dklen,
            p: self.p,
            n: self.n,
            r: self.r,
            salt: From::from(self.salt),
        }
    }
}

impl From<json::Kdf> for Kdf {
    fn from(json: json::Kdf) -> Self {
        match json {
            json::Kdf::Pbkdf2(params) => Kdf::Pbkdf2(From::from(params)),
            json::Kdf::Scrypt(params) => Kdf::Scrypt(From::from(params)),
        }
    }
}

impl Into<json::Kdf> for Kdf {
    fn into(self) -> json::Kdf {
        match self {
            Kdf::Pbkdf2(params) => json::Kdf::Pbkdf2(params.into()),
            Kdf::Scrypt(params) => json::Kdf::Scrypt(params.into()),
        }
    }
}

impl rlp::Decodable for Kdf {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        let kdf: String = d.val_at(2)?;

        d.at(5)?.decoder().decode_value(|bytes| {
            let unwrapped = UntrustedRlp::new(bytes);
            let c = unwrapped.val_at(0)?;
            let dklen = unwrapped.val_at(1)?;
            let n = unwrapped.val_at(2)?;
            let p = unwrapped.val_at(3)?;
            let r = unwrapped.val_at(4)?;

            let val: String = unwrapped.val_at(5)?;
            let salt_vec = val.from_hex().expect("unexpected salt");
            let mut salt = [0u8; 32];
            salt.copy_from_slice(&salt_vec);

            match kdf.as_str() {
                "pbkdf2" => {
                    Ok(Kdf::Pbkdf2(Pbkdf2 {
                        c: c,
                        dklen: dklen,
                        prf: Prf::HmacSha256,
                        salt: salt,
                    }))
                }
                "scrypt" => {
                    Ok(Kdf::Scrypt(Scrypt {
                        dklen: dklen,
                        p: p,
                        n: n,
                        r: r,
                        salt: salt,
                    }))
                }
                _ => Err(DecoderError::Custom("invalid dkf type.")),
            }
        })
    }
}

impl rlp::Encodable for Kdf {
    fn rlp_append(&self, s: &mut RlpStream) {
        let mut stream = RlpStream::default();
        stream.begin_list(6);
        match self {
            Kdf::Pbkdf2(params) => {
                stream.append(&params.c);
                stream.append(&params.dklen);
                stream.append(&0_u8);
                stream.append(&0_u8);
                stream.append(&0_u8);
                stream.append(&params.salt.to_hex());
            }
            Kdf::Scrypt(params) => {
                stream.append(&0_u8);
                stream.append(&params.dklen);
                stream.append(&params.n);
                stream.append(&params.p);
                stream.append(&params.r);
                stream.append(&params.salt.to_hex());
            }
        }
        s.append_internal(&stream.as_raw());
    }
}
