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

mod generic;
mod traits;

use lru_cache::LruCache;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, BTreeMap, HashSet};
use std::sync::Arc;
use std::fmt;

use aion_types::{H256, U256, Address};
use bytes::{Bytes, ToPretty};
use self::generic::{Filth, BasicAccount};
use blake2b::{BLAKE2B_EMPTY, BLAKE2B_NULL_RLP, blake2b};
use rlp::*;
use pod_account::*;
use trie;
use trie::{Trie, SecTrieDB, TrieFactory, TrieError};

use kvdb::{DBValue, HashStore};

pub use self::generic::Account;
pub use self::traits::{VMAccount, AccType};
use state::Backend;

const STORAGE_CACHE_ITEMS: usize = 8192;

type VMCache = RefCell<LruCache<Bytes, Bytes>>;
type VMStorageChange = HashMap<Bytes, Bytes>;
pub type AionVMAccount = Account<VMCache, VMStorageChange>;

#[derive(Copy, Clone)]
pub enum RequireCache {
    None,
    CodeSize,
    Code,
}

impl AionVMAccount {
    fn empty_storage_cache() -> VMCache { RefCell::new(LruCache::new(STORAGE_CACHE_ITEMS)) }

    fn empty_storage_change() -> VMStorageChange { HashMap::new() }
}

impl From<BasicAccount> for AionVMAccount {
    fn from(basic: BasicAccount) -> Self {
        AionVMAccount {
            balance: basic.balance,
            nonce: basic.nonce,
            storage_root: basic.storage_root,
            delta_root: basic.storage_root,
            storage_cache: Self::empty_storage_cache(),
            storage_changes: HashMap::new(),
            code_hash: basic.code_hash,
            code_size: None,
            code_cache: Arc::new(vec![]),
            transformed_code_hash: BLAKE2B_EMPTY,
            transformed_code_size: None,
            transformed_code_cache: Arc::new(vec![]),
            objectgraph_hash: BLAKE2B_EMPTY,
            object_graph_size: None,
            object_graph_cache: Arc::new(vec![]),
            code_filth: Filth::Clean,
            address_hash: Cell::new(None),
            empty_but_commit: false,
            account_type: AccType::FVM,
            storage_removable: HashSet::new(),
        }
    }
}

impl AionVMAccount {
    pub fn new_contract(balance: U256, nonce: U256) -> Self {
        Self {
            balance: balance,
            nonce: nonce,
            storage_root: BLAKE2B_NULL_RLP,
            delta_root: BLAKE2B_NULL_RLP,
            storage_cache: Self::empty_storage_cache(),
            storage_changes: Self::empty_storage_change(),
            code_hash: BLAKE2B_EMPTY,
            code_cache: Arc::new(vec![]),
            transformed_code_hash: BLAKE2B_EMPTY,
            transformed_code_size: None,
            transformed_code_cache: Arc::new(vec![]),
            objectgraph_hash: BLAKE2B_EMPTY,
            object_graph_size: None,
            object_graph_cache: Arc::new(vec![]),
            code_size: None,
            code_filth: Filth::Clean,
            address_hash: Cell::new(None),
            empty_but_commit: false,
            account_type: AccType::FVM,
            storage_removable: HashSet::new()
        }
    }

    pub fn new_basic(balance: U256, nonce: U256) -> Self {
        Self {
            balance: balance,
            nonce: nonce,
            storage_root: BLAKE2B_NULL_RLP,
            delta_root: BLAKE2B_NULL_RLP,
            storage_cache: Self::empty_storage_cache(),
            storage_changes: Self::empty_storage_change(),
            code_hash: BLAKE2B_EMPTY,
            code_cache: Arc::new(vec![]),
            transformed_code_hash: BLAKE2B_EMPTY,
            transformed_code_size: None,
            transformed_code_cache: Arc::new(vec![]),
            objectgraph_hash: BLAKE2B_EMPTY,
            object_graph_size: None,
            object_graph_cache: Arc::new(vec![]),
            code_size: Some(0),
            code_filth: Filth::Clean,
            address_hash: Cell::new(None),
            empty_but_commit: false,
            account_type: AccType::FVM,
            storage_removable: HashSet::new()
        }
    }

