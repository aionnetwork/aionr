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

use std::fmt;
use std::collections::BTreeMap;
use itertools::Itertools;
use blake2b::{blake2b};
use aion_types::{H256, U256, U128};
use kvdb::HashStore;
use triehash::sec_trie_root;
use bytes::Bytes;
use trie::TrieFactory;
use state::{VMAccount, AionVMAccount};
use ajson;
use types::account_diff::*;
use rlp::{self, RlpStream};

#[derive(Debug, Clone, PartialEq, Eq)]
/// An account, expressed as Plain-Old-Data (hence the name).
/// Does not have a DB overlay cache, code hash or anything like that.
pub struct PodAccount {
    /// The balance of the account.
    pub balance: U256,
    /// The nonce of the account.
    pub nonce: U256,
    /// The code of the account or `None` in the special case that it is unknown.
    pub code: Option<Bytes>,
    /// The storage of the account.
    pub storage: BTreeMap<Bytes, Bytes>,
}

impl PodAccount {
    /// Convert Account to a PodAccount.
    /// NOTE: This will silently fail unless the account is fully cached.
    pub fn from_account(acc: &AionVMAccount) -> PodAccount {
        PodAccount {
            balance: *acc.balance(),
            nonce: *acc.nonce(),
            storage: acc
                .storage_changes()
                .iter()
                .fold(BTreeMap::new(), |mut m, (k, v)| {
                    m.insert(k.clone().as_slice().into(), v.clone().as_slice().into());
                    m
                }),
            code: acc.code().map(|x| x.to_vec()),
        }
    }

    /// Returns the RLP for this account.
    pub fn rlp(&self) -> Bytes {
        let mut stream = RlpStream::new_list(4);
        stream.append(&self.nonce);
        stream.append(&self.balance);
        stream.append(&sec_trie_root(
            self.storage
                .iter()
                .map(|(k, v)| (k, rlp::encode(&U256::from(&**v)))),
        ));
        stream.append(&blake2b(&self.code.as_ref().unwrap_or(&vec![])));
        stream.out()
    }

    /// Place additional data into given hash DB.
    pub fn insert_additional(&self, db: &mut HashStore, factory: &TrieFactory) {
        match self.code {
            Some(ref c) if !c.is_empty() => {
                db.insert(c);
            }
            _ => {}
        }
        let mut r = H256::new();
        let mut t = factory.create(db, &mut r);
        for (k, v) in &self.storage {
            if let Err(e) = t.insert(k, &rlp::encode(&U128::from(&**v))) {
                warn!(target:"db","Encountered potential DB corruption: {}", e);
            }
        }
    }
}

impl From<ajson::blockchain::Account> for PodAccount {
    fn from(a: ajson::blockchain::Account) -> Self {
        PodAccount {
            balance: a.balance.into(),
            nonce: a.nonce.into(),
            code: Some(a.code.into()),
            storage: a
                .storage
                .into_iter()
                .map(|(key, value)| {
                    let key: U128 = key.into();
                    let value: U128 = value.into();
                    (
                        <[u8; 16]>::from(key).to_vec(),
                        <[u8; 16]>::from(value).to_vec(),
                    )
                })
                .collect(),
        }
    }
}

impl From<ajson::spec::Account> for PodAccount {
    fn from(a: ajson::spec::Account) -> Self {
        PodAccount {
            balance: a.balance.map_or_else(U256::zero, Into::into),
            nonce: a.nonce.map_or_else(U256::zero, Into::into),
            code: Some(a.code.map_or_else(Vec::new, Into::into)),
            storage: a.storage.map_or_else(BTreeMap::new, |s| {
                s.into_iter()
                    .map(|(key, value)| {
                        let key: U128 = key.into();
                        let value: U128 = value.into();
                        (
                            <[u8; 16]>::from(key).to_vec(),
                            <[u8; 16]>::from(value).to_vec(),
                        )
                    })
                    .collect()
            }),
        }
    }
}

impl fmt::Display for PodAccount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "(bal={}; nonce={}; code={} bytes, #{}; storage={} items)",
            self.balance,
            self.nonce,
            self.code.as_ref().map_or(0, |c| c.len()),
            self.code.as_ref().map_or_else(H256::new, |c| blake2b(c)),
            self.storage.len(),
        )
    }
}

/// Determine difference between two optionally existant `Account`s. Returns None
/// if they are the same.
pub fn diff_pod(pre: Option<&PodAccount>, post: Option<&PodAccount>) -> Option<AccountDiff> {
    match (pre, post) {
        (None, Some(x)) => {
            Some(AccountDiff {
                balance: Diff::Born(x.balance),
                nonce: Diff::Born(x.nonce),
                code: Diff::Born(
                    x.code
                        .as_ref()
                        .expect(
                            "account is newly created; newly created accounts must be given code; \
                             all caches should remain in place; qed",
                        )
                        .clone(),
                ),
                storage: x
                    .storage
                    .iter()
                    .map(|(k, v)| (k.clone(), Diff::Born(v.clone())))
                    .collect(),
            })
        }
        (Some(x), None) => {
            Some(AccountDiff {
                balance: Diff::Died(x.balance),
                nonce: Diff::Died(x.nonce),
                code: Diff::Died(
                    x.code
                        .as_ref()
                        .expect(
                            "account is deleted; only way to delete account is running SUICIDE; \
                             account must have had own code cached to make operation; all caches \
                             should remain in place; qed",
                        )
                        .clone(),
                ),
                storage: x
                    .storage
                    .iter()
                    .map(|(k, v)| (k.clone(), Diff::Died(v.clone())))
                    .collect(),
            })
        }
        (Some(pre), Some(post)) => {
            let storage: Vec<_> = pre
                .storage
                .keys()
                .merge(post.storage.keys())
                .filter(|k| {
                    pre.storage.get(&**k).unwrap_or(&Vec::new())
                        != post.storage.get(&**k).unwrap_or(&Vec::new())
                })
                .collect();
            let r = AccountDiff {
                balance: Diff::new(pre.balance, post.balance),
                nonce: Diff::new(pre.nonce, post.nonce),
                code: match (pre.code.clone(), post.code.clone()) {
                    (Some(pre_code), Some(post_code)) => Diff::new(pre_code, post_code),
                    _ => Diff::Same,
                },
                storage: storage
                    .into_iter()
                    .map(|k| {
                        (
                            k.clone(),
                            Diff::new(
                                pre.storage.get(k).cloned().unwrap_or_else(Vec::new),
                                post.storage.get(k).cloned().unwrap_or_else(Vec::new),
                            ),
                        )
                    })
                    .collect(),
            };
            if r.balance.is_same() && r.nonce.is_same() && r.code.is_same() && r.storage.is_empty()
            {
                None
            } else {
                Some(r)
            }
        }
        _ => None,
    }
}
