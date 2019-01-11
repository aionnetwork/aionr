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

use std::fmt;
use std::ptr;
use rand::{Rng, OsRng};
use rustc_hex::ToHex;
use blake2b::Blake2b;
use aion_types::{H256, Ed25519Public};
use Ed25519Secret;
use Address;
use Error;
use rcrypto::ed25519::keypair;

pub fn public_to_address_ed25519(public: &Ed25519Public) -> Address {
    let hash = public.blake2b();
    let mut result = Address::default();
    result.copy_from_slice(&hash[..]);
    result.0[0] = 0xA0;
    result
}

pub fn generate_keypair() -> Ed25519KeyPair {
    let mut rng = OsRng::new().unwrap();
    let seed = random_32_bytes(&mut rng);
    let (sk, pk) = keypair(&seed);

    Ed25519KeyPair {
        secret: Ed25519Secret::from_slice(&sk).unwrap(),
        public: Ed25519Public::from_slice(&pk),
    }
}

fn random_32_bytes(rng: &mut OsRng) -> [u8; 32] {
    let mut ret = [0u8; 32];
    rng.fill_bytes(&mut ret);
    ret
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ed25519KeyPair {
    secret: Ed25519Secret,
    public: Ed25519Public,
}

impl fmt::Display for Ed25519KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "secret:  {}", self.secret.to_hex())?;
        writeln!(f, "public:  {}", self.public.to_hex())?;
        write!(f, "address: {}", self.address().to_hex())
    }
}

impl From<Vec<u8>> for Ed25519KeyPair {
    fn from(s: Vec<u8>) -> Self {
        let mut pk = H256::default();
        pk.clone_from_slice(&s[64..96]);
        Ed25519KeyPair {
            secret: Ed25519Secret::from_slice(&s[..64]).unwrap(),
            public: pk,
        }
    }
}

impl Ed25519KeyPair {
    pub fn secret(&self) -> &Ed25519Secret { &self.secret }

    pub fn public(&self) -> &Ed25519Public { &self.public }

    pub fn address(&self) -> Address { public_to_address_ed25519(&self.public) }

    pub fn from_secret(secret: Ed25519Secret) -> Result<Ed25519KeyPair, Error> {
        if secret.0.len() != 64 {
            return Err(Error::InvalidSecret);
        }

        let mut pk = H256::default();
        let s = &(secret.0)[32..];

        unsafe {
            ptr::copy(s.as_ptr(), pk.as_mut_ptr(), 32);

            Ok(Ed25519KeyPair {
                secret: secret.clone(),
                public: pk,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use {Ed25519KeyPair, Ed25519Secret};
    use std::time::Instant;
    use super::*;

    #[test]
    fn from_secret() {
        let secret = Ed25519Secret::from_str("7ea8af7d0982509cd815096d35bc3a295f57b2a078e4e25731e3ea977b9544626702b86f33072a55f46003b1e3e242eb18556be54c5ab12044c3c20829e0abb5").unwrap();
        let _ = Ed25519KeyPair::from_secret(secret).unwrap();
    }

    #[test]
    fn keypair_display() {
        let expected =
"secret:  7ea8af7d0982509cd815096d35bc3a295f57b2a078e4e25731e3ea977b9544626702b86f33072a55f46003b1e3e242eb18556be54c5ab12044c3c20829e0abb5
public:  6702b86f33072a55f46003b1e3e242eb18556be54c5ab12044c3c20829e0abb5
address: a07bfd7baa8497fd43258a5442a26f277206f62a98668ae2212ab3f4c71a10c8".to_owned();
        let secret = Ed25519Secret::from_str("7ea8af7d0982509cd815096d35bc3a295f57b2a078e4e25731e3ea977b9544626702b86f33072a55f46003b1e3e242eb18556be54c5ab12044c3c20829e0abb5").unwrap();
        let kp = Ed25519KeyPair::from_secret(secret).unwrap();
        assert_eq!(format!("{}", kp), expected);
    }

    #[test]
    pub fn benchtest_generate_keypair() {
        let mut keypair = generate_keypair();
        let count = 1000;

        // warm up
        for _ in 0..count {
            keypair = generate_keypair();
        }

        let time = Instant::now();

        for _ in 0..count {
            keypair = generate_keypair();
        }

        let took = time.elapsed();

        println!(
            "[benchtest_generate_keypair] Ed25519 generate keypair(ns/call): {}",
            (took.as_secs() * 1000_000_000 + took.subsec_nanos() as u64) / count
        );
        assert!(!keypair.public.is_zero());
    }
}