    pub fn from_pod(pod: PodAccount) -> Self {
        let mut storage_changes = HashMap::new();
        for item in pod.storage.into_iter() {
            storage_changes.insert(item.0[..].to_vec(), item.1[..].to_vec());
        }
        AionVMAccount {
            balance: pod.balance,
            nonce: pod.nonce,
            storage_root: BLAKE2B_NULL_RLP,
            delta_root: BLAKE2B_NULL_RLP,
            storage_cache: Self::empty_storage_cache(),
            storage_changes: storage_changes,
            code_hash: pod.code.as_ref().map_or(BLAKE2B_EMPTY, |c| blake2b(c)),
            code_filth: Filth::Dirty,
            code_size: Some(pod.code.as_ref().map_or(0, |c| c.len())),
            code_cache: Arc::new(pod.code.map_or_else(
                || {
                    warn!(target:"account","POD account with unknown code is being created! Assuming no code.");
                    vec![]
                },
                |c| c,
            )),
            transformed_code_hash: BLAKE2B_EMPTY,
            transformed_code_size: None,
            transformed_code_cache: Arc::new(vec![]),
            objectgraph_hash: BLAKE2B_EMPTY,
            object_graph_size: None,
            object_graph_cache: Arc::new(vec![]),
            address_hash: Cell::new(None),
            empty_but_commit: false,
            account_type: AccType::FVM,
            storage_removable: HashSet::new()
        }
    }

    fn storage_is_clean(&self) -> bool { self.storage_changes.is_empty() }

    /// Commit the `storage_changes` to the backing DB and update `storage_root`.
    pub fn commit_storage(
        &mut self,
        trie_factory: &TrieFactory,
        db: &mut HashStore,
    ) -> trie::Result<()>
    {
        let account_type = self.acc_type().clone();
        let mut t = trie_factory.from_existing(db, &mut self.storage_root)?;
        for (k, v) in self.storage_changes.drain() {
            // cast key and value to trait type,
            // so we can call overloaded `to_bytes` method
            let mut is_zero = true;
            for item in v.clone() {
                if item != 0x00 {
                    is_zero = false;
                    break;
                }
            }
            if account_type == AccType::AVM {
                // avm always commits storage in storage_changes
                // and removes storage in storage_removable
                is_zero = false;
                for k in self.storage_removable.drain() {
                    t.remove(&k)?;
                }
            }
            debug!(target: "vm", "CommitStorage: key = {:?}, value = {:?}, is_zero = {:?}", k, v, is_zero);
            // account just commit storage key/value pairs,
            // the real length of value should be dealed by caller
            match is_zero {
                true => t.remove(&k)?,
                false => t.insert(&k, &encode(&v))?,
            };

            self.storage_cache.borrow_mut().insert(k, v);
        }

        Ok(())
    }

    pub fn discard_storage_changes(&mut self) { self.storage_changes.clear(); }

    /// Return the storage overlay.
    pub fn storage_changes(&self) -> &VMStorageChange { &self.storage_changes }

    pub fn get_empty_but_commit(&mut self) -> bool { return self.empty_but_commit; }

    /// Clone basic account data
    pub fn clone_basic(&self) -> Self {
        Self {
            balance: self.balance.clone(),
            nonce: self.nonce.clone(),
            storage_root: self.storage_root.clone(),
            delta_root: self.delta_root.clone(),
            storage_cache: Self::empty_storage_cache(),
            storage_changes: Self::empty_storage_change(),
            code_hash: self.code_hash.clone(),
            code_size: self.code_size.clone(),
            code_cache: self.code_cache.clone(),
            transformed_code_hash: self.transformed_code_hash.clone(),
            transformed_code_size: self.transformed_code_size.clone(),
            transformed_code_cache: self.transformed_code_cache.clone(),
            objectgraph_hash: self.objectgraph_hash.clone(),
            object_graph_size: self.object_graph_size.clone(),
            object_graph_cache: self.object_graph_cache.clone(),
            code_filth: self.code_filth.clone(),
            address_hash: self.address_hash.clone(),
            empty_but_commit: self.empty_but_commit.clone(),
            account_type: self.account_type.clone(),
            storage_removable: HashSet::new()
        }
    }

