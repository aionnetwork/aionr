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

//! Smart contract based transaction filter.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use aion_types::{H256, Address};
use client::{BlockChainClient, BlockId, ChainNotify};
use bytes::Bytes;
use parking_lot::Mutex;
use spec::CommonParams;
use transaction::{Action, SignedTransaction};
use blake2b::BLAKE2B_EMPTY;

use_contract!(transact_acl, "TransactAcl", "res/contracts/tx_acl.json");

const MAX_CACHE_SIZE: usize = 4096;

mod tx_permissions {
    pub const _ALL: u32 = 0xffffffff;
    pub const NONE: u32 = 0x0;
    pub const BASIC: u32 = 0b00000001;
    pub const CALL: u32 = 0b00000010;
    pub const CREATE: u32 = 0b00000100;
    pub const _PRIVATE: u32 = 0b00001000;
}

/// Connection filter that uses a contract to manage permissions.
pub struct TransactionFilter {
    contract: transact_acl::TransactAcl,
    contract_address: Address,
    permission_cache: Mutex<HashMap<(H256, Address), u32>>,
}

impl TransactionFilter {
    /// Create a new instance if address is specified in params.
    pub fn from_params(params: &CommonParams) -> Option<TransactionFilter> {
        params.transaction_permission_contract.map(|address| {
            TransactionFilter {
                contract: transact_acl::TransactAcl::default(),
                contract_address: address,
                permission_cache: Mutex::new(HashMap::new()),
            }
        })
    }

    /// Clear cached permissions.
    pub fn clear_cache(&self) { self.permission_cache.lock().clear(); }

    /// Check if transaction is allowed at given block.
    pub fn transaction_allowed(
        &self,
        parent_hash: &H256,
        transaction: &SignedTransaction,
        client: &BlockChainClient,
    ) -> bool
    {
        let mut cache = self.permission_cache.lock();
        let len = cache.len();

        let tx_type = match transaction.action {
            Action::Create => tx_permissions::CREATE,
            Action::Call(address) => {
                if client
                    .code_hash(&address, BlockId::Hash(*parent_hash))
                    .map_or(false, |c| c != BLAKE2B_EMPTY)
                {
                    tx_permissions::CALL
                } else {
                    tx_permissions::BASIC
                }
            }
        };
        let sender = transaction.sender();
        match cache.entry((*parent_hash, sender)) {
            Entry::Occupied(entry) => *entry.get() & tx_type != 0,
            Entry::Vacant(entry) => {
                let contract_address = self.contract_address;
                let permissions = self
                    .contract
                    .functions()
                    .allowed_tx_types()
                    .call(sender, &|data| {
                        client.call_contract(BlockId::Hash(*parent_hash), contract_address, data)
                    })
                    .map(|p| p.low_u32())
                    .unwrap_or_else(|e| {
                        debug!(target:"tx","Error calling tx permissions contract: {:?}", e);
                        tx_permissions::NONE
                    });

                if len < MAX_CACHE_SIZE {
                    entry.insert(permissions);
                }
                trace!(target:"tx","Permissions required: {}, got: {}", tx_type, permissions);
                permissions & tx_type != 0
            }
        }
    }
}

impl ChainNotify for TransactionFilter {
    fn new_blocks(
        &self,
        imported: Vec<H256>,
        _invalid: Vec<H256>,
        _enacted: Vec<H256>,
        _retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _proposed: Vec<Bytes>,
        _duration: u64,
    )
    {
        if !imported.is_empty() {
            self.clear_cache();
        }
    }
}
