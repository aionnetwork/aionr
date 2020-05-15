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

//! A minimal "state backend" trait: an abstraction over the sources of data
//! a blockchain state may draw upon.
//!
//! Currently assumes a very specific DB + cache structure, but
//! should become general over time to the point where not even a
//! merkle trie is strictly necessary.

use std::sync::Arc;

use state::{AionVMAccount};
use aion_types::{Address, H256};
use kvdb::{AsHashStore, HashStore};

/// State backend. See module docs for more details.
pub trait Backend: Send {
    /// Treat the backend as a read-only hashdb.
    fn as_hashstore(&self) -> &HashStore;

    /// Treat the backend as a writeable hashdb.
    fn as_hashstore_mut(&mut self) -> &mut HashStore;

    /// Add an account entry to the cache.
    fn add_to_account_cache(&mut self, addr: Address, data: Option<AionVMAccount>, modified: bool);

    /// Add a global code cache entry. This doesn't need to worry about canonicality because
    /// it simply maps hashes to raw code and will always be correct in the absence of
    /// hash collisions.
    fn cache_code(&self, hash: H256, code: Arc<Vec<u8>>);

    /// Add a global transformed code cache entry.
    fn cache_transformed_code(&self, hash: H256, code: Arc<Vec<u8>>);

    /// Get basic copy of the cached account. Not required to include storage.
    /// Returns 'None' if cache is disabled or if the account is not cached.
    fn get_cached_account(&self, addr: &Address) -> Option<Option<AionVMAccount>>;

    /// Get value from a cached account.
    /// `None` is passed to the closure if the account entry cached
    /// is known not to exist.
    /// `None` is returned if the entry is not cached.
    fn get_cached<F, U>(&self, a: &Address, f: F) -> Option<U>
    where F: FnOnce(Option<&mut AionVMAccount>) -> U;

    /// Get cached code based on hash.
    fn get_cached_code(&self, hash: &H256) -> Option<Arc<Vec<u8>>>;

    /// Get transformed cached code based on hash.
    fn get_transformed_cached_code(&self, hash: &H256) -> Option<Arc<Vec<u8>>>;

    /// Note that an account with the given address is non-null.
    fn note_non_null_account(&self, address: &Address);

    /// Check whether an account is known to be empty. Returns true if known to be
    /// empty, false otherwise.
    fn is_known_null(&self, address: &Address) -> bool;

    fn force_update(&mut self, address: &Address, account: &AionVMAccount);
}

/// A basic backend. Just wraps the given database, directly inserting into and deleting from
/// it. Doesn't cache anything.
pub struct Basic<H>(pub H);

impl<H: AsHashStore + Send + Sync> Backend for Basic<H> {
    fn as_hashstore(&self) -> &HashStore { self.0.as_hashstore() }

    fn as_hashstore_mut(&mut self) -> &mut HashStore { self.0.as_hashstore_mut() }

    fn add_to_account_cache(&mut self, _: Address, _: Option<AionVMAccount>, _: bool) {}

    fn cache_code(&self, _: H256, _: Arc<Vec<u8>>) {}

    fn cache_transformed_code(&self, _hash: H256, _code: Arc<Vec<u8>>) {}

    fn get_cached_account(&self, _: &Address) -> Option<Option<AionVMAccount>> { None }

    fn get_cached<F, U>(&self, _: &Address, _: F) -> Option<U>
    where F: FnOnce(Option<&mut AionVMAccount>) -> U {
        None
    }

    fn get_cached_code(&self, _: &H256) -> Option<Arc<Vec<u8>>> { None }

    fn get_transformed_cached_code(&self, _hash: &H256) -> Option<Arc<Vec<u8>>> { None }

    fn note_non_null_account(&self, _: &Address) {}
    fn is_known_null(&self, _: &Address) -> bool { false }
    fn force_update(&mut self, _addr: &Address, _account: &AionVMAccount) {}
}
