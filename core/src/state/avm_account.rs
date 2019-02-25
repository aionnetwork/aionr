
use std::sync::Arc;
use std::collections::{HashMap};
use blake2b::{BLAKE2B_EMPTY, BLAKE2B_NULL_RLP, blake2b};
use aion_types::{H256, U256, H128, U128, Address};
use bytes::{Bytes, ToPretty};
use trie;
use trie::{SecTrieDB, Trie, TrieFactory, TrieError};
use pod_account::*;
use rlp::*;
use lru_cache::LruCache;
use basic_account::BasicAccount;
use kvdb::{HashStore};

use std::cell::{RefCell, Cell};
use super::backend::Backend;
use super::{RequireCache, AccountState};

const STORAGE_CACHE_ITEMS: usize = 8192;

/// Boolean type for clean/dirty status.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Filth {
    /// Data has not been changed.
    Clean,
    /// Data has been changed.
    Dirty,
}

pub struct AVMAccount {
    // Balance of the account.
    balance: U256,
    // Nonce of the account.
    nonce: U256,
    // Trie-backed storage.
    storage_root: H256,
    // LRU Cache of the trie-backed storage.
    // This is limited to `STORAGE_CACHE_ITEMS` recent queries
    storage_cache: RefCell<LruCache<Bytes, Bytes>>,
    // Modified storage. Accumulates changes to storage made in `set_storage`
    // Takes precedence over `storage_cache`.
    storage_changes: HashMap<Bytes, Bytes>,

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

impl From<BasicAccount> for AVMAccount {
    fn from(basic: BasicAccount) -> Self {
        AVMAccount {
            balance: basic.balance,
            nonce: basic.nonce,
            storage_root: basic.storage_root,
            storage_cache: Self::empty_storage_cache(),
            storage_changes: HashMap::new(),
            code_hash: basic.code_hash,
            code_size: None,
            code_cache: Arc::new(vec![]),
            code_filth: Filth::Clean,
            address_hash: Cell::new(None),
        }
    }
}

impl<'a> AVMAccount {
    /// Create a new account with the given balance.
    pub fn new_basic(balance: U256, nonce: U256) -> AVMAccount {
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

    /// Create a new account from RLP.
    pub fn from_rlp(rlp: &[u8]) -> AVMAccount {
        let basic: BasicAccount = ::rlp::decode(rlp);
        basic.into()
    }
}

pub struct AVMAccountEntry {
    account: Option<AVMAccount>,
    old_balance: Option<U256>,
    state: AccountState,
}

impl AVMAccountEntry {
    pub fn new_clean(account: Option<AVMAccount>) -> AVMAccountEntry {
        AVMAccountEntry {
            old_balance: account.as_ref().map(|a| a.balance().clone()),
            account: account,
            state: AccountState::CleanFresh,
        }
    }
}

impl AVMAccount
{
    fn balance(&self) -> &U256 {
        return &self.balance;
    }

    pub fn cache_given_code(&mut self, code: Arc<Bytes>) {
        trace!(
            target: "account",
            "Account::cache_given_code: ic={}; self.code_hash={:?}, self.code_cache={}",
            self.is_cached(),
            self.code_hash,
            self.code_cache.pretty()
        );

        self.code_size = Some(code.len());
        self.code_cache = code;
    }

    /// Provide a database to get `code_size`. Should not be called if it is a contract without code.
    pub fn cache_code_size(&mut self, db: &HashStore) -> bool {
        // TODO: fill out self.code_cache;
        trace!(
            target: "account",
            "Account::cache_code_size: ic={}; self.code_hash={:?}, self.code_cache={}",
            self.is_cached(),
            self.code_hash,
            self.code_cache.pretty()
        );
        self.code_size.is_some() || if self.code_hash != BLAKE2B_EMPTY {
            match db.get(&self.code_hash) {
                Some(x) => {
                    self.code_size = Some(x.len());
                    true
                }
                _ => {
                    warn!(target: "account","Failed reverse get of {}", self.code_hash);
                    false
                }
            }
        } else {
            false
        }
    }

    pub fn address_hash(&self, address: &Address) -> H256 {
        let hash = self.address_hash.get();
        hash.unwrap_or_else(|| {
            let hash = blake2b(address);
            self.address_hash.set(Some(hash.clone()));
            hash
        })
    }

    pub fn code_hash(&self) -> H256 { self.code_hash.clone() }

    pub fn is_cached(&self) -> bool {
        !self.code_cache.is_empty()
            || (self.code_cache.is_empty() && self.code_hash == BLAKE2B_EMPTY)
    }

    pub fn cache_code(&mut self, db: &HashStore) -> Option<Arc<Bytes>> {
        // TODO: fill out self.code_cache;
        trace!(
            target: "account",
            "Account::cache_code: ic={}; self.code_hash={:?}, self.code_cache={}",
            self.is_cached(),
            self.code_hash,
            self.code_cache.pretty()
        );

        if self.is_cached() {
            return Some(self.code_cache.clone());
        }

        match db.get(&self.code_hash) {
            Some(x) => {
                self.code_size = Some(x.len());
                self.code_cache = Arc::new(x.into_vec());
                Some(self.code_cache.clone())
            }
            _ => {
                warn!(target: "account","Failed reverse get of {}", self.code_hash);
                None
            }
        }
    }

    pub fn cached_storage_at(&self, key: &Vec<u8>) -> Option<Vec<u8>> {
        if let Some(value) = self.storage_changes.get(key) {
            return Some(value.clone());
        }
        if let Some(value) = self.storage_cache.borrow_mut().get_mut(key) {
            return Some(value.clone());
        }
        None
    }

    fn empty_storage_cache() -> RefCell<LruCache<Vec<u8>, Vec<u8>>> {
        RefCell::new(LruCache::new(STORAGE_CACHE_ITEMS))
    }

    pub fn storage_at(&self, db: &HashStore, key: &Vec<u8>) -> trie::Result<Vec<u8>> {
        if let Some(value) = self.cached_storage_at(key) {
            return Ok(value);
        }
        let db = SecTrieDB::new(db, &self.storage_root)?;

        let value: Vec<u8> = db.get_with(key, ::rlp::decode)?.unwrap_or_else(|| vec![]);
        self.storage_cache
            .borrow_mut()
            .insert(key.clone(), value.clone());
        Ok(value)
    }
}

pub struct AVMAccMgr {
    pub cache: RefCell<HashMap<Address, AVMAccount>>,
}

impl AVMAccMgr {
    pub fn new() -> Self {
        AVMAccMgr {
            cache: RefCell::new(HashMap::new()),
        }
    }
    pub fn new_account(&mut self, address: &Address) {
        self.cache.borrow_mut().insert(*address, AVMAccount::new_basic(0.into(), 0.into()));
    }
}

pub trait AVMInterface {
    fn new_avm_account(&mut self, a: &Address) -> trie::Result<()>;
    fn check_avm_acc_exists(&self, a: &Address) -> trie::Result<bool>;
    fn set_avm_storage(&mut self, a: &Address, key: &Vec<u8>, value: Vec<u8>) -> trie::Result<()>;
    fn get_avm_storage(&self, a: &Address, key: &Vec<u8>) -> trie::Result<Vec<u8>>;
    fn remove_avm_account(&mut self, a: &Address) -> trie::Result<()>;
    fn ensure_avm_cached<F, U>(
        &self,
        a: &Address,
        require: RequireCache,
        check_null: bool,
        f: F,
    ) -> trie::Result<U>
    where
        F: Fn(Option<&AVMAccount>) -> U;
}