    pub fn overwrite_with(&mut self, other: Self) {
        self.balance = other.balance;
        self.nonce = other.nonce;
        self.storage_root = other.storage_root;
        self.delta_root = other.delta_root;
        self.code_hash = other.code_hash;
        self.code_filth = other.code_filth;
        self.code_cache = other.code_cache;
        self.code_size = other.code_size;
        self.address_hash = other.address_hash;
        self.transformed_code_hash = other.transformed_code_hash;
        self.transformed_code_size = other.transformed_code_size;
        self.transformed_code_cache = other.transformed_code_cache;
        self.object_graph_size = other.object_graph_size;
        self.objectgraph_hash = other.objectgraph_hash;
        self.object_graph_cache = other.object_graph_cache;
        self.empty_but_commit = other.empty_but_commit;
        self.account_type = other.account_type;

        let mut cache = self.storage_cache.borrow_mut();
        for (k, v) in other.storage_cache.into_inner() {
            cache.insert(k.clone(), v.clone()); //TODO: cloning should not be required here
        }
        self.storage_changes = other.storage_changes;
        self.storage_removable = other.storage_removable;
    }

    /// Clone account data, dirty storage keys and cached storage keys.
    // fn clone_all(&self) -> Self {
    //     let mut account = self.clone_dirty();
    //     account.storage_cache = self.storage_cache.clone();
    //     account
    // }

    pub fn set_empty_but_commit(&mut self) { self.empty_but_commit = true; }
}

impl VMAccount for AionVMAccount {
    fn from_rlp(rlp: &[u8]) -> AionVMAccount {
        let basic: BasicAccount = ::rlp::decode(rlp);
        basic.into()
    }

    fn init_code(&mut self, code: Bytes) {
        self.code_hash = blake2b(&code);
        self.code_cache = Arc::new(code);
        self.code_size = Some(self.code_cache.len());
        self.code_filth = Filth::Dirty;
    }

    // for AVM account, this must be called, so update account type to AVM
    fn init_transformed_code(&mut self, code: Bytes) {
        self.transformed_code_hash = blake2b(&code);
        self.transformed_code_cache = Arc::new(code);
        self.transformed_code_size = Some(self.transformed_code_cache.len());
        self.code_filth = Filth::Dirty;
        self.account_type = AccType::AVM;
    }

    fn init_objectgraph(&mut self, data: Bytes) {
        self.account_type = AccType::AVM;
        self.objectgraph_hash = blake2b(&data);
        self.object_graph_cache = Arc::new(data);
    }

    fn objectgraph(&self) -> Option<Arc<Bytes>> {
        if self.object_graph_cache.is_empty() {
            return None;
        }

        Some(self.object_graph_cache.clone())
    }

    fn reset_code(&mut self, code: Bytes) { self.init_code(code); }

    fn balance(&self) -> &U256 { &self.balance }

    fn nonce(&self) -> &U256 { &self.nonce }

    fn code_hash(&self) -> H256 { self.code_hash.clone() }

    fn transformed_code_hash(&self) -> H256 { self.transformed_code_hash.clone() }

    fn objectgraph_hash(&self) -> H256 { self.objectgraph_hash.clone() }

    fn address_hash(&self, address: &Address) -> H256 {
        let hash = self.address_hash.get();
        hash.unwrap_or_else(|| {
            let hash = blake2b(address);
            self.address_hash.set(Some(hash.clone()));
            hash
        })
    }

    fn code(&self) -> Option<Arc<Bytes>> {
        if self.code_cache.is_empty() {
            return None;
        }

        Some(self.code_cache.clone())
    }

    fn transformed_code(&self) -> Option<Arc<Bytes>> {
        if self.transformed_code_cache.is_empty() {
            return None;
        }

        Some(self.transformed_code_cache.clone())
    }

    fn code_size(&self) -> Option<usize> { self.code_size.clone() }

    fn transformed_code_size(&self) -> Option<usize> { self.transformed_code_size.clone() }

