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

//! Contract interface specification.

mod bytes;
mod cipher;
mod crypto;
mod error;
mod hash;
mod id;
mod kdf;
mod key_file;
mod vault_file;
mod vault_key_file;
mod version;

pub use self::bytes::Bytes;
pub use self::cipher::{Cipher, CipherSer, CipherSerParams, Aes128Ctr};
pub use self::crypto::{Crypto, CipherText};
pub use self::error::Error;
pub use self::hash::{H128, H160, H256};
pub use self::id::Uuid;
pub use self::kdf::{Kdf, KdfSer, Prf, Pbkdf2, Scrypt, KdfSerParams};
pub use self::key_file::{KeyFile, OpaqueKeyFile};
pub use self::vault_file::VaultFile;
pub use self::vault_key_file::{VaultKeyFile, VaultKeyMeta, insert_vault_name_to_json_meta, remove_vault_name_from_json_meta};
pub use self::version::Version;
