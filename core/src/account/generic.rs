use std::cell::{Cell};
use std::sync::Arc;

use aion_types::{H256, U256};
use bytes::{Bytes};
use account::traits::AccType;

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
    /// Account type: FVM or AVM
    pub account_type: u8,
}

/// Single account in the system.
/// Keeps track of changes to the code and storage.
/// The changes are applied in `commit_storage` and `commit_code`
#[derive(Clone)]
pub struct Account<T, U> {
    // Balance of the account.
    pub balance: U256,
    // Nonce of the account.
    pub nonce: U256,
    // Trie-backed storage.
    pub storage_root: H256,
    // LRU Cache of the trie-backed storage.
    // This is limited to `STORAGE_CACHE_ITEMS` recent queries
    pub storage_cache: T,
    // Modified storage. Accumulates changes to storage made in `set_storage`
    // Takes precedence over `storage_cache`.
    pub storage_changes: U,

    // Code hash of the account.
    pub code_hash: H256,
    // Size of the accoun code.
    pub code_size: Option<usize>,
    // Code cache of the account.
    pub code_cache: Arc<Bytes>,
    // Account code new or has been modified.
    pub code_filth: Filth,
    // Cached address hash.
    pub address_hash: Cell<Option<H256>>,
    // empty_flag: for Aion Java Kernel Only
    pub empty_but_commit: bool,
    // account type: 0x00 = normal; 0x01 = EVM; 0x02 = AVM
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