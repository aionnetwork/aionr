use std::fmt;
use std::sync::Arc;
use std::collections::{HashMap, BTreeMap};
use blake2b::{BLAKE2B_EMPTY, BLAKE2B_NULL_RLP, blake2b};
use aion_types::{H256, U256, H128, U128, Address};
use kvdb::{DBValue, HashStore};
use bytes::{Bytes, ToPretty};
use trie;
use trie::{SecTrieDB, Trie, TrieFactory, TrieError};
use pod_account::*;
use rlp::*;
use lru_cache::LruCache;
use basic_account::BasicAccount;

use std::cell::{RefCell, Cell};

const STORAGE_CACHE_ITEMS: usize = 8192;

/// Boolean type for clean/dirty status.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Filth {
    /// Data has not been changed.
    Clean,
    /// Data has been changed.
    Dirty,
}

pub struct AVMAccount<'a> {
    // Balance of the account.
    balance: U256,
    // Nonce of the account.
    nonce: U256,
    // Trie-backed storage.
    storage_root: H256,
    // LRU Cache of the trie-backed storage.
    // This is limited to `STORAGE_CACHE_ITEMS` recent queries
    storage_cache: RefCell<LruCache<H128, H128>>,
    // Modified storage. Accumulates changes to storage made in `set_storage`
    // Takes precedence over `storage_cache`.
    storage_changes: HashMap<&'a [u8], &'a [u8]>,

    // Code hash of the account.
    code_hash: H256,
    // Size of the accoun code.
    code_size: Option<usize>,
    // Code cache of the account.
    code_cache: Arc<Bytes>,
    // Account code new or has been modified.
    code_filth: Filth,
    // Cached address hash.
    address_hash: Cell<Option<H256>>,
}

impl<'a> AVMAccount<'a> {
    /// Create a new account with the given balance.
    pub fn new_basic(balance: U256, nonce: U256) -> AVMAccount<'a> {
        AVMAccount {
            balance: balance,
            nonce: nonce,
            storage_root: BLAKE2B_NULL_RLP,
            storage_cache: Self::empty_storage_cache(),
            storage_changes: HashMap::new(),
            code_hash: BLAKE2B_EMPTY,
            code_cache: Arc::new(vec![]),
            code_size: Some(0),
            code_filth: Filth::Clean,
            address_hash: Cell::new(None),
        }
    }
}

impl<'a> AVMAccount<'a> {
    fn empty_storage_cache() -> RefCell<LruCache<H128, H128>> {
        RefCell::new(LruCache::new(STORAGE_CACHE_ITEMS))
    }
}
