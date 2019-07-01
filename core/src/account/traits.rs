use std::sync::Arc;

use aion_types::{Address, H256, U256};
use acore_bytes::Bytes;
use kvdb::{KeyValueDB, HashStore};
use state::{Backend, RequireCache};
use trie::TrieError;

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum AccType {
    FVM,
    AVM,
}

pub trait VMAccount: Sync + Send {
    fn from_rlp(rlp: &[u8]) -> Self;

    /// Set this account's code to the given code.
    /// NOTE: Account should have been created with `new_contract()`
    fn init_code(&mut self, code: Bytes);
    fn init_transformed_code(&mut self, code: Bytes);

    fn init_objectgraph(&mut self, data: Bytes);

    fn objectgraph(&self) -> Option<Arc<Bytes>>;

    fn reset_code(&mut self, code: Bytes);

    fn balance(&self) -> &U256;

    fn nonce(&self) -> &U256;

    fn code_hash(&self) -> H256;

    fn transformed_code_hash(&self) -> H256;

    fn object_graph_hash(&self) -> H256;

    fn address_hash(&self, address: &Address) -> H256;

    /// returns the account's code. If `None` then the code cache isn't available -
    /// get someone who knows to call `note_code`.
    fn code(&self) -> Option<Arc<Bytes>>;

    fn transformed_code(&self) -> Option<Arc<Bytes>>;

    /// returns the account's code size. If `None` then the code cache or code size cache isn't available -
    /// get someone who knows to call `note_code`.
    fn code_size(&self) -> Option<usize>;

    fn transformed_code_size(&self) -> Option<usize>;

    /// Is `code_cache` valid; such that code is going to return Some?
    fn is_cached(&self) -> bool;

    fn is_transformed_cached(&self) -> bool;

    fn is_objectgraph_cached(&self) -> bool;

    /// Provide a database to get `code_hash`. Should not be called if it is a contract without code.
    fn cache_code(&mut self, db: &HashStore) -> Option<Arc<Bytes>>;

    fn cache_transformed_code(&mut self, db: &HashStore) -> Option<Arc<Bytes>>;
    fn cache_objectgraph(&mut self, a: &Address, db: &HashStore) -> Option<Arc<Bytes>>;
    fn cache_objectgraph_size(&mut self, db: &HashStore) -> bool;

    /// Provide code to cache. For correctness, should be the correct code for the
    /// account.
    fn cache_given_code(&mut self, code: Arc<Bytes>);
    fn cache_given_transformed_code(&mut self, code: Arc<Bytes>);
    fn cache_given_objectgraph(&mut self, data: Arc<Bytes>);

    /// Provide a database to get `code_size`. Should not be called if it is a contract without code.
    fn cache_code_size(&mut self, db: &HashStore) -> bool;
    fn cache_transformed_code_size(&mut self, db: &HashStore) -> bool;

    /// Check if account has zero nonce, balance, no code and no storage.
    ///
    /// NOTE: Will panic if `!self.storage_is_clean()`
    fn is_empty(&self) -> bool;

    /// Check if account has zero nonce, balance, no code.
    fn is_null(&self) -> bool;

    /// Check if account is basic (Has no code).
    fn is_basic(&self) -> bool;

    /// Return the storage root associated with this account or None if it has been altered via the overlay.
    fn storage_root(&self) -> Option<&H256>;

    /// Increment the nonce of the account by one.
    fn inc_nonce(&mut self);

    /// Increase account balance.
    fn add_balance(&mut self, x: &U256);

    /// Decrease account balance.
    /// Panics if balance is less than `x`
    fn sub_balance(&mut self, x: &U256);

    /// Commit any unsaved code. `code_hash` will always return the hash of the `code_cache` after this.
    fn commit_code(&mut self, db: &mut HashStore);

    /// Export to RLP.
    fn rlp(&self) -> Bytes;

    /// Clone account data and dirty storage keys
    fn clone_dirty(&self) -> Self;

    fn acc_type(&self) -> AccType;

    fn update_account_cache<B: Backend>(
        &mut self,
        a: &Address,
        require: RequireCache,
        state_db: &B,
        db: &HashStore,
        graph_db: Arc<KeyValueDB>,
    );

    fn prove_storage(
        &self,
        db: &HashStore,
        storage_key: H256,
    ) -> Result<(Vec<Bytes>, H256), Box<TrieError>>;
}
