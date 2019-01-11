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
//#![feature(test)]
//extern crate test;

extern crate time;
extern crate byteorder;
extern crate crypto as rcrypto;
extern crate aion_types;
extern crate rand;
extern crate rustc_hex;
extern crate blake2b as blake2b_util;
extern crate rlp;

mod error;
mod ed25519;
mod blake2b;

pub use self::error::Error;
pub use self::ed25519::signature_ed25519::{recover_ed25519, sign_ed25519, verify_signature_ed25519, Ed25519Signature};
pub use self::ed25519::secret_ed25519::Ed25519Secret;
pub use self::ed25519::keypair_ed25519::{generate_keypair, Ed25519KeyPair, public_to_address_ed25519};
pub use aion_types::{Address, H256, Ed25519Public};

pub type Message = H256;

/// Uninstantiatable error type for infallible generators.
#[derive(Debug)]
pub enum Void {}

/// Generates new keypair.
pub trait Generator {
    type Error;
}
