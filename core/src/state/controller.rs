use std::cell::RefCell;
use std::collections::HashMap;
use aion_types::{Address, U256};
use state::{
    VMAccount,
    FVMAccount,
    AVMAccount,
    Backend,
    RequireCache,
};
use trie;
use account_db::Factory as AccountFactory;

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
/// Account modification state. Used to check if the account was
/// Modified in between commits and overall.
pub enum AccountState {
    /// Account was loaded from disk and never modified in this state object.
    CleanFresh,
    /// Account was loaded from the global cache and never modified.
    CleanCached,
    /// Account has been modified and is not committed to the trie yet.
    /// This is set if any of the account data is changed, including
    /// storage and code.
    Dirty,
    /// Account was modified and committed to the trie.
    Committed,
}

#[derive(Debug)]
/// In-memory copy of the account data. Holds the optional account
/// and the modification status.
/// Account entry can contain existing (`Some`) or non-existing
/// account (`None`)
pub struct AccountEntry<T>
where T: VMAccount
{
    /// Account entry. `None` if account known to be non-existant.
    pub account: Option<T>,
    /// Unmodified account balance.
    pub old_balance: Option<U256>,
    /// Entry state.
    pub state: AccountState,
}

// Account cache item. Contains account data and
// modification state
impl<T: VMAccount> AccountEntry<T> {
    pub fn is_dirty(&self) -> bool { self.state == AccountState::Dirty }

    /// Clone dirty data into new `AccountEntry`. This includes
    /// basic account data and modified storage keys.
    /// Returns None if clean.
    pub fn clone_if_dirty(&self) -> Option<AccountEntry<T>> {
        match self.is_dirty() {
            true => Some(self.clone_dirty()),
            false => None,
        }
    }

    /// Clone dirty data into new `AccountEntry`. This includes
    /// basic account data and modified storage keys.
    pub fn clone_dirty(&self) -> AccountEntry<T> {
        AccountEntry {
            old_balance: self.old_balance,
            account: self.account.as_ref().map(T::clone_dirty),
            state: self.state,
        }
    }

    // Create a new account entry and mark it as dirty.
    pub fn new_dirty(account: Option<T>) -> AccountEntry<T> {
        AccountEntry {
            old_balance: account.as_ref().map(|a| a.balance().clone()),
            account: account,
            state: AccountState::Dirty,
        }
    }

    // Create a new account entry and mark it as clean.
    pub fn new_clean(account: Option<T>) -> AccountEntry<T> {
        AccountEntry {
            old_balance: account.as_ref().map(|a| a.balance().clone()),
            account: account,
            state: AccountState::CleanFresh,
        }
    }

    // Create a new account entry and mark it as clean and cached.
    pub fn new_clean_cached(account: Option<T>) -> AccountEntry<T> {
        AccountEntry {
            old_balance: account.as_ref().map(|a| a.balance().clone()),
            account: account,
            state: AccountState::CleanCached,
        }
    }

    // Replace data with another entry but preserve storage cache.
    pub fn overwrite_with(&mut self, other: AccountEntry<T>) {
        self.state = other.state;
        match other.account {
            Some(acc) => {
                if let Some(ref mut ours) = self.account {
                    ours.overwrite_with(acc);
                }
            }
            None => self.account = None,
        }
    }
}

pub struct VMAccountManager<T>
where T: VMAccount
{
    pub cache: RefCell<HashMap<Address, AccountEntry<T>>>,
    // The original account is preserved in
    pub checkpoints: RefCell<Vec<HashMap<Address, Option<AccountEntry<T>>>>>,
    pub account_start_nonce: U256,
}

pub trait AccountCacheOps<T>
where T: VMAccount
{
    ///
    fn insert_cache(&self, address: &Address, account: AccountEntry<T>);
    ///
    fn note_cache(&self, address: &Address);
    // get cached account entry
    // fn get_cached(&self, address: &Address) -> Option<&AccountEntry<T>>;
}

impl<T: VMAccount> AccountCacheOps<T> for VMAccountManager<T> {
    fn insert_cache(&self, address: &Address, account: AccountEntry<T>) {
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

}

// macro_rules! impl_cache_ops {
//     ($T: ty) => {
//         impl AccountCacheOps<$T> for VMAccountManager<$T> {
//             fn insert_cache(&self, address: &Address, account: AccountEntry<$T>) {
//                 // Dirty account which is not in the cache means this is a new account.
//                 // It goes directly into the checkpoint as there's nothing to rever to.
//                 //
//                 // In all other cases account is read as clean first, and after that made
//                 // dirty in and added to the checkpoint with `note_cache`.
//                 let is_dirty = account.is_dirty();
//                 let old_value = self.cache.borrow_mut().insert(*address, account);
//                 if is_dirty {
//                     if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
//                         checkpoint.entry(*address).or_insert(old_value);
//                     }
//                 }
//             }

//             fn note_cache(&self, address: &Address) {
//                 if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
//                     checkpoint.entry(*address).or_insert_with(|| {
//                         self.cache
//                             .borrow()
//                             .get(address)
//                             .map(AccountEntry::clone_dirty)
//                     });
//                 }
//             }
//         }
//     };
// }

// impl_cache_ops!(FVMAccount);
// impl_cache_ops!(AVMAccount);

impl<T: VMAccount> VMAccountManager<T> {
    pub fn new(account_start_nonce: U256) -> Self {
        Self {
            cache: RefCell::new(HashMap::new()),
            checkpoints: RefCell::new(Vec::new()),
            account_start_nonce: account_start_nonce,
        }
    }

    pub fn new_with_cache(cache: HashMap<Address, AccountEntry<T>>, account_start_nonce: U256) -> Self {
        Self {
            cache: RefCell::new(cache),
            checkpoints: RefCell::new(Vec::new()),
            account_start_nonce: account_start_nonce,
        }
    }
}