    fn is_cached(&self) -> bool {
        !self.code_cache.is_empty()
            || (self.code_cache.is_empty() && self.code_hash == BLAKE2B_EMPTY)
    }

    fn is_transformed_cached(&self) -> bool {
        !self.transformed_code_cache.is_empty()
        // || (self.transformed_code_cache.is_empty() && self.transformed_code_hash == BLAKE2B_EMPTY)
    }

    fn is_objectgraph_cached(&self) -> bool {
        !self.object_graph_cache.is_empty()
        // || (self.object_graph_cache.is_empty() && self.objectgraph_hash == BLAKE2B_EMPTY)
    }

    fn cache_code(&mut self, db: &HashStore) -> Option<Arc<Bytes>> {
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
                // println!("Account: code cache = {:?}", x);
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

    fn cache_transformed_code(&mut self, db: &HashStore) -> Option<Arc<Bytes>> {
        if self.is_transformed_cached() {
            return Some(self.transformed_code_cache.clone());
        }

        match db.get(&blake2b(self.address_hash.get().unwrap().clone())) {
            Some(x) => {
                self.account_type = AccType::AVM;
                self.transformed_code_size = Some(x.len());
                self.transformed_code_cache = Arc::new(x.into_vec());
                Some(self.transformed_code_cache.clone())
            }
            _ => {
                warn!(target: "account","Failed reverse get of {}", self.transformed_code_hash);
                None
            }
        }
    }

    // objectgraph uses delta_root as key,
    // it is cached during updating account cache
    fn cache_objectgraph(&mut self, a: &Address, db: &HashStore) -> Option<Arc<Bytes>> {
        if let Some(root) = db.get(a) {
            self.storage_root = root[..].into();
            // if storage_root has been stored, it should be avm created account
            self.account_type = AccType::AVM;
            // always cache object graph and key/value storage root
            println!("try to get object graph from: {:?}", self.delta_root);
            match db.get(&self.delta_root) {
                Some(data) => {
                    self.object_graph_size = Some(data.len());
                    self.objectgraph_hash = blake2b(&data);
                    self.object_graph_cache = Arc::new(data[..].to_vec());
                    Some(self.object_graph_cache.clone())
                }
                None => {
                    self.object_graph_size = None;
                    self.objectgraph_hash = BLAKE2B_EMPTY;
                    None
                }
            }
        } else {
            None
        }
    }

    fn cache_given_code(&mut self, code: Arc<Bytes>) {
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

    fn cache_given_transformed_code(&mut self, code: Arc<Bytes>) {
        trace!(
            target: "account",
            "Account::cache_given_code: ic={}; self.code_hash={:?}, self.code_cache={}",
            self.is_transformed_cached(),
            self.transformed_code_hash,
            self.transformed_code_cache.pretty()
        );

        self.transformed_code_size = Some(code.len());
        self.transformed_code_cache = code;
    }

    fn cache_given_objectgraph(&mut self, data: Arc<Bytes>) {
        self.object_graph_size = Some(data.len());
        self.object_graph_cache = data;
    }

    fn cache_code_size(&mut self, db: &HashStore) -> bool {
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

    fn cache_transformed_code_size(&mut self, db: &HashStore) -> bool {
        self.transformed_code_size.is_some() || if self.transformed_code_hash != BLAKE2B_EMPTY {
            match db.get(&self.transformed_code_hash) {
                Some(x) => {
                    self.transformed_code_size = Some(x.len());
                    true
                }
                _ => {
                    warn!(target: "account","Failed reverse get of {}", self.transformed_code_hash);
                    false
                }
            }
        } else {
            false
        }
    }

    fn cache_objectgraph_size(&mut self, db: &HashStore) -> bool {
        self.object_graph_size.is_some() || if self.objectgraph_hash != BLAKE2B_EMPTY {
            match db.get(&self.objectgraph_hash) {
                Some(x) => {
                    self.object_graph_size = Some(x.len());
                    true
                }
                _ => {
                    warn!(target: "account","Failed reverse get of {}", self.objectgraph_hash);
                    false
                }
            }
        } else {
            false
        }
    }

    fn is_empty(&self) -> bool {
        assert!(
            self.storage_is_clean(),
            "Account::is_empty() may only legally be called when storage is clean."
        );
        self.is_null() && self.storage_root == BLAKE2B_NULL_RLP
    }

    fn is_null(&self) -> bool {
        debug!(target: "vm", "check null: balance = {:?}, nonce = {:?}, code_hash = {:?}",
            self.balance.is_zero(), self.nonce.is_zero(), self.code_hash == BLAKE2B_EMPTY);
        self.balance.is_zero() && self.nonce.is_zero() && self.code_hash == BLAKE2B_EMPTY
    }

    fn is_basic(&self) -> bool {
        self.code_hash == BLAKE2B_EMPTY && self.transformed_code_hash == BLAKE2B_EMPTY
    }

    fn storage_root(&self) -> Option<&H256> {
        if self.storage_is_clean() {
            Some(&self.storage_root)
        } else {
            None
        }
    }

    fn inc_nonce(&mut self) { self.nonce = self.nonce + U256::from(1u8); }

    /// Increase account balance.
    fn add_balance(&mut self, x: &U256) { self.balance = self.balance + *x; }

    /// Decrease account balance.
    /// Panics if balance is less than `x`
    fn sub_balance(&mut self, x: &U256) {
        assert!(self.balance >= *x);
        self.balance = self.balance - *x;
    }

    /// Commit any unsaved code. `code_hash` will always return the hash of the `code_cache` after this.
    fn commit_code(&mut self, db: &mut HashStore) {
        trace!(
            target: "account",
            "Commiting code of {:?} - {:?}, {:?}",
            self,
            self.code_filth == Filth::Dirty,
            self.code_cache.is_empty()
        );
        match (
            self.code_filth == Filth::Dirty,
            self.code_cache.is_empty(),
            self.transformed_code_cache.is_empty(),
        ) {
            (true, true, true) => {
                self.code_size = Some(0);
                self.transformed_code_size = Some(0);
                self.code_filth = Filth::Clean;
            }
            (true, false, true) => {
                db.emplace(
                    self.code_hash.clone(),
                    DBValue::from_slice(&*self.code_cache),
                );
                self.code_size = Some(self.code_cache.len());
                self.transformed_code_size = Some(0);
                self.code_filth = Filth::Clean;
            }
            (true, true, false) => {
                self.code_size = Some(0);
                db.emplace(
                    blake2b(self.address_hash.get().unwrap()),
                    DBValue::from_slice(&*self.transformed_code_cache),
                );
                self.transformed_code_size = Some(self.transformed_code_cache.len());
                self.code_filth = Filth::Clean;
            }
            (true, false, false) => {
                self.code_size = Some(self.code_cache.len());
                db.emplace(
                    self.code_hash.clone(),
                    DBValue::from_slice(&*self.code_cache),
                );

                // use blake2b(address_hash) as key of transformed code
                db.emplace(
                    blake2b(self.address_hash.get().unwrap()),
                    DBValue::from_slice(&*self.transformed_code_cache),
                );
                self.transformed_code_size = Some(self.transformed_code_cache.len());
                self.code_filth = Filth::Clean;
            }
            (false, _, _) => {}
        }
    }

    /// Export to RLP.
    fn rlp(&self) -> Bytes {
        let mut stream = RlpStream::new_list(4);
        stream.append(&self.nonce);
        stream.append(&self.balance);
        //let vm_type: AccType = self.acc_type().into();
        if self.acc_type() == AccType::AVM {
            debug!(target: "vm", "rlp encode using delta_root");
            stream.append(&self.delta_root);
        } else {
            stream.append(&self.storage_root);
        }
        stream.append(&self.code_hash);
        stream.out()
    }

    /// Clone account data and dirty storage keys
    fn clone_dirty(&self) -> Self {
        let mut account = self.clone_basic();
        account.storage_changes = self.storage_changes.clone();
        account.code_cache = self.code_cache.clone();
        account.transformed_code_cache = self.transformed_code_cache.clone();
        account
    }

    fn acc_type(&self) -> AccType { self.account_type.clone() }

    /// avm should update object graph cache
    /// at this moment, address_hash is always updated
    /// cache code
    fn update_account_cache<B: Backend>(
        &mut self,
        a: &Address,
        require: RequireCache,
        state_db: &B,
        db: &HashStore,
    )
    {
        if let Some(root) = db.get(a) {
            self.storage_root = root[..].into();
            // if storage_root has been stored, it should be avm created account
            self.account_type = AccType::AVM;
            // always cache object graph and key/value storage root
            debug!(target: "vm", "try to get object graph from: {:?}", self.delta_root);
            match db.get(&self.delta_root) {
                Some(data) => {
                    self.object_graph_size = Some(data.len());
                    self.objectgraph_hash = blake2b(&data);
                    self.object_graph_cache = Arc::new(data[..].to_vec());
                }
                None => {
                    self.object_graph_size = None;
                    self.objectgraph_hash = BLAKE2B_EMPTY;
                }
            }
            debug!(target: "vm", "object graph = {:?}", self.object_graph_cache);
        }

        if let RequireCache::None = require {
            return;
        }

        if self.is_cached() && self.is_transformed_cached() {
            return;
        }

        // println!("Account: update code cache");
        // if there's already code in the global cache, always cache it localy
        let hash = self.code_hash();
        match state_db.get_cached_code(&hash) {
            Some(code) => {
                self.cache_given_code(code);
            }
            None => {
                match require {
                    RequireCache::None => {}
                    RequireCache::Code => {
                        if let Some(code) = self.cache_code(db) {
                            // propagate code loaded from the database to
                            // the global code cache.
                            state_db.cache_code(hash, code)
                        }
                    }
                    RequireCache::CodeSize => {
                        self.cache_code_size(db);
                    }
                }
            }
        }

        if self.account_type == AccType::AVM {
            // update transformed code cache
            let hash = blake2b(self.address_hash.get().unwrap());
            match state_db.get_cached_code(&hash) {
                Some(code) => self.cache_given_transformed_code(code),
                None => {
                    match require {
                        RequireCache::None => {}
                        RequireCache::Code => {
                            if let Some(code) = self.cache_transformed_code(db) {
                                // propagate code loaded from the database to
                                // the global code cache.
                                // println!("Account: transformed code = {:?}", code);
                                state_db.cache_code(hash, code)
                            }
                        }
                        RequireCache::CodeSize => {
                            self.cache_transformed_code_size(db);
                        }
                    }
                }
            }
        }
    }

    /// Prove a storage key's existence or nonexistence in the account's storage
    /// trie.
    /// `storage_key` is the hash of the desired storage key, meaning
    /// this will only work correctly under a secure trie.
    fn prove_storage(
        &self,
        db: &HashStore,
        storage_key: H256,
    ) -> Result<(Vec<Bytes>, H256), Box<TrieError>>
    {
        use trie::{Trie, TrieDB};
        use trie::recorder::Recorder;

        let mut recorder = Recorder::new();

        let trie = TrieDB::new(db, &self.storage_root)?;
        let item: U256 = {
            let query = (&mut recorder, ::rlp::decode);
            trie.get_with(&storage_key, query)?
                .unwrap_or_else(U256::zero)
        };

        Ok((
            recorder.drain().into_iter().map(|r| r.data).collect(),
            item.into(),
        ))
    }
}

impl AionVMAccount {
    pub fn storage_at(&self, db: &HashStore, key: &Bytes) -> trie::Result<Option<Bytes>> {
        if let Some(value) = self.cached_storage_at(key) {
            return Ok(Some(value));
        }
        let db = SecTrieDB::new(db, &self.storage_root)?;

        if self.acc_type() == AccType::AVM
            && !db.contains(key)? {
                return Ok(None);
            }

        let item: Bytes = db.get_with(key, ::rlp::decode)?.unwrap_or_else(|| vec![]);
        self.storage_cache
            .borrow_mut()
            .insert(key.clone(), item.clone());
        Ok(Some(item))
    }

    pub fn cached_storage_at(&self, key: &Bytes) -> Option<Bytes> {
        if let Some(value) = self.storage_changes.get(key) {
            return Some(value.clone());
        }

        if let Some(value) = self.storage_cache.borrow_mut().get_mut(key) {
            return Some(value.clone());
        }

        None
    }

    pub fn set_storage(&mut self, key: Bytes, value: Bytes) {
        // update removable set
        if self.storage_removable.contains(&key) {
            self.storage_removable.remove(&key);
        }

        self.storage_changes.insert(key, value);
    }

    pub fn remove_storage(&mut self, key: Bytes) {
        // update storage changes
        if self.storage_changes.contains_key(&key) {
            let old = self.storage_changes.remove(&key);
            debug!(target: "vm", "removed avm value {:?}", old);
        }
        
        self.storage_removable.insert(key);
    }

    pub fn update_root(&mut self, address: &Address, db: &mut HashStore) {
        debug!(target: "vm", "account type: {:?}", self.acc_type());
        if self.account_type == AccType::AVM {
            let mut concatenated_root = Vec::new();
            concatenated_root.extend_from_slice(&self.storage_root[..]);
            concatenated_root.extend_from_slice(&self.objectgraph_hash[..]);
            debug!(target: "vm", "concatenated root = {:?}", concatenated_root);
            self.delta_root = blake2b(&concatenated_root);
            debug!(target: "vm", "updated storage root = {:?}, delta_root = {:?}, code hash = {:?}", 
                self.storage_root, self.delta_root, self.code_hash);
            // save object graph
            debug!(target: "vm", "hash for object graph = {:?}", self.delta_root);
            db.emplace(
                self.delta_root.clone(),
                DBValue::from_slice(self.object_graph_cache.as_slice()),
            );
            // save key/valud storage root
            db.emplace(address.clone(), DBValue::from_slice(&self.storage_root[..]));
        }
    }

    pub fn save_object_graph(&mut self, address: &Address, db: &mut HashStore) {
        // save object graph
        println!("hash for object graph = {:?}", self.delta_root);
        db.emplace(
            self.delta_root.clone(),
            DBValue::from_slice(self.object_graph_cache.as_slice()),
        );
        // save key/valud storage root
        db.emplace(address.clone(), DBValue::from_slice(&self.storage_root[..]));
    }

    pub fn update_object_graph(&mut self, db: &HashStore) {
        match db.get(&self.storage_root) {
            Some(x) => {
                self.object_graph_size = Some(x.len());
                self.object_graph_cache = Arc::new(x[..].to_vec());
            }
            None => {
                self.object_graph_size = None;
            }
        }
    }

    #[cfg(test)]
    /// Provide a byte array which hashes to the `code_hash`. returns the hash as a result.
    pub fn note_code(&mut self, code: Bytes) -> Result<(), H256> {
        let h = blake2b(&code);
        if self.code_hash == h {
            self.code_cache = Arc::new(code);
            self.code_size = Some(self.code_cache.len());
            Ok(())
        } else {
            Err(h)
        }
    }
}

impl fmt::Debug for AionVMAccount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AionVMAccount")
            .field("balance", &self.balance)
            .field("nonce", &self.nonce)
            .field("code", &self.code())
            .field("code_hash", &self.code_hash())
            .field(
                "storage",
                &self.storage_changes.iter().collect::<BTreeMap<_, _>>(),
            )
            .field("storage_root", &self.storage_root)
            .field("account_type", &self.account_type)
            .field("empty_but_commit", &self.empty_but_commit)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvdb::MemoryDB;
    use account_db::*;

    #[test]
    fn storage_at() {
        let mut db = MemoryDB::new();
        let mut db = AccountDBMut::new(&mut db, &Address::new());
        let rlp = {
            let mut a = AionVMAccount::new_contract(69.into(), 0.into());
            let key = vec![0u8; 16];
            a.set_storage(key, vec![0x12, 0x34]);
            a.commit_storage(&Default::default(), &mut db).unwrap();
            a.init_code(vec![]);
            a.commit_code(&mut db);
            a.rlp()
        };

        let a = AionVMAccount::from_rlp(&rlp);
        assert_eq!(
            *a.storage_root().unwrap(),
            "d2e59a50e7414e56da75917275d1542a13fd345bf88a657a4222a0d50ad58868".into()
        );
        let value = a.storage_at(&db.immutable(), &vec![0x00; 16]).unwrap();
        assert_eq!(value, vec![0x12, 0x34]);
        let value = a.storage_at(&db.immutable(), &vec![0x01]).unwrap();
        assert_eq!(value, Vec::<u8>::new());
    }

    #[test]
    fn commit_storage() {
        let mut a = AionVMAccount::new_contract(69.into(), 0.into());
        let mut db = MemoryDB::new();
        let mut db = AccountDBMut::new(&mut db, &Address::new());
        a.set_storage(vec![0u8; 16], vec![0x12, 0x34]);
        assert_eq!(a.storage_root(), None);
        a.commit_storage(&Default::default(), &mut db).unwrap();
        assert_eq!(
            *a.storage_root().unwrap(),
            "d2e59a50e7414e56da75917275d1542a13fd345bf88a657a4222a0d50ad58868".into()
        );
    }

    #[test]
    fn note_code() {
        let mut db = MemoryDB::new();
        let mut db = AccountDBMut::new(&mut db, &Address::new());

        let rlp = {
            let mut a = AionVMAccount::new_contract(69.into(), 0.into());
            a.init_code(vec![0x55, 0x44, 0xffu8]);
            a.commit_code(&mut db);
            a.rlp()
        };

        let mut a = Account::from_rlp(&rlp);
        assert!(a.cache_code(&db.immutable()).is_some());

        let mut a = Account::from_rlp(&rlp);
        assert_eq!(a.note_code(vec![0x55, 0x44, 0xffu8]), Ok(()));
    }

    #[test]
    fn cache_transformed_code() {
        let address = Address::new();
        let mut db = MemoryDB::new();
        let mut db = AccountDBMut::new(&mut db, &address);
        let mut a = AionVMAccount::new_contract(69.into(), 0.into());

        let rlp = {
            a.init_transformed_code(vec![0x55, 0x44, 0xffu8]);
            // update account's address hash
            a.address_hash(&address);
            a.commit_code(&mut db);
            a.rlp()
        };

        let mut a = AionVMAccount::from_rlp(&rlp);
        a.address_hash(&address);
        assert_eq!(a.cache_code(&db.immutable()), Some(Arc::new(vec![])));
        assert_eq!(a.account_type, AccType::FVM);
        assert_eq!(
            a.cache_transformed_code(&db.immutable()),
            Some(Arc::new(vec![0x55, 0x44, 0xffu8]))
        );
        assert_eq!(a.account_type, AccType::AVM);
    }

    #[test]
    fn cache_objectgraph() {
        let address = Address::new();
        let mut db = MemoryDB::new();
        let mut db = AccountDBMut::new(&mut db, &address);
        let mut a = AionVMAccount::new_contract(69.into(), 0.into());

        let rlp = {
            a.init_objectgraph(vec![0x55, 0x44, 0xffu8]);
            a.commit_storage(&Default::default(), &mut db).unwrap();
            // calculate delta_root and save it in accountDB
            a.update_root(&address, &mut db);
            a.rlp()
        };

        let mut a = AionVMAccount::from_rlp(&rlp);
        assert_eq!(
            a.cache_objectgraph(&address, &db.immutable()),
            Some(Arc::new(vec![0x55, 0x44, 0xffu8]))
        );
    }

    #[test]
    fn cached_storage_at() {
        let mut db = MemoryDB::new();
        let mut db = AccountDBMut::new(&mut db, &Address::new());
        let mut a = AionVMAccount::new_contract(69.into(), 0.into());

        a.set_storage(vec![0x12, 0x34], vec![0x67, 0x78]);
        a.commit_storage(&Default::default(), &mut db).unwrap();

        assert!(a.cached_storage_at(&vec![0x12, 0x34]).is_some());

        a.storage_cache.borrow_mut().clear();
        assert!(!a.cached_storage_at(&vec![0x12, 0x34]).is_some());
    }
}

// account will not actually be shared between threads
unsafe impl Sync for AionVMAccount {}
