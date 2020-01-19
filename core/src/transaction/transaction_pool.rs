/*******************************************************************************
 * Copyright (c) 2017-2018 Aion foundation.
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
 * Contributors:
 *     Aion foundation.
 *
 ******************************************************************************/

use std::collections::HashMap;

use parking_lot::RwLock;

use aion_types::{Address, H256, U256};
use crate::transaction::banning_queue::BanningTransactionQueue;
use crate::transaction::transaction_queue::{AccountDetails, RemovalReason, VerifiedTransaction,
TransactionOrigin, TransactionQueueStatus};
use crate::transaction::transaction::{Condition, SignedTransaction, PendingTransaction};
use crate::transaction::local_transactions::Status as LocalTransactionStatus;
use crate::transaction::error::Error;

/// Transaction pool
pub struct TransactionPool {
    /// Transaction queue
    transaction_queue: RwLock<BanningTransactionQueue>,
    /// Transactions waiting to enter queue
    waiting_transactions: RwLock<Vec<VerifiedTransaction>>,
    /// Flag to indicate if any transaction is sealed since last update
    is_transaction_sealed: RwLock<bool>,
    /// Transaction hash to be removed from queue with its removal reason
    transactions_to_remove: RwLock<HashMap<H256, RemovalReason>>,
    /// Transaction index
    transaction_index: RwLock<u64>,
}

impl TransactionPool {
    /// New transaction pool
    pub fn new(transaction_queue: RwLock<BanningTransactionQueue>) -> TransactionPool {
        TransactionPool {
            transaction_queue: transaction_queue,
            waiting_transactions: RwLock::new(Vec::new()),
            is_transaction_sealed: RwLock::new(false),
            transactions_to_remove: RwLock::new(HashMap::new()),
            transaction_index: RwLock::new(0),
        }
    }

    /// Clear and reset
    pub fn clear(&self) {
        self.transaction_queue.write().clear();
        self.waiting_transactions.write().clear();
        *self.is_transaction_sealed.write() = false;
        self.transactions_to_remove.write().clear();
        *self.transaction_index.write() = 0;
    }

    /// Update transaction queue
    pub fn update<F, G>(&self, fetch_account: &F, best_block: &G)
    where
        F: Fn(&Address) -> AccountDetails,
        G: Fn() -> u64,
    {
        // trace!(target: "txpool", "update transaction pool best block: {:?}. waiting transactions: {:?}", best_block(), self.waiting_transactions.read().len());
        // Update transaction queue, remove sealed/old transactions
        if *self.is_transaction_sealed.read() {
            trace!(target: "txpool", "remove sealed/old transactions");
            self.transaction_queue
                .write()
                .remove_old(fetch_account, best_block());
            *self.is_transaction_sealed.write() = false;
            trace!(target: "txpool", "is_transaction_sealed set to false");
        }

        // Remove invalid transactions
        self.transactions_to_remove.write().retain(|hash, reason| {
            trace!(target: "txpool", "remove transaction from queue: {:?}", &hash);
            self.transaction_queue
                .write()
                .remove(hash, fetch_account, reason);
            false
        });

        // Import waiting transactions
        self.waiting_transactions.write().retain(|transaction| {
            match transaction.origin() {
                TransactionOrigin::Local | TransactionOrigin::RetractedBlock => {
                    let _ = self
                        .transaction_queue
                        .write()
                        .add(transaction.clone(), fetch_account);
                }
                TransactionOrigin::External => {
                    let _ = self
                        .transaction_queue
                        .write()
                        .add_with_banlist(transaction.clone(), fetch_account);
                }
            }
            false
        });
        trace!(target: "txpool", "transaction queue status: {:?}", self.transaction_queue.read().status());
    }

    /// Add transaction to queue
    pub fn add_transaction(
        &self,
        transaction: SignedTransaction,
        origin: TransactionOrigin,
        condition: Option<Condition>,
        insertion_block: u64,
    ) -> Result<(), Error>
    {
        // trace!(target: "txpool", "try to add transaction to waiting queue. Current queue size: {:?}", self.waiting_transactions.read().len());
        if self
            .transaction_queue
            .read()
            .find(&transaction.hash())
            .is_some()
        {
            return Err(Error::AlreadyImported);
        }
        self.waiting_transactions
            .write()
            .push(VerifiedTransaction::new(
                transaction,
                origin,
                condition,
                insertion_block,
                self.next_transaction_index(),
            ));
        // trace!(target: "txpool", "transaction added to waiting queue. Current queue size: {:?}", self.waiting_transactions.read().len());
        Ok(())
    }

    /// Record transactions sealed
    pub fn record_transaction_sealed(&self) {
        *self.is_transaction_sealed.write() = true;
        trace!(target: "txpool", "is_transaction_sealed set to true");
    }

    pub fn find_transaction(&self, hash: &H256) -> Option<PendingTransaction> {
        self.transaction_queue.read().find(hash)
    }

    /// Get pending transaction
    pub fn top_transactions(&self, best_block: u64, best_timestamp: u64) -> Vec<SignedTransaction> {
        self.transaction_queue
            .read()
            .top_transactions_at(best_block, best_timestamp)
    }

    /// Get pending transaction
    pub fn pending_transactions(
        &self,
        best_block: u64,
        best_timestamp: u64,
    ) -> Vec<PendingTransaction>
    {
        self.transaction_queue
            .read()
            .pending_transactions(best_block, best_timestamp)
    }

    /// Get pending transaction hashes
    pub fn pending_hashes(&self) -> Vec<H256> { self.transaction_queue.read().pending_hashes() }

    /// Get future transaction
    pub fn future_transactions(&self) -> Vec<PendingTransaction> {
        self.transaction_queue.read().future_transactions()
    }

    /// Get local transactions
    pub fn local_transactions(&self) -> HashMap<H256, LocalTransactionStatus> {
        self.transaction_queue.read().local_transactions().clone()
    }

    /// Get last nonce of an address in the queue
    pub fn last_nonce(&self, address: &Address) -> Option<U256> {
        self.transaction_queue.read().last_nonce(address)
    }

    /// Check if any local pending transactions
    pub fn has_local_pending_transactions(&self) -> bool {
        self.transaction_queue
            .read()
            .has_local_pending_transactions()
    }

    /// Check status of the queue
    pub fn status(&self) -> TransactionQueueStatus { self.transaction_queue.read().status() }

    /// Remove transaction from queue
    pub fn remove_transaction(&self, transaction_hash: H256, removal_reason: RemovalReason) {
        trace!(target: "txpool", "record transaction to remove: {:?}", &transaction_hash);
        self.transactions_to_remove
            .write()
            .insert(transaction_hash, removal_reason);
    }

    /// Ban transaction
    pub fn ban_transaction(&self, hash: &H256) -> bool {
        self.transaction_queue.write().ban_transaction(hash)
    }

    /// Penalize transaction
    pub fn penalize(&self, hash: &H256) { self.transaction_queue.write().penalize(hash); }

    /// Count transaction index
    fn next_transaction_index(&self) -> u64 {
        let index: u64 = *self.transaction_index.read();
        *self.transaction_index.write() += 1;
        index
    }
}
