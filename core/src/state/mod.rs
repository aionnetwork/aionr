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

//! A mutable state representation suitable to execute transactions.
//! Generic over a `Backend`. Deals with `Account`s.
//! Unconfirmed sub-states are managed with `checkpoint`s which may be canonicalized
//! or rolled back.

use blake2b::{BLAKE2B_EMPTY, BLAKE2B_NULL_RLP};
use std::cell::{RefMut, RefCell};
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use types::error::Error;
use types::executed::{Executed, ExecutionError};
use executive::Executive;
use factory::Factories;
use factory::VmFactory;
use machine::EthereumMachine as Machine;
use pod_account::*;
use pod_state::{self, PodState};
use receipt::Receipt;
use db::StateDB;
use transaction::SignedTransaction;
use types::state::state_diff::StateDiff;
use vms::EnvInfo;

use aion_types::{Address, H256, U256};
use acore_bytes::Bytes;
use kvdb::{KeyValueDB};

use trie;
use trie::recorder::Recorder;
use trie::{Trie, TrieDB, TrieError};

mod account_state;
mod account;
mod substate;

pub mod backend;

#[cfg(test)]
mod tests;

pub use state::account::{
    AionVMAccount,
    VMAccount,
    AccType,
    RequireCache,
    BasicAccount
};
pub use self::backend::Backend;
pub use self::substate::Substate;

use self::account_state::{AccountEntry, AccountState};

/// Used to return information about an `State::apply` operation.
#[derive(Debug)]
pub struct ApplyOutcome {
    /// The receipt for the applied transaction.
    pub receipt: Receipt,
}

/// Result type for the execution ("application") of a transaction.
pub type ApplyResult = Result<ApplyOutcome, Error>;

/// Representation of the entire state of all accounts in the system.
///
/// `State` can work together with `StateDB` to share account cache.
///
/// Local cache contains changes made locally and changes accumulated
/// locally from previous commits. Global cache reflects the database
/// state and never contains any changes.
///
/// Cache items contains account data, or the flag that account does not exist
/// and modification state (see `AccountState`)
///
/// Account data can be in the following cache states:
/// * In global but not local - something that was queried from the database,
/// but never modified
/// * In local but not global - something that was just added (e.g. new account)
/// * In both with the same value - something that was changed to a new value,
/// but changed back to a previous block in the same block (same State instance)
/// * In both with different values - something that was overwritten with a
/// new value.
///
/// All read-only state queries check local cache/modifications first,
/// then global state cache. If data is not found in any of the caches
/// it is loaded from the DB to the local cache.
///
/// **** IMPORTANT *************************************************************
/// All the modifications to the account data must set the `Dirty` state in the
/// `AccountEntry`. This is done in `require` and `require_or_from`. So just
/// use that.
/// ****************************************************************************
///
/// Upon destruction all the local cache data propagated into the global cache.
/// Propagated items might be rejected if current state is non-canonical.
///
/// State checkpointing.
///
/// A new checkpoint can be created with `checkpoint()`. checkpoints can be
/// created in a hierarchy.
/// When a checkpoint is active all changes are applied directly into
/// `cache` and the original value is copied into an active checkpoint.
/// Reverting a checkpoint with `revert_to_checkpoint` involves copying
/// original values from the latest checkpoint back into `cache`. The code
/// takes care not to overwrite cached storage while doing that.
/// checkpoint can be discarded with `discard_checkpoint`. All of the orignal
/// backed-up values are moved into a parent checkpoint (if any).
///
pub struct State<B: Backend> {
    db: B,
    root: H256,

    cache: RefCell<HashMap<Address, AccountEntry<AionVMAccount>>>,
    // The original account is preserved in
    checkpoints: RefCell<Vec<HashMap<Address, Option<AccountEntry<AionVMAccount>>>>>,
    account_start_nonce: U256,

    factories: Factories,
    kvdb: Arc<KeyValueDB>,
}

/// Mode of dealing with null accounts.
#[derive(PartialEq)]
pub enum CleanupMode<'a> {
    /// Create accounts which would be null.
    ForceCreate,
    /// Don't delete null accounts upon touching, but also don't create them.
    NoEmpty,
    /// Mark all touched accounts.
    TrackTouched(&'a mut HashSet<Address>),
}

const SEC_TRIE_DB_UNWRAP_STR: &'static str =
    "A state can only be created with valid root. Creating a SecTrieDB with a valid root will not \
     fail. Therefore creating a SecTrieDB with this state's root will not fail.";

