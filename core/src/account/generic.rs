use std::cell::{Cell};
use std::sync::Arc;
use std::collections::HashSet;

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
    // Account type: FVM or AVM
    //pub account_type: U256,
}

/// Single account in the system.
/// Keeps track of changes to the code and storage.
/// The changes are applied in `commit_storage` and `commit_code`
#[derive(Clone)]
pub struct Account<T, U> {

    pub balance: U256,

    pub nonce: U256,

    // trie-backed storage.
    pub storage_root: H256,

    // avm storage root
    pub delta_root: H256,

    // LRU cache of the trie-backed storage.
    // limited to `STORAGE_CACHE_ITEMS` recent queries
    pub storage_cache: T,

    // modified storage. Accumulates changes to storage made in `set_storage`
    // takes precedence over `storage_cache`.
    pub storage_changes: U,

    // aion: java kernel specific
    pub storage_removable: HashSet<Bytes>,

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
