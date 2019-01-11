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

//! Local Transactions List.
use std::collections::HashMap;
use aion_types::{H256, U256};
use transaction::{self, SignedTransaction, PendingTransaction};
use parking_lot::Mutex;
use io::IoChannel;

/// Status of local transaction.
/// Can indicate that the transaction is currently part of the queue (`Pending/Future`)
/// or gives a reason why the transaction was removed.
#[derive(Debug, PartialEq, Clone)]
pub enum Status {
    /// The transaction is currently in the transaction queue.
    Pending,
    /// The transaction is in future part of the queue.
    Future,
    /// Transaction is already mined.
    Mined(SignedTransaction),
    /// Transaction is dropped because of limit
    Dropped(SignedTransaction),
    /// Replaced because of higher gas price of another transaction.
    Replaced(SignedTransaction, U256, H256),
    /// Transaction was never accepted to the queue.
    Rejected(SignedTransaction, transaction::Error),
    /// Transaction is invalid.
    Invalid(SignedTransaction),
    /// Transaction was canceled.
    Canceled(PendingTransaction),
}

/// Keeps track of local transactions that are in the queue or were mined/dropped recently.
pub struct LocalTransactionsList {
    transactions: HashMap<H256, Status>,
    old_transactions: Vec<H256>,
    max_old: usize,
    io_channel: Mutex<IoChannel<TxIoMessage>>,
}

impl Default for LocalTransactionsList {
    fn default() -> Self { Self::new(10, Mutex::new(IoChannel::disconnected())) }
}

impl LocalTransactionsList {
    /// Create a new list of local transactions.
    pub fn new(max_old: usize, io_channel: Mutex<IoChannel<TxIoMessage>>) -> Self {
        LocalTransactionsList {
            transactions: Default::default(),
            old_transactions: Vec::new(),
            max_old: max_old,
            io_channel: io_channel,
        }
    }

    /// Create a new list of local transactions with default max old size
    pub fn new_default(io_channel: Mutex<IoChannel<TxIoMessage>>) -> Self {
        Self::new(10, io_channel)
    }

    /// Mark transaction with given hash as pending.
    pub fn mark_pending(&mut self, hash: H256) {
        debug!(target: "own_tx", "Imported to Current (hash {:?})", hash);
        self.transactions.insert(hash, Status::Pending);
    }

    /// Mark transaction with given hash as future.
    pub fn mark_future(&mut self, hash: H256) {
        debug!(target: "own_tx", "Imported to Future (hash {:?})", hash);
        self.transactions.insert(hash, Status::Future);
    }

    /// Mark given transaction as rejected from the queue.
    pub fn mark_rejected(&mut self, tx: SignedTransaction, err: transaction::Error) {
        debug!(target: "own_tx", "Transaction rejected (hash {:?}): {:?}", tx.hash(), err);
        self.mark_old(tx.hash().clone());
        self.transactions
            .insert(tx.hash(), Status::Rejected(tx, err));
    }

    /// Mark the transaction as replaced by transaction with given hash.
    pub fn mark_replaced(&mut self, tx: SignedTransaction, gas_price: U256, hash: H256) {
        debug!(target: "own_tx", "Transaction (hash {:?}) replaced by {:?} (new gas price: {:?})", tx.hash(), hash, gas_price);

        // Send message to signal the status change of the transaction
        let error_message: String = format!(
            "Transaction replaced by {} with new gas price {}.",
            &hash.to_string(),
            &gas_price.to_string()
        );
        let _ = self.io_channel.lock().send(TxIoMessage::Dropped {
            txhash: tx.hash(),
            error: error_message,
        });

        self.mark_old(tx.hash().clone());
        self.transactions
            .insert(tx.hash(), Status::Replaced(tx, gas_price, hash));
    }

    /// Mark transaction as invalid.
    pub fn mark_invalid(&mut self, tx: SignedTransaction) {
        warn!(target: "own_tx", "Transaction marked invalid (hash {:?})", tx.hash());

        // Send message to signal the status change of the transaction
        let error_message: String = String::from("Transaction marked invalid.");
        let _ = self.io_channel.lock().send(TxIoMessage::Dropped {
            txhash: tx.hash(),
            error: error_message,
        });

        self.mark_old(tx.hash().clone());
        self.transactions.insert(tx.hash(), Status::Invalid(tx));
    }

