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
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::collections::HashMap;

use aion_types::{H256, U256};
use acore_bytes::{Bytes};
use state::account::traits::AccType;

use lru_cache::LruCache;

#[derive(Clone, Debug, Hash, PartialEq)]
pub enum KeyTag {
    REMOVE,
    DIRTY(Bytes),
    CLEAN(Bytes),
}

impl KeyTag {
    pub fn is_removed(&self) -> bool { return *self == KeyTag::REMOVE; }

    // get the value from KeyTag
    pub fn get(&self) -> Option<Bytes> {
        match *self {
            KeyTag::REMOVE => None,
            KeyTag::DIRTY(ref v) | KeyTag::CLEAN(ref v) => Some(v.clone()),
        }
    }

    #[cfg(test)]
    pub(super) fn is_dirty(&self) -> bool {
        match *self {
            KeyTag::DIRTY(_) => true,
            _ => false,
        }
    }

    #[cfg(test)]
    pub(super) fn is_clean(&self) -> bool {
        match *self {
            KeyTag::CLEAN(_) => true,
            _ => false,
        }
    }
}

/// Basic account type.
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct BasicAccount {
    /// Nonce of the account.
    pub nonce: U256,
    /// Balance of the account.
    pub balance: U256,
    /// Storage root of the account.
    pub storage_root: H256,
    /// Code hash of the account.
    pub code_hash: H256,
}

pub type VMCache = RefCell<LruCache<Bytes, Bytes>>;
pub type VMStorageChange = HashMap<Bytes, KeyTag>;

/// Single account in the system.
/// Keeps track of changes to the code and storage.
/// The changes are applied in `commit_storage` and `commit_code`
#[derive(Clone)]
pub struct Account {
    pub balance: U256,

    pub nonce: U256,

    // trie-backed storage.
    pub storage_root: H256,

    // avm storage root
    pub delta_root: H256,

    // LRU cache of the trie-backed storage.
    // limited to `STORAGE_CACHE_ITEMS` recent queries
    pub storage_cache: VMCache,

    // modified storage. Accumulates changes to storage made in `set_storage`
    // takes precedence over `storage_cache`.
    pub storage_changes: VMStorageChange,

    // code hash of the account.
    pub code_hash: H256,

    // size of the account code.
    pub code_size: Option<usize>,

    // code cache of the account.
    pub code_cache: Arc<Bytes>,

    // avm code hash of the account.
    pub transformed_code_hash: H256,
    // avm size of the transformed code.
    pub transformed_code_size: Option<usize>,
    // avm specific code cache
    pub transformed_code_cache: Arc<Bytes>,

    // avm object graph
    pub object_graph_cache: Arc<Bytes>,
    pub object_graph_hash: H256,
    pub object_graph_size: Option<usize>,

    // account code new or has been modified.
    pub code_filth: Filth,

    // cached address hash.
    pub address_hash: Cell<Option<H256>>,

    // empty_flag: for Aion Java Kernel Only
    pub empty_but_commit: bool,

    // account type: 0x01 = EVM; 0x02 = AVM
    pub account_type: AccType,
}

/// Boolean type for clean/dirty status.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Filth {
    /// Data has not been changed.
    Clean,
    /// Data has been changed.
    Dirty,
}
