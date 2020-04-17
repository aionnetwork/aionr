#![warn(unused_extern_crates)]

extern crate rand;
extern crate rustc_serialize as serialize;
extern crate libc;
extern crate rustc_hex;

pub mod aessafe;
pub mod bcrypt;
pub mod bcrypt_pbkdf;
pub mod blockmodes;
pub mod blowfish;
pub mod buffer;
mod cryptoutil;
pub mod curve25519;
pub mod digest;
pub mod ed25519;
pub mod hkdf;
pub mod hmac;
pub mod mac;
pub mod md5;
pub mod pbkdf2;
pub mod scrypt;
pub mod sha1;
pub mod sha2;
pub mod sha3;
mod simd;
mod step_by;
pub mod symmetriccipher;
pub mod util;
pub mod vrf;

use std::fmt;
use pbkdf2::pbkdf2;
use scrypt::{scrypt, ScryptParams};
use sha2::Sha256;
use hmac::Hmac;

pub const KEY_LENGTH: usize = 32;
pub const KEY_ITERATIONS: usize = 10240;
pub const KEY_LENGTH_AES: usize = KEY_LENGTH / 2;

/// Default authenticated data to use (in RPC).
pub const DEFAULT_MAC: [u8; 2] = [0, 0];

#[derive(PartialEq, Debug)]
pub enum ScryptError {
    // log(N) < r / 16
    InvalidN,
    // p <= (2^31-1 * 32)/(128 * r)
    InvalidP,
}

impl fmt::Display for ScryptError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let s = match *self {
            ScryptError::InvalidN => "Invalid N argument of the scrypt encryption",
            ScryptError::InvalidP => "Invalid p argument of the scrypt encryption",
        };

        write!(f, "{}", s)
    }
}

#[derive(PartialEq, Debug)]
pub enum Error {
    Scrypt(ScryptError),
    InvalidMessage,
}

impl From<ScryptError> for Error {
    fn from(err: ScryptError) -> Self { Error::Scrypt(err) }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let s = match *self {
            Error::Scrypt(ref err) => err.to_string(),
            Error::InvalidMessage => "Invalid message".into(),
        };

        write!(f, "{}", s)
    }
}

impl Into<String> for Error {
    fn into(self) -> String { format!("{}", self) }
}

pub fn derive_key_iterations(password: &str, salt: &[u8; 32], c: u32) -> (Vec<u8>, Vec<u8>) {
    let mut h_mac = Hmac::new(Sha256::new(), password.as_bytes());
    let mut derived_key = vec![0u8; KEY_LENGTH];
    pbkdf2(&mut h_mac, salt, c, &mut derived_key);
    let derived_right_bits = &derived_key[0..KEY_LENGTH_AES];
    let derived_left_bits = &derived_key[KEY_LENGTH_AES..KEY_LENGTH];
    (derived_right_bits.to_vec(), derived_left_bits.to_vec())
}

pub fn derive_key_scrypt(
    password: &str,
    salt: &[u8; 32],
    n: u32,
    p: u32,
    r: u32,
) -> Result<(Vec<u8>, Vec<u8>), Error>
{
    // sanity checks
    let log_n = (32 - n.leading_zeros() - 1) as u8;
    if log_n as u32 >= r * 16 {
        return Err(Error::Scrypt(ScryptError::InvalidN));
    }

    if p as u64 > ((u32::max_value() as u64 - 1) * 32) / (128 * (r as u64)) {
        return Err(Error::Scrypt(ScryptError::InvalidP));
    }

    let mut derived_key = vec![0u8; KEY_LENGTH];
    let scrypt_params = ScryptParams::new(log_n, r, p);
    scrypt(password.as_bytes(), salt, &scrypt_params, &mut derived_key);
    let derived_right_bits = &derived_key[0..KEY_LENGTH_AES];
    let derived_left_bits = &derived_key[KEY_LENGTH_AES..KEY_LENGTH];
    Ok((derived_right_bits.to_vec(), derived_left_bits.to_vec()))
}

pub fn derive_mac(derived_left_bits: &[u8], cipher_text: &[u8]) -> Vec<u8> {
    let mut mac = vec![0u8; KEY_LENGTH_AES + cipher_text.len()];
    mac[0..KEY_LENGTH_AES].copy_from_slice(derived_left_bits);
    mac[KEY_LENGTH_AES..cipher_text.len() + KEY_LENGTH_AES].copy_from_slice(cipher_text);
    mac
}

/// AES encryption
pub mod aes {
    use blockmodes::{CtrMode, CbcDecryptor, PkcsPadding};
    use aessafe::{AesSafe128Encryptor, AesSafe128Decryptor};
    use symmetriccipher::{Encryptor, Decryptor, SymmetricCipherError};
    use buffer::{RefReadBuffer, RefWriteBuffer, WriteBuffer};

    /// Encrypt a message (CTR mode)
    pub fn encrypt(k: &[u8], iv: &[u8], plain: &[u8], dest: &mut [u8]) {
        let mut encryptor = CtrMode::new(AesSafe128Encryptor::new(k), iv.to_vec());
        encryptor
            .encrypt(
                &mut RefReadBuffer::new(plain),
                &mut RefWriteBuffer::new(dest),
                true,
            )
            .expect("Invalid length or padding");
    }

    /// Decrypt a message (CTR mode)
    pub fn decrypt(k: &[u8], iv: &[u8], encrypted: &[u8], dest: &mut [u8]) {
        let mut encryptor = CtrMode::new(AesSafe128Encryptor::new(k), iv.to_vec());
        encryptor
            .decrypt(
                &mut RefReadBuffer::new(encrypted),
                &mut RefWriteBuffer::new(dest),
                true,
            )
            .expect("Invalid length or padding");
    }

    /// Decrypt a message using cbc mode
    pub fn decrypt_cbc(
        k: &[u8],
        iv: &[u8],
        encrypted: &[u8],
        dest: &mut [u8],
    ) -> Result<usize, SymmetricCipherError>
    {
        let mut encryptor =
            CbcDecryptor::new(AesSafe128Decryptor::new(k), PkcsPadding, iv.to_vec());
        let len = dest.len();
        let mut buffer = RefWriteBuffer::new(dest);
        encryptor.decrypt(&mut RefReadBuffer::new(encrypted), &mut buffer, true)?;
        Ok(len - buffer.remaining())
    }
}

#[cfg(test)]
mod tests {
    // TODO: test cases
}
