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

//! State of all accounts in the system expressed in Plain Old Data.

use std::fmt;
use std::collections::BTreeMap;
use itertools::Itertools;
use aion_types::{H256, Address};
use triehash::sec_trie_root;
use crate::pod_account::{self, PodAccount};
use crate::types::state::state_diff::StateDiff;
use ajson;

/// State of all accounts in the system expressed in Plain Old Data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PodState(BTreeMap<Address, PodAccount>);

impl PodState {
    /// Contruct a new object from the `m`.
    pub fn new() -> PodState { Default::default() }

    /// Contruct a new object from the `m`.
    pub fn from(m: BTreeMap<Address, PodAccount>) -> PodState { PodState(m) }

    /// Get the underlying map.
    pub fn get(&self) -> &BTreeMap<Address, PodAccount> { &self.0 }

    /// Get the root hash of the trie of the RLP of this.
    pub fn root(&self) -> H256 { sec_trie_root(self.0.iter().map(|(k, v)| (k, v.rlp()))) }

    /// Drain object to get the underlying map.
    pub fn drain(self) -> BTreeMap<Address, PodAccount> { self.0 }
}

impl From<ajson::blockchain::State> for PodState {
    fn from(s: ajson::blockchain::State) -> PodState {
        let state = s
            .into_iter()
            .map(|(addr, acc)| (addr.into(), PodAccount::from(acc)))
            .collect();
        PodState(state)
    }
}

impl From<ajson::spec::State> for PodState {
    fn from(s: ajson::spec::State) -> PodState {
        let state: BTreeMap<_, _> = s
            .into_iter()
            .filter(|pair| !pair.1.is_empty())
            .map(|(addr, acc)| (addr.into(), PodAccount::from(acc)))
            .collect();
        PodState(state)
    }
}

impl fmt::Display for PodState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (add, acc) in &self.0 {
            writeln!(f, "{} => {}", add, acc)?;
        }
        Ok(())
    }
}

/// Calculate and return diff between `pre` state and `post` state.
pub fn diff_pod(pre: &PodState, post: &PodState) -> StateDiff {
    StateDiff {
        raw: pre
            .get()
            .keys()
            .merge(post.get().keys())
            .filter_map(|acc| {
                pod_account::diff_pod(pre.get().get(acc), post.get().get(acc))
                    .map(|d| (acc.clone(), d))
            })
            .collect(),
    }
}