impl<B: Backend> State<B> {
    /// Creates new state with empty state root
    /// Used for tests.
    pub fn new(
        mut db: B,
        account_start_nonce: U256,
        factories: Factories,
        kvdb: Arc<KeyValueDB>,
    ) -> State<B>
    {
        let mut root = H256::new();
        {
            // init trie and reset root too null
            let _ = factories.trie.create(db.as_hashstore_mut(), &mut root);
        }

        State {
            db: db,
            root: root,
            factories: factories,
            kvdb: kvdb,
            cache: RefCell::new(HashMap::new()),
            checkpoints: RefCell::new(Vec::new()),
            account_start_nonce: account_start_nonce,
        }
    }

    /// Creates new state with existing state root
    pub fn from_existing(
        db: B,
        root: H256,
        account_start_nonce: U256,
        factories: Factories,
        kvdb: Arc<KeyValueDB>,
    ) -> Result<State<B>, TrieError>
    {
        if !db.as_hashstore().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root));
        }

        let state = State {
            db: db,
            root: root,
            factories: factories,
            kvdb: kvdb,
            cache: RefCell::new(HashMap::new()),
            checkpoints: RefCell::new(Vec::new()),
            account_start_nonce: account_start_nonce,
        };

        Ok(state)
    }

    pub fn export_kvdb(&self) -> Arc<KeyValueDB> { self.kvdb.clone() }

    /// Get a VM factory that can execute on this state.
    pub fn vm_factory(&self) -> VmFactory { self.factories.vm.clone() }

    /// Swap the current backend for another.
    // TODO: [rob] find a less hacky way to avoid duplication of `Client::state_at`.
    pub fn replace_backend<T: Backend>(self, backend: T) -> State<T> {
        State {
            db: backend,
            root: self.root,
            factories: self.factories,
            kvdb: self.kvdb,
            cache: self.cache,
            checkpoints: self.checkpoints,
            account_start_nonce: self.account_start_nonce,
        }
    }

    /// Create a recoverable checkpoint of this state.
    /// AVM has no need of checkpoints
    pub fn checkpoint(&mut self) { self.checkpoints.get_mut().push(HashMap::new()) }

    /// Merge last checkpoint with previous.
    pub fn discard_checkpoint(&mut self) {
        // merge with previous checkpoint
        let last = self.checkpoints.get_mut().pop();
        if let Some(mut checkpoint) = last {
            if let Some(ref mut prev) = self.checkpoints.get_mut().last_mut() {
                if prev.is_empty() {
                    **prev = checkpoint;
                } else {
                    for (k, v) in checkpoint.drain() {
                        prev.entry(k).or_insert(v);
                    }
                }
            }
        }
    }

    /// Revert to the last checkpoint and discard it.
    pub fn revert_to_checkpoint(&mut self) {
        if let Some(mut checkpoint) = self.checkpoints.get_mut().pop() {
            for (k, v) in checkpoint.drain() {
                match v {
                    Some(v) => {
                        match self.cache.get_mut().entry(k) {
                            Entry::Occupied(mut e) => {
                                // Merge checkpointed changes back into the main account
                                // storage preserving the cache.
                                e.get_mut().overwrite_with(v);
                            }
                            Entry::Vacant(e) => {
                                e.insert(v);
                            }
                        }
                    }
                    None => {
                        if let Entry::Occupied(e) = self.cache.get_mut().entry(k) {
                            if e.get().is_dirty() {
                                e.remove();
                            }
                        }
                    }
                }
            }
        }
    }

    fn insert_cache(&self, address: &Address, account: AccountEntry<AionVMAccount>) {
        // Dirty account which is not in the cache means this is a new account.
        // It goes directly into the checkpoint as there's nothing to rever to.
        //
        // In all other cases account is read as clean first, and after that made
        // dirty in and added to the checkpoint with `note_cache`.
        let is_dirty = account.is_dirty();
        let old_value = self.cache.borrow_mut().insert(*address, account);
        if is_dirty {
            if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
                checkpoint.entry(*address).or_insert(old_value);
            }
        }
    }

    fn note_cache(&self, address: &Address) {
        if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
            checkpoint.entry(*address).or_insert_with(|| {
                self.cache
                    .borrow()
                    .get(address)
                    .map(AccountEntry::clone_dirty)
            });
        }
    }

    /// Destroy the current object and return root and database.
    pub fn drop(mut self) -> (H256, B) {
        self.propagate_to_global_cache();
        (self.root, self.db)
    }

    /// Return reference to root
    pub fn root(&self) -> &H256 { &self.root }

    /// Create a new contract at address `contract`. If there is already an account at the address
    /// it will have its code reset, ready for `init_code()`.
    pub fn new_contract(&mut self, contract: &Address, balance: U256, nonce_offset: U256) {
        self.insert_cache(
            contract,
            AccountEntry::new_dirty(Some(AionVMAccount::new_contract(
                balance,
                self.account_start_nonce + nonce_offset,
            ))),
        );
    }

    /// Remove an existing account.
    pub fn kill_account(&mut self, account: &Address) {
        trace!(target: "vm", "kill account: {:?}", account);
        self.insert_cache(account, AccountEntry::<AionVMAccount>::new_dirty(None));
    }

    /// Determine whether an account exists.
    pub fn exists(&self, a: &Address) -> trie::Result<bool> {
        debug!(target: "vm", "check account of: {:?}", a);
        // Bloom filter does not contain empty accounts, so it is important here to
        // check if account exists in the database directly before EIP-161 is in effect.
        // self.ensure_fvm_cached(a, RequireCache::None, false, |a| a.is_some())
        self.ensure_cached(a, RequireCache::None, false, |a| a.is_some())
    }

    /// Determine whether an account exists and if not empty.
    pub fn exists_and_not_null(&self, a: &Address) -> trie::Result<bool> {
        debug!(target: "vm", "exist and not null");
        self.ensure_cached(a, RequireCache::None, false, |a| {
            a.map_or(false, |a| !a.is_null())
        })
    }

    /// Determine whether an account exists and has code or non-zero nonce.
    pub fn exists_and_has_code_or_nonce(&self, a: &Address) -> trie::Result<bool> {
        debug!(target: "vm", "exist and has code or nonce");
        self.ensure_cached(a, RequireCache::CodeSize, false, |a| {
            a.map_or(false, |a| {
                a.code_hash() != BLAKE2B_EMPTY || *a.nonce() != self.account_start_nonce
            })
        })
    }

    /// Get the balance of account `a`.
    pub fn balance(&self, a: &Address) -> trie::Result<U256> {
        debug!(target: "vm", "get balance of: {:?}", a);
        self.ensure_cached(a, RequireCache::None, true, |a| {
            a.as_ref()
                .map_or(U256::zero(), |account| *account.balance())
        })
    }

    /// Get the nonce of account `a`.
    pub fn nonce(&self, a: &Address) -> trie::Result<U256> {
        debug!(target: "vm", "get nonce of {:?}", a);
        self.ensure_cached(a, RequireCache::None, true, |a| {
            a.as_ref()
                .map_or(self.account_start_nonce, |account| *account.nonce())
        })
    }

    /// Get the storage root of account `a`.
    pub fn storage_root(&self, a: &Address) -> trie::Result<Option<H256>> {
        debug!(target: "vm", "get storage root of: {:?}", a);
        self.ensure_cached(a, RequireCache::None, true, |a| {
            a.as_ref()
                .and_then(|account| account.storage_root().cloned())
        })
    }

    /// Mutate storage of account `address` so that it is `value` for `key`.
    pub fn storage_at(&self, address: &Address, key: &Bytes) -> trie::Result<Option<Bytes>> {
        // Storage key search and update works like this:
        // 1. If there's an entry for the account in the local cache check for the key and return it if found.
        // 2. If there's an entry for the account in the global cache check for the key or load it into that account.
        // 3. If account is missing in the global cache load it into the local cache and cache the key there.

        // Ok(None) for avm null
        // Ok(vec![]) for fastvm empty
        // Err() is error
        // Ok(Some) for some
        // check local cache first without updating
        {
            let local_cache = self.cache.borrow_mut();
            let account = local_cache.get(address);
            let mut local_account = None;
            if let Some(maybe_acc) = account {
                match maybe_acc.account {
                    Some(ref account) => {
                        if let Some(value) = account.cached_storage_at(key) {
                            // println!("TT: 1");
                            return Ok(Some(value));
                        } else {
                            // storage not cached, will try local search later
                            local_account = Some(maybe_acc);
                        }
                    }
                    // NOTE: No account found, is it possible in both fastvm and avm, maybe not
                    _ => {
                        return Ok(None);
                    }
                }
            }
            // check the global cache and and cache storage key there if found,
            let trie_res = self.db.get_cached(address, |acc| {
                match acc {
                    // NOTE: the same question as above
                    None => Ok(None),
                    Some(a) => {
                        let account_db = self
                            .factories
                            .accountdb
                            .readonly(self.db.as_hashstore(), a.address_hash(address));
                        a.storage_at(account_db.as_hashstore(), key)
                    }
                }
            });

            if let Some(res) = trie_res {
                return res;
            }

            // otherwise cache the account localy and cache storage key there.
            if let Some(ref mut acc) = local_account {
                if let Some(ref account) = acc.account {
                    let account_db = self
                        .factories
                        .accountdb
                        .readonly(self.db.as_hashstore(), account.address_hash(address));
                    return account.storage_at(account_db.as_hashstore(), key);
                } else {
                    return Ok(None);
                }
            }
        }

        // check if the account could exist before any requests to trie
        if self.db.is_known_null(address) {
            // println!("TT: 6");
            return Ok(None);
        }

        // account is not found in the global cache, get from the DB and insert into local
        let db = self
            .factories
            .trie
            .readonly(self.db.as_hashstore(), &self.root)
            .expect(SEC_TRIE_DB_UNWRAP_STR);
        let maybe_acc = db.get_with(address, AionVMAccount::from_rlp)?;
        let r = maybe_acc.as_ref().map_or(Ok(Some(vec![])), |a| {
            let account_db = self
                .factories
                .accountdb
                .readonly(self.db.as_hashstore(), a.address_hash(address));
            a.storage_at(account_db.as_hashstore(), key)
        });
        self.insert_cache(address, AccountEntry::new_clean(maybe_acc));
        r
    }

    /// Get accounts' code.
    pub fn code(&self, a: &Address) -> trie::Result<Option<Arc<Bytes>>> {
        debug!(target: "vm", "get code of: {:?}", a);
        self.ensure_cached(a, RequireCache::Code, true, |a| {
            a.as_ref().map_or(None, |a| a.code().clone())
        })
    }

    pub fn init_transformed_code(&mut self, a: &Address, code: Bytes) -> trie::Result<()> {
        self.require_or_from(
            a,
            true,
            || AionVMAccount::new_contract(0.into(), self.account_start_nonce),
            |_| {},
        )?
        .init_transformed_code(code);
        Ok(())
    }

    // object graph should ensure cached???
    pub fn get_objectgraph(&self, a: &Address) -> trie::Result<Option<Arc<Bytes>>> {
        let ret = self.ensure_cached(a, RequireCache::Code, true, |a| {
            a.as_ref().map_or(None, |a| a.objectgraph().clone())
        });

        debug!(target: "vm", "get object graph of: {:?} = {:?}", a, ret);

        return ret;
    }

    pub fn set_objectgraph(&mut self, a: &Address, data: Bytes) -> trie::Result<()> {
        //WORKAROUND: avm will set object graph after selfdestruct, avoid creating new account
        {
            let cache = self.cache.borrow();
            if let Some(maybe_acc) = cache.get(a) {
                if maybe_acc.is_dirty() && !maybe_acc.account.is_some() {
                    return Ok(());
                }
            }
        }

        self.require_or_from(
            a,
            true,
            || AionVMAccount::new_contract(0.into(), self.account_start_nonce),
            |_| {},
        )?
        .init_objectgraph(data);
        Ok(())
    }

    /// Get accounts' code. avm specific code (dedundant code saving)
    pub fn transformed_code(&self, a: &Address) -> trie::Result<Option<Arc<Bytes>>> {
        let ret = self.ensure_cached(a, RequireCache::Code, true, |a| {
            a.as_ref().map_or(None, |a| a.transformed_code().clone())
        });

        debug!(target: "vm", "get transformed code of: {:?} = {:?}", a, ret);

        return ret;
    }

    /// Get an account's code hash.
    pub fn code_hash(&self, a: &Address) -> trie::Result<H256> {
        debug!(target: "vm", "get code hash of: {:?}", a);
        self.ensure_cached(a, RequireCache::None, true, |a| {
            a.as_ref().map_or(BLAKE2B_EMPTY, |a| a.code_hash())
        })
    }

    /// Get accounts' code size.
    pub fn code_size(&self, a: &Address) -> trie::Result<Option<usize>> {
        debug!(target: "vm", "get code size");
        self.ensure_cached(a, RequireCache::CodeSize, true, |a| {
            a.as_ref().and_then(|a| a.code_size())
        })
    }

    /// Add `incr` to the balance of account `a`.
    pub fn add_balance(
        &mut self,
        a: &Address,
        incr: &U256,
        cleanup_mode: CleanupMode,
    ) -> trie::Result<()>
    {
        debug!(target: "state", "add_balance({}, {}): {}", a, incr, self.balance(a)?);
        let is_value_transfer = !incr.is_zero();
        if is_value_transfer || (cleanup_mode == CleanupMode::ForceCreate && !self.exists(a)?) {
            self.require(a, false)?.add_balance(incr);
        //panic!("hi");
        } else if let CleanupMode::TrackTouched(set) = cleanup_mode {
            if self.exists(a)? {
                set.insert(*a);
                self.touch(a)?;
            }
        }
        Ok(())
    }

    /// Subtract `decr` from the balance of account `a`.
    pub fn sub_balance(
        &mut self,
        a: &Address,
        decr: &U256,
        cleanup_mode: &mut CleanupMode,
    ) -> trie::Result<()>
    {
        debug!(target: "state", "sub_balance({}, {}): {}", a, decr, self.balance(a)?);
        if !decr.is_zero() || !self.exists(a)? {
            self.require(a, false)?.sub_balance(decr);
        }
        if let CleanupMode::TrackTouched(ref mut set) = *cleanup_mode {
            set.insert(*a);
        }
        Ok(())
    }

    /// Subtracts `by` from the balance of `from` and adds it to that of `to`.
    pub fn transfer_balance(
        &mut self,
        from: &Address,
        to: &Address,
        by: &U256,
        mut cleanup_mode: CleanupMode,
    ) -> trie::Result<()>
    {
        self.sub_balance(from, by, &mut cleanup_mode)?;
        self.add_balance(to, by, cleanup_mode)?;
        Ok(())
    }

    /// Increment the nonce of account `a` by 1.
    pub fn inc_nonce(&mut self, a: &Address) -> trie::Result<()> {
        self.require(a, false).map(|mut x| x.inc_nonce())
    }

    /// Mutate storage of account `a` so that it is `value` for `key`.
    pub fn set_storage(&mut self, a: &Address, key: Bytes, value: Bytes) -> trie::Result<()> {
        trace!(target: "state", "set_storage({}:{:?} to {:?})", a, key, value);
        self.require(a, false)?.set_storage(key, value);
        Ok(())
    }

    pub fn remove_storage(&mut self, a: &Address, key: Bytes) -> trie::Result<()> {
        self.require(a, false)?.remove_storage(key);
        Ok(())
    }

    /// Initialise the code of account `a` so that it is `code`.
    /// NOTE: Account should have been created with `new_contract`.
    pub fn init_code(&mut self, a: &Address, code: Bytes) -> trie::Result<()> {
        self.require_or_from(
            a,
            true,
            || AionVMAccount::new_contract(0.into(), self.account_start_nonce),
            |_| {},
        )?
        .init_code(code);
        Ok(())
    }

    pub fn set_empty_but_commit(&mut self, a: &Address) -> trie::Result<()> {
        self.require_or_from(
            a,
            true,
            || AionVMAccount::new_contract(0.into(), self.account_start_nonce),
            |_| {},
        )?
        .set_empty_but_commit();
        Ok(())
    }

    /// Reset the code of account `a` so that it is `code`.
    pub fn reset_code(&mut self, a: &Address, code: Bytes) -> trie::Result<()> {
        self.require_or_from(
            a,
            true,
            || AionVMAccount::new_contract(0.into(), self.account_start_nonce),
            |_| {},
        )?
        .reset_code(code);
        Ok(())
    }

    /// Execute a given transaction, producing a receipt.
    /// This will change the state accordingly.
    pub fn apply(
        &mut self,
        env_info: &EnvInfo,
        machine: &Machine,
        t: &SignedTransaction,
    ) -> ApplyResult
    {
        let e = self.execute(env_info, machine, t, true, false)?;

        self.commit()?;
        let state_root = self.root().clone();

        let receipt = Receipt::new(
            state_root,
            e.gas_used,
            e.transaction_fee,
            e.logs,
            e.output,
            e.exception,
        );
        trace!(target: "state", "Transaction receipt: {:?}", receipt);

        Ok(ApplyOutcome {
            receipt,
        })
    }

    pub fn apply_batch(
        &mut self,
        env_info: &EnvInfo,
        machine: &Machine,
        txs: &[SignedTransaction],
    ) -> Vec<ApplyResult>
    {
        let exec_results = self.execute_bulk(env_info, machine, txs, false, false);

        let mut receipts = Vec::new();
        for result in exec_results {
            //self.commit_touched(result.clone().unwrap().touched);
            let outcome = match result {
                Ok(e) => {
                    let state_root = e.state_root.clone();
                    let receipt = Receipt::new(
                        state_root,
                        e.gas_used,
                        e.transaction_fee,
                        e.logs,
                        e.output,
                        e.exception,
                    );
                    Ok(ApplyOutcome {
                        receipt,
                    })
                }
                Err(x) => Err(From::from(x)),
            };
            receipts.push(outcome);
        }

        trace!(target: "state", "Transaction receipt: {:?}", receipts);

        return receipts;
    }

    fn execute_bulk(
        &mut self,
        env_info: &EnvInfo,
        machine: &Machine,
        txs: &[SignedTransaction],
        check_nonce: bool,
        virt: bool,
    ) -> Vec<Result<Executed, ExecutionError>>
    {
        let mut e = Executive::new(self, env_info, machine);

        match virt {
            true => e.transact_virtual_bulk(txs, check_nonce),
            false => e.transact_bulk(txs, check_nonce, false),
        }
    }

    // Execute a given transaction without committing changes.
    //
    // `virt` signals that we are executing outside of a block set and restrictions like
    // gas limits and gas costs should be lifted.
    fn execute(
        &mut self,
        env_info: &EnvInfo,
        machine: &Machine,
        t: &SignedTransaction,
        check_nonce: bool,
        virt: bool,
    ) -> Result<Executed, ExecutionError>
    {
        let mut e = Executive::new(self, env_info, machine);

        match virt {
            true => e.transact_virtual(t, check_nonce),
            false => e.transact(t, check_nonce, false),
        }
    }

    fn touch(&mut self, a: &Address) -> trie::Result<()> {
        self.require(a, false)?;
        Ok(())
    }

    pub fn commit_touched(&mut self, _accounts: HashSet<Address>) -> Result<(), Error> { Ok(()) }

    /// Commits our cached account changes into the trie.
    pub fn commit(&mut self) -> Result<(), Error> {
        // first, commit the sub trees.
        let mut accounts = self.cache.borrow_mut();
        // debug!(target: "cons", "commit accounts = {:?}", accounts);
        for (address, ref mut a) in accounts.iter_mut().filter(|&(_, ref a)| a.is_dirty()) {
            debug!(target: "cons", "commit account: [{:?} - {:?}]", address, a);
            if let Some(ref mut account) = a.account {
                let addr_hash = account.address_hash(address);
                {
                    let mut account_db = self
                        .factories
                        .accountdb
                        .create(self.db.as_hashstore_mut(), addr_hash);
                    account.commit_code(account_db.as_hashstore_mut());
                    // Tmp workaround to ignore storage changes on null accounts
                    // until java kernel fixed the problem
                    debug!(target: "vm", "check null of {:?}", address);
                    if !account.is_null()
                        || address == &H256::from(
                            "0000000000000000000000000000000000000000000000000000000000000100",
                        )
                        || address == &H256::from(
                            "0000000000000000000000000000000000000000000000000000000000000200",
                        ) {
                        account
                            .commit_storage(&self.factories.trie, account_db.as_hashstore_mut())?;
                        account.update_root(self.kvdb.clone());
                    } else if !account.storage_changes().is_empty() {
                        // TODO: check key/value storage in avm
                        // to see whether discard is needed
                        account.discard_storage_changes();
                        a.state = AccountState::CleanFresh;
                    } else {
                        if !account.get_empty_but_commit() {
                            // Aion Java Kernel specific:
                            // 1. for code != NULL && return code == NULL && no storage chanage
                            // eg: [0x00, 0x60, 0x00]
                            // 2. code is NULL, this account should be commited
                            a.state = AccountState::CleanFresh;
                        }
                    }
                }
                if !account.is_empty() {
                    self.db.note_non_null_account(address);
                }
            }
        }

        {
            let mut trie = self
                .factories
                .trie
                .from_existing(self.db.as_hashstore_mut(), &mut self.root)?;
            for (address, ref mut a) in accounts.iter_mut().filter(|&(_, ref a)| a.is_dirty()) {
                a.state = AccountState::Committed;
                match a.account {
                    Some(ref mut account) => {
                        trie.insert(address, &account.rlp())?;
                    }
                    None => {
                        trie.remove(address)?;
                    }
                };
            }
        }
        debug!(target: "cons", "after commit: accounts = {:?}, state root = {:?}", accounts, self.root);

        Ok(())
    }

    /// Propagate local cache into shared canonical state cache.
    fn propagate_to_global_cache(&mut self) {
        let mut addresses = self.cache.borrow_mut();
        trace!(target:"state","Committing cache {:?} entries", addresses.len());
        for (address, a) in addresses.drain().filter(|&(_, ref a)| {
            a.state == AccountState::Committed || a.state == AccountState::CleanFresh
        }) {
            self.db
                .add_to_account_cache(address, a.account, a.state == AccountState::Committed);
        }
    }

    /// Clear state cache
    pub fn clear(&mut self) {
        self.cache.borrow_mut().clear();
        self.cache.borrow_mut().clear();
    }

    /// Populate the state from `accounts`.
    /// Used for tests.
    pub fn populate_from(&mut self, accounts: PodState) {
        assert!(self.checkpoints.borrow().is_empty());
        for (add, acc) in accounts.drain().into_iter() {
            self.cache.borrow_mut().insert(
                add,
                AccountEntry::new_dirty(Some(AionVMAccount::from_pod(acc))),
            );
        }
    }

    /// Populate a PodAccount map from this state.
    pub fn to_pod(&self) -> PodState {
        assert!(self.checkpoints.borrow().is_empty());
        // TODO: handle database rather than just the cache.
        // will need fat db.
        PodState::from(
            self.cache
                .borrow()
                .iter()
                .fold(BTreeMap::new(), |mut m, (add, opt)| {
                    if let Some(ref acc) = opt.account {
                        m.insert(add.clone(), PodAccount::from_account(acc));
                    }
                    m
                }),
        )
    }

    // Return a list of all touched addresses in cache.
    fn touched_addresses(&self) -> Vec<Address> {
        assert!(self.checkpoints.borrow().is_empty());
        self.cache.borrow().iter().map(|(add, _)| *add).collect()
    }

    fn query_pod(&mut self, query: &PodState, touched_addresses: &[Address]) -> trie::Result<()> {
        let pod = query.get();

        for address in touched_addresses {
            if !self.ensure_cached(address, RequireCache::Code, true, |a| a.is_some())? {
                continue;
            }

            if let Some(pod_account) = pod.get(address) {
                // needs to be split into two parts for the refcell code here
                // to work.
                for key in pod_account.storage.keys() {
                    self.storage_at(address, &key[..].to_vec())?;
                }
            }
        }

        Ok(())
    }

    /// Returns a `StateDiff` describing the difference from `orig` to `self`.
    /// Consumes self.
    pub fn diff_from<X: Backend>(&self, orig: State<X>) -> trie::Result<StateDiff> {
        let addresses_post = self.touched_addresses();
        let pod_state_post = self.to_pod();
        let mut state_pre = orig;
        state_pre.query_pod(&pod_state_post, &addresses_post)?;
        Ok(pod_state::diff_pod(&state_pre.to_pod(), &pod_state_post))
    }

    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn ensure_cached<F, U>(
        &self,
        a: &Address,
        require: RequireCache,
        check_null: bool,
        f: F,
    ) -> trie::Result<U>
    where
        F: Fn(Option<&AionVMAccount>) -> U,
    {
        // check local cache first
        debug!(target: "vm", "search local cache");
        if let Some(ref mut maybe_acc) = self.cache.borrow_mut().get_mut(a) {
            if let Some(ref mut account) = maybe_acc.account {
                let accountdb = self
                    .factories
                    .accountdb
                    .readonly(self.db.as_hashstore(), account.address_hash(a));
                account.update_account_cache(
                    a,
                    require,
                    &self.db,
                    accountdb.as_hashstore(),
                    self.kvdb.clone(),
                );
                return Ok(f(Some(account)));
            }
            return Ok(f(None));
        }
        // check global cache
        debug!(target: "vm", "search global cache");
        let result = self.db.get_cached(a, |mut acc| {
            if let Some(ref mut account) = acc {
                let accountdb = self
                    .factories
                    .accountdb
                    .readonly(self.db.as_hashstore(), account.address_hash(a));
                account.update_account_cache(
                    a,
                    require,
                    &self.db,
                    accountdb.as_hashstore(),
                    self.kvdb.clone(),
                );
            }
            f(acc.map(|a| &*a))
        });
        match result {
            Some(r) => Ok(r),
            None => {
                // first check if it is not in database for sure
                if check_null && self.db.is_known_null(a) {
                    return Ok(f(None));
                }

                trace!(target: "vm", "search local database");
                // not found in the global cache, get from the DB and insert into local
                let db = self
                    .factories
                    .trie
                    .readonly(self.db.as_hashstore(), &self.root)?;
                let mut maybe_acc = db.get_with(a, AionVMAccount::from_rlp)?;
                if let Some(ref mut account) = maybe_acc.as_mut() {
                    let accountdb = self
                        .factories
                        .accountdb
                        .readonly(self.db.as_hashstore(), account.address_hash(a));
                    account.update_account_cache(
                        a,
                        require,
                        &self.db,
                        accountdb.as_hashstore(),
                        self.kvdb.clone(),
                    );
                }
                let r = f(maybe_acc.as_ref());
                self.insert_cache(a, AccountEntry::new_clean(maybe_acc));
                Ok(r)
            }
        }
    }

    /// Pull account `a` in our cache from the trie DB. `require_code` requires that the code be cached, too.
    fn require<'a>(
        &'a self,
        a: &Address,
        require_code: bool,
    ) -> trie::Result<RefMut<'a, AionVMAccount>>
    {
        self.require_or_from(
            a,
            require_code,
            || AionVMAccount::new_basic(0u8.into(), self.account_start_nonce),
            |_| {},
        )
    }

    /// Pull account `a` in our cache from the trie DB. `require_code` requires that the code be cached, too.
    /// If it doesn't exist, make account equal the evaluation of `default`.
    fn require_or_from<'a, F, G>(
        &'a self,
        a: &Address,
        require_code: bool,
        default: F,
        not_default: G,
    ) -> trie::Result<RefMut<'a, AionVMAccount>>
    where
        F: FnOnce() -> AionVMAccount,
        G: FnOnce(&mut AionVMAccount),
    {
        let contains_key = self.cache.borrow().contains_key(a);
        if !contains_key {
            match self.db.get_cached_account(a) {
                Some(acc) => self.insert_cache(a, AccountEntry::new_clean_cached(acc)),
                None => {
                    let maybe_acc = if !self.db.is_known_null(a) {
                        let db = self
                            .factories
                            .trie
                            .readonly(self.db.as_hashstore(), &self.root)?;
                        AccountEntry::new_clean(db.get_with(a, AionVMAccount::from_rlp)?)
                    } else {
                        AccountEntry::new_clean(None)
                    };
                    self.insert_cache(a, maybe_acc);
                }
            }
        }
        self.note_cache(a);

        // at this point the entry is guaranteed to be in the cache.
        Ok(RefMut::map(self.cache.borrow_mut(), |c| {
            let entry = c
                .get_mut(a)
                .expect("entry known to exist in the cache; qed");

            match &mut entry.account {
                &mut Some(ref mut acc) => not_default(acc),
                slot => *slot = Some(default()),
            }

            // set the dirty flag after changing account data.
            entry.state = AccountState::Dirty;
            match entry.account {
                Some(ref mut account) => {
                    if require_code {
                        let addr_hash = account.address_hash(a);
                        let accountdb = self
                            .factories
                            .accountdb
                            .readonly(self.db.as_hashstore(), addr_hash);
                        account.update_account_cache(
                            a,
                            RequireCache::Code,
                            &self.db,
                            accountdb.as_hashstore(),
                            self.kvdb.clone(),
                        );
                    }
                    account
                }
                _ => panic!("Required account must always exist; qed"),
            }
        }))
    }
}