    /// Mark transaction as canceled.
    pub fn mark_canceled(&mut self, tx: PendingTransaction) {
        warn!(target: "own_tx", "Transaction canceled (hash {:?})", tx.hash());

        // Send message to signal the status change of the transaction
        let error_message: String = String::from("Transaction canceled.");
        let _ = self.io_channel.lock().send(TxIoMessage::Dropped {
            txhash: tx.hash(),
            error: error_message,
        });

        self.mark_old(tx.hash().clone());
        self.transactions.insert(tx.hash(), Status::Canceled(tx));
    }

    /// Mark transaction as dropped because of limit.
    pub fn mark_dropped(&mut self, tx: SignedTransaction) {
        warn!(target: "own_tx", "Transaction dropped (hash {:?})", tx.hash());

        // Send message to signal the status change of the transaction
        let error_message: String =
            String::from("Transaction with low priority dropped due to limit.");
        let _ = self.io_channel.lock().send(TxIoMessage::Dropped {
            txhash: tx.hash(),
            error: error_message,
        });

        self.mark_old(tx.hash().clone());
        self.transactions.insert(tx.hash(), Status::Dropped(tx));
    }

    /// Mark transaction as mined.
    pub fn mark_mined(&mut self, tx: SignedTransaction) {
        info!(target: "own_tx", "Transaction mined (hash {:?})", tx.hash());

        // Send message to signal the status change of the transaction
        let _ = self.io_channel.lock().send(TxIoMessage::Included {
            txhash: tx.hash(),
            result: vec![0],
        });

        self.mark_old(tx.hash().clone());
        self.transactions.insert(tx.hash(), Status::Mined(tx));
    }

    /// Returns true if the transaction is already in local transactions.
    pub fn contains(&self, hash: &H256) -> bool { self.transactions.contains_key(hash) }

    /// Return a map of all currently stored transactions.
    pub fn all_transactions(&self) -> &HashMap<H256, Status> { &self.transactions }

    /// Internally mark the transaction as old. Old transactions storage are
    /// limited by the old_max parameter.
    fn mark_old(&mut self, hash: H256) {
        self.clear_old();
        self.old_transactions.push(hash);
    }

    /// Clear old transactions storage to make space
    fn clear_old(&mut self) {
        while self.old_transactions.len() >= self.max_old {
            if let Some(hash) = self.old_transactions.pop() {
                self.transactions.remove(&hash);
            }
        }
    }
}

/// transaction status message useful in pb
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TxIoMessage {
    /// transaction included in block
    Included {
        /// transaction hash
        txhash: H256,
        /// transaction execute result
        result: Vec<u8>,
    },
    /// transaction dropped
    Dropped {
        /// transaction hash
        txhash: H256,
        /// transaction execute error
        error: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use key::generate_keypair;
    use io::IoService;

    #[test]
    fn should_add_transaction_as_pending() {
        // given
        let mut list = LocalTransactionsList::default();

        // when
        list.mark_pending(10.into());
        list.mark_future(20.into());

        // then
        assert!(list.contains(&10.into()), "Should contain the transaction.");
        assert!(list.contains(&20.into()), "Should contain the transaction.");
        assert_eq!(list.all_transactions()[&10.into()], Status::Pending);
        assert_eq!(list.all_transactions()[&20.into()], Status::Future);
    }

    #[test]
    fn should_clear_old_transactions() {
        // given
        let mut list = LocalTransactionsList::new(
            1,
            Mutex::new(IoService::<TxIoMessage>::start().unwrap().channel()),
        );
        let tx1 = new_tx(10.into());
        let tx1_hash = tx1.hash();
        let tx2 = new_tx(50.into());
        let tx2_hash = tx2.hash();

        list.mark_pending(10.into());
        list.mark_invalid(tx1);
        list.mark_dropped(tx2);
        assert!(list.contains(&tx2_hash));
        assert!(!list.contains(&tx1_hash));
        assert!(list.contains(&10.into()));

        // when
        list.mark_future(15.into());

        // then
        assert!(list.contains(&10.into()));
        assert!(list.contains(&15.into()));
    }

    fn new_tx(nonce: U256) -> SignedTransaction {
        let keypair = generate_keypair();
        transaction::Transaction::new(
            nonce,
            U256::from(1245),
            U256::from(10),
            transaction::Action::Create,
            U256::from(100),
            Default::default(),
        )
        .sign(keypair.secret(), None)
    }
}
