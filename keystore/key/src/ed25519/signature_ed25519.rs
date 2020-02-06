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

use std::ops::Deref;
use std::cmp::PartialEq;
use std::fmt;
use std::str::FromStr;
use std::hash::{Hash, Hasher};
use rustc_hex::{ToHex, FromHex};
use aion_types::{H256, H768};
use {Error, Message};
use Ed25519Secret;
use aion_types::Ed25519Public;
use rcrypto::ed25519::{signature, verify};

pub struct Ed25519Signature([u8; 96]);

impl Ed25519Signature {
    pub fn get_public(&self) -> Ed25519Public { H256::from_slice(&self.0[..32]) }

    pub fn get_signature(&self) -> &[u8] { &self.0[0..96] }
}

impl PartialEq for Ed25519Signature {
    fn eq(&self, other: &Self) -> bool { self.0.iter().zip(other.0.iter()).all(|(a, b)| a == b) }
}

// also manual for the same reason, but the pretty printing might be useful.
impl fmt::Debug for Ed25519Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("Ed25519Signature")
            .field("sig", &self.0[32..96].to_hex())
            .finish()
    }
}

impl fmt::Display for Ed25519Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.0.to_hex())
    }
}

impl FromStr for Ed25519Signature {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.from_hex() {
            Ok(ref hex) if hex.len() == 96 => {
                let mut data = [0; 96];
                data.copy_from_slice(&hex[0..96]);
                Ok(Ed25519Signature(data))
            }
            _ => Err(Error::InvalidSignature),
        }
    }
}

impl Default for Ed25519Signature {
    fn default() -> Self { Ed25519Signature([0u8; 96]) }
}

impl Hash for Ed25519Signature {
    fn hash<H: Hasher>(&self, state: &mut H) { H768::from(self.0).hash(state); }
}

impl From<Vec<u8>> for Ed25519Signature {
    fn from(s: Vec<u8>) -> Self {
        let mut sig = [0u8; 96];
        sig.copy_from_slice(&s);
        Ed25519Signature(sig)
    }
}

impl Into<[u8; 96]> for Ed25519Signature {
    fn into(self) -> [u8; 96] { self.0.clone() }
}

impl Deref for Ed25519Signature {
    type Target = [u8; 96];

    fn deref(&self) -> &Self::Target { &self.0 }
}

pub fn sign_ed25519(key: &Ed25519Secret, message: &Message) -> Result<Ed25519Signature, Error> {
    let sig = signature(message, &key.0).to_vec();
    let mut result = [0u8; 96];
    result[0..32].copy_from_slice(&key.0[32..]);
    result[32..96].copy_from_slice(&sig);

    Ok(Ed25519Signature(result))
}

pub fn verify_signature_ed25519(
    pk: Ed25519Public,
    sig: Ed25519Signature,
    message: &Message,
) -> bool
{
    let signature = &sig.0[32..96];
    verify(message, &pk, signature)
}

pub fn recover_ed25519(
    signature: &Ed25519Signature,
    message: &Message,
) -> Result<Ed25519Public, Error>
{
    let pk = &signature.0[..32];
    let sig = &signature.0[32..96];

    match verify(message, pk, &sig) {
        true => {
            let p: Ed25519Public = H256::from(pk);
            Ok(p)
        }
        false => Err(Error::InvalidSignature),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    #[cfg(feature = "benches")]
    use std::time::Instant;
    use Message;
    use rcrypto::ed25519::verify;
    use generate_keypair;
    use super::{sign_ed25519, Ed25519Signature};

    #[test]
    fn signature_to_and_from_str() {
        let keypair = generate_keypair();
        let message = Message::default();
        let signature = sign_ed25519(&keypair.secret(), &message).unwrap();
        let string = format!("{}", signature);
        let deserialized = Ed25519Signature::from_str(&string).unwrap();
        assert_eq!(signature, deserialized);
    }

    #[test]
    fn sign_and_verify_public() {
        let keypair = generate_keypair();
        let message = Message::default();
        let signature = sign_ed25519(&keypair.secret(), &message).unwrap();
        let sig = &signature.0[32..96];

        assert!(verify(&message, &keypair.public().0, sig));
    }

    #[test]
    #[cfg(feature = "benches")]
    pub fn benchtest_sign_ed25519() {
        let keypair = generate_keypair();
        let message =
            Message::from_str("a6697e974e6a320f454390be03f74955e8978f1a6971ea6730542e37b66179bc")
                .unwrap();
        let mut signature = sign_ed25519(&keypair.secret(), &message).unwrap();

        let count = 1000;

        // warm up
        let time = Instant::now();

        for _ in 0..count {
            signature = sign_ed25519(&keypair.secret(), &message).unwrap();
        }

        let took = time.elapsed();

        println!(
            "[benchtest_sign_ed25519] Ed25519 sign message(ns/call): {}",
            (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
        );

        assert!(!signature.is_empty());
    }

    #[test]
    #[cfg(feature = "benches")]
    pub fn benchtest_verify_ed25519() {
        let keypair = generate_keypair();
        let message =
            Message::from_str("a6697e974e6a320f454390be03f74955e8978f1a6971ea6730542e37b66179bc")
                .unwrap();
        let signature = sign_ed25519(&keypair.secret(), &message).unwrap();
        let sig = &signature.0[32..96];

        let count = 1000;
        let time = Instant::now();

        let mut result = false;

        for _ in 0..count {
            result = verify(&message, &keypair.public().0, sig);
        }

        let took = time.elapsed();

        println!(
            "[benchtest_verify_ed25519] Ed25519 verify message(ns/call): {}",
            (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
        );

        assert!(result);
    }
}