// State proof implementations; useful for light client protocols.
impl<B: Backend> State<B> {
    /// Prove an account's existence or nonexistence in the state trie.
    /// Returns a merkle proof of the account's trie node omitted or an encountered trie error.
    /// If the account doesn't exist in the trie, prove that and return defaults.
    /// Requires a secure trie to be used for accurate results.
    /// `account_key` == blake2b(address)
    pub fn prove_account(&self, account_key: H256) -> trie::Result<(Vec<Bytes>, BasicAccount)> {
        let mut recorder = Recorder::new();
        let trie = TrieDB::new(self.db.as_hashstore(), &self.root)?;
        let maybe_account: Option<BasicAccount> = {
            let query = (&mut recorder, ::rlp::decode);
            trie.get_with(&account_key, query)?
        };
        let account = maybe_account.unwrap_or_else(|| {
            BasicAccount {
                balance: 0.into(),
                nonce: self.account_start_nonce,
                code_hash: BLAKE2B_EMPTY,
                storage_root: BLAKE2B_NULL_RLP,
            }
        });

        Ok((
            recorder.drain().into_iter().map(|r| r.data).collect(),
            account,
        ))
    }

    /// Prove an account's storage key's existence or nonexistence in the state.
    /// Returns a merkle proof of the account's storage trie.
    /// Requires a secure trie to be used for correctness.
    /// `account_key` == blake2b(address)
    /// `storage_key` == blake2b(key)
    pub fn prove_storage(
        &self,
        account_key: H256,
        storage_key: H256,
    ) -> trie::Result<(Vec<Bytes>, H256)>
    {
        // TODO: probably could look into cache somehow but it's keyed by
        // address, not blake2b(address).
        let trie = TrieDB::new(self.db.as_hashstore(), &self.root)?;
        //TODO: update account type
        let acc = match trie.get_with(&account_key, AionVMAccount::from_rlp)? {
            Some(acc) => acc,
            None => return Ok((Vec::new(), H256::new())),
        };

        let account_db = self
            .factories
            .accountdb
            .readonly(self.db.as_hashstore(), account_key);
        acc.prove_storage(account_db.as_hashstore(), storage_key)
    }
}

impl<B: Backend> fmt::Debug for State<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "fvm accounts = {:?}, avm accounts = {:?}, state_root = {:?}",
            self.cache.borrow(),
            self.cache.borrow(),
            self.root()
        )
    }
}

// TODO: cloning for `State` shouldn't be possible in general; Remove this and use
// checkpoints where possible.
impl Clone for State<StateDB> {
    fn clone(&self) -> State<StateDB> {
        let cache = {
            let mut cache: HashMap<Address, AccountEntry<AionVMAccount>> = HashMap::new();
            for (key, val) in self.cache.borrow().iter() {
                if let Some(entry) = val.clone_if_dirty() {
                    cache.insert(key.clone(), entry);
                }
            }
            cache
        };

        State {
            db: self.db.boxed_clone(),
            root: self.root.clone(),
            factories: self.factories.clone(),
            kvdb: self.kvdb.clone(),
            cache: RefCell::new(cache),
            checkpoints: RefCell::new(Vec::new()),
            account_start_nonce: self.account_start_nonce.clone(),
        }
    }
}
