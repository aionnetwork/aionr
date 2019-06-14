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

use std::str;
use rustc_hex::{FromHex, ToHex};
use {json, Error, crypto};
use blake2b::blake2b;
use random::Random;
use smallvec::SmallVec;
use account::{Cipher, Kdf, Aes128Ctr, Pbkdf2, Prf};
use rlp::{self, RlpStream, UntrustedRlp, DecoderError};
// use subtle;
use key::Ed25519Secret;

/// Encrypted data
#[derive(Debug, PartialEq, Clone)]
pub struct Crypto {
    /// Encryption parameters
    pub cipher: Cipher,
    /// Encrypted data buffer
    pub ciphertext: Vec<u8>,
    /// Key derivation function parameters
    pub kdf: Kdf,
    /// Message authentication code
    pub mac: [u8; 32],
}

impl From<json::Crypto> for Crypto {
    fn from(json: json::Crypto) -> Self {
        Crypto {
            cipher: json.cipher.into(),
            ciphertext: json.ciphertext.into(),
            kdf: json.kdf.into(),
            mac: json.mac.into(),
        }
    }
}

impl From<Crypto> for json::Crypto {
    fn from(c: Crypto) -> Self {
        json::Crypto {
            cipher: c.cipher.into(),
            ciphertext: c.ciphertext.into(),
            kdf: c.kdf.into(),
            mac: c.mac.into(),
        }
    }
}

impl str::FromStr for Crypto {
    type Err = <json::Crypto as str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> { s.parse::<json::Crypto>().map(Into::into) }
}

impl From<Crypto> for String {
    fn from(c: Crypto) -> Self { json::Crypto::from(c).into() }
}

impl Crypto {
    /// Encrypt account secret for ed25519
    pub fn with_secret_ed25519(secret: &Ed25519Secret, password: &str, iterations: u32) -> Self {
        Crypto::with_plain(&*secret as &[u8], password, iterations)
    }

    /// Encrypt custom plain data
    pub fn with_plain(plain: &[u8], password: &str, iterations: u32) -> Self {
        let salt: [u8; 32] = Random::random();
        let iv: [u8; 16] = Random::random();

        // two parts of derived key
        // DK = [ DK[0..15] DK[16..31] ] = [derived_left_bits, derived_right_bits]
        let (derived_left_bits, derived_right_bits) =
            crypto::derive_key_iterations(password, &salt, iterations);

        // preallocated (on-stack in case of `Secret`) buffer to hold cipher
        // length = length(plain) as we are using CTR-approach
        let plain_len = plain.len();
        let mut ciphertext: SmallVec<[u8; 32]> = SmallVec::from_vec(vec![0; plain_len]);

        // aes-128-ctr with initial vector of iv
        crypto::aes::encrypt(&derived_left_bits, &iv, plain, &mut *ciphertext);

        // KECCAK(DK[16..31] ++ <ciphertext>), where DK[16..31] - derived_right_bits
        let mac = blake2b(crypto::derive_mac(&derived_right_bits, &*ciphertext));

        Crypto {
            cipher: Cipher::Aes128Ctr(Aes128Ctr {
                iv,
            }),
            ciphertext: ciphertext.into_vec(),
            kdf: Kdf::Pbkdf2(Pbkdf2 {
                dklen: crypto::KEY_LENGTH as u32,
                salt,
                c: iterations,
                prf: Prf::HmacSha256,
            }),
            mac: mac.0,
        }
    }

    /// Try to decrypt and convert result to account secret
    pub fn secret_ed25519(&self, password: &str) -> Result<Ed25519Secret, Error> {
        if self.ciphertext.len() > 64 {
            return Err(Error::InvalidSecret);
        }

        let secret = self.do_decrypt(password, 64)?;
        let key = Ed25519Secret::from_slice(&secret);

        match key {
            Some(k) => return Ok(k),
            None => return Err(Error::InvalidSecret),
        };
    }

    /// Try to decrypt and return result as is
    pub fn decrypt(&self, password: &str) -> Result<Vec<u8>, Error> {
        let expected_len = self.ciphertext.len();
        self.do_decrypt(password, expected_len)
    }

    fn do_decrypt(&self, password: &str, expected_len: usize) -> Result<Vec<u8>, Error> {
        let (derived_left_bits, derived_right_bits) = match self.kdf {
            Kdf::Pbkdf2(ref params) => {
                crypto::derive_key_iterations(password, &params.salt, params.c)
            }
            Kdf::Scrypt(ref params) => {
                crypto::derive_key_scrypt(password, &params.salt, params.n, params.p, params.r)?
            }
        };

        //        let mac  = blake2b(crypto::derive_mac(&derived_right_bits, &self.ciphertext));
        //        if subtle::slices_equal(&mac, &self.mac) == 0 {
        //            return Err(Error::InvalidPassword);
        //        }

        let mut plain: SmallVec<[u8; 32]> = SmallVec::from_vec(vec![0; expected_len]);

        match self.cipher {
            Cipher::Aes128Ctr(ref params) => {
                // checker by callers
                debug_assert!(expected_len >= self.ciphertext.len());

                let from = expected_len - self.ciphertext.len();
                crypto::aes::decrypt(
                    &derived_left_bits,
                    &params.iv,
                    &self.ciphertext,
                    &mut plain[from..],
                );
                Ok(plain.into_iter().collect())
            }
        }
    }
}

impl rlp::Decodable for Crypto {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        d.decoder().decode_value(|bytes| {
            let unwrapped = UntrustedRlp::new(bytes);
            Ok(Crypto {
                cipher: { unwrapped.as_val()? },
                ciphertext: {
                    let val: String = unwrapped.val_at(1)?;
                    val.from_hex()
                        .map_err(|_| DecoderError::Custom("ciphertext parse error"))?
                },
                kdf: unwrapped.as_val()?,
                mac: {
                    let val: String = unwrapped.val_at(3)?;
                    let mut result = [0; 32];
                    result.copy_from_slice(
                        &val.from_hex()
                            .map_err(|_| DecoderError::Custom("mac parse error"))?,
                    );
                    result
                },
            })
        })
    }
}

impl rlp::Encodable for Crypto {
    fn rlp_append(&self, s: &mut RlpStream) {
        let mut cipher_params = [0u8; 16];
        let kdf;
        match self.cipher {
            Cipher::Aes128Ctr(ref params) => {
                cipher_params.clone_from_slice(&params.iv);
            }
        }

        match self.kdf {
            Kdf::Pbkdf2(ref _params) => kdf = String::from("pbkdf2"),
            Kdf::Scrypt(ref _params) => kdf = String::from("scrypt"),
        }

        let mut stream = RlpStream::default();
        let mut stream_cipher = RlpStream::default();
        stream.begin_list(6);
        stream.append(&"aes-128-ctr");
        stream.append(&self.ciphertext.to_hex());
        stream.append(&kdf);
        stream.append(&self.mac.to_hex());

        stream_cipher.begin_list(1);
        stream_cipher.append(&cipher_params.to_hex());

        stream.append(&stream_cipher.as_raw());
        stream.append(&self.kdf);

        s.append_internal(&stream.as_raw());
    }
}