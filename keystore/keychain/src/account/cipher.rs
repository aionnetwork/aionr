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

use crate::json;
use rlp::{self, UntrustedRlp, DecoderError};
use rustc_hex::FromHex;

#[derive(Debug, PartialEq, Clone)]
pub struct Aes128Ctr {
    pub iv: [u8; 16],
}

#[derive(Debug, PartialEq, Clone)]
pub enum Cipher {
    Aes128Ctr(Aes128Ctr),
}

impl From<json::Aes128Ctr> for Aes128Ctr {
    fn from(json: json::Aes128Ctr) -> Self {
        Aes128Ctr {
            iv: json.iv.into(),
        }
    }
}

impl Into<json::Aes128Ctr> for Aes128Ctr {
    fn into(self) -> json::Aes128Ctr {
        json::Aes128Ctr {
            iv: From::from(self.iv),
        }
    }
}

impl From<json::Cipher> for Cipher {
    fn from(json: json::Cipher) -> Self {
        match json {
            json::Cipher::Aes128Ctr(params) => Cipher::Aes128Ctr(From::from(params)),
        }
    }
}

impl Into<json::Cipher> for Cipher {
    fn into(self) -> json::Cipher {
        match self {
            Cipher::Aes128Ctr(params) => json::Cipher::Aes128Ctr(params.into()),
        }
    }
}

impl rlp::Decodable for Cipher {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        let cipher: String = d.val_at(0)?;
        d.at(4)?.decoder().decode_value(|bytes| {
            let unwrapped = UntrustedRlp::new(bytes);
            let cipher_params: String = unwrapped.val_at(0)?;
            match cipher.as_str() {
                "aes-128-ctr" => {
                    let mut data = [0; 16];
                    data.copy_from_slice(
                        &cipher_params
                            .from_hex()
                            .map_err(|_| DecoderError::Custom("aes-128-ctr parse error"))?,
                    );
                    Ok(Cipher::Aes128Ctr(Aes128Ctr {
                        iv: data,
                    }))
                }
                _ => Err(DecoderError::Custom("Invalid cipher type.")),
            }
        })
    }
}
