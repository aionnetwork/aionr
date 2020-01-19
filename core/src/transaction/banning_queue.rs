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

//! Banning Queue
//! Transacton Queue wrapper maintaining additional list of banned senders and contract hashes.

use std::time::Duration;
use std::ops::{Deref, DerefMut};
use aion_types::{H256, U256, Address};
use blake2b::blake2b;
use crate::transaction::{self, Action};
use transient_hashmap::TransientHashMap;
use crate::transaction::transaction_queue::{TransactionQueue, AccountDetails, VerifiedTransaction};

type Count = u16;

/// Auto-Banning threshold
pub enum Threshold {
    /// Should ban after given number of misbehaves reported.
    BanAfter(Count),
    /// Should never ban anything
    NeverBan,
}

impl Default for Threshold {
    fn default() -> Self { Threshold::NeverBan }
}

/// Transaction queue with banlist.
pub struct BanningTransactionQueue {
    queue: TransactionQueue,
    ban_threshold: Threshold,
    senders_bans: TransientHashMap<Address, Count>,
    recipients_bans: TransientHashMap<Address, Count>,
    codes_bans: TransientHashMap<H256, Count>,
}

impl BanningTransactionQueue {
    /// Creates new banlisting transaction queue
    pub fn new(queue: TransactionQueue, ban_threshold: Threshold, ban_lifetime: Duration) -> Self {
        let ban_lifetime_sec = ban_lifetime.as_secs() as u32;
        assert!(
            ban_lifetime_sec > 0,
            "Lifetime has to be specified in seconds."
        );
        BanningTransactionQueue {
            queue: queue,
            ban_threshold: ban_threshold,
            senders_bans: TransientHashMap::new(ban_lifetime_sec),
            recipients_bans: TransientHashMap::new(ban_lifetime_sec),
            codes_bans: TransientHashMap::new(ban_lifetime_sec),
        }
    }

    /// Borrows internal queue.
    /// NOTE: you can insert transactions to the queue even
    /// if they would be rejected because of ban otherwise.
    /// But probably you shouldn't.
    pub fn queue(&mut self) -> &mut TransactionQueue { &mut self.queue }

    /// Add to the queue taking bans into consideration.
    /// May reject transaction because of the banlist.
    pub fn add_with_banlist<F>(
        &mut self,
        transaction: VerifiedTransaction,
        fetch_account: &F,
    ) -> Result<transaction::ImportResult, transaction::Error>
    where
        F: Fn(&Address) -> AccountDetails,
    {
        if let Threshold::BanAfter(threshold) = self.ban_threshold {
            // NOTE In all checks use direct query to avoid increasing ban timeout.

            // Check sender
            let sender = transaction.sender();
            let count = self
                .senders_bans
                .direct()
                .get(&sender)
                .cloned()
                .unwrap_or(0);
            if count > threshold {
                debug!(target: "txqueue", "Ignoring transaction {:?} because sender is banned.", transaction.hash());
                return Err(transaction::Error::SenderBanned);
            }

            // Check recipient
            if let Action::Call(recipient) = transaction.transaction().action {
                let count = self
                    .recipients_bans
                    .direct()
                    .get(&recipient)
                    .cloned()
                    .unwrap_or(0);
                if count > threshold {
                    debug!(target: "txqueue", "Ignoring transaction {:?} because recipient is banned.", transaction.hash());
                    return Err(transaction::Error::RecipientBanned);
                }
            }

            // Check code
            if let Action::Create = transaction.transaction().action {
                let code_hash = blake2b(&transaction.transaction().data);
                let count = self
                    .codes_bans
                    .direct()
                    .get(&code_hash)
                    .cloned()
                    .unwrap_or(0);
                if count > threshold {
                    debug!(target: "txqueue", "Ignoring transaction {:?} because code is banned.", transaction.hash());
                    return Err(transaction::Error::CodeBanned);
                }
            }
        }
        self.queue.add(transaction, fetch_account)
    }

    /// Ban transaction with given hash.
    /// Transaction has to be in the queue.
    ///
    /// Bans sender and recipient/code and returns `true` when any ban has reached threshold.
    pub fn ban_transaction(&mut self, hash: &H256) -> bool {
        let transaction = self.queue.find(hash);
        match transaction {
            Some(transaction) => {
                let sender = transaction.sender();
                // Ban sender
                let sender_banned = self.ban_sender(sender.clone());
                // Ban recipient and codehash
                let recipient_or_code_banned = match transaction.action {
                    Action::Call(recipient) => self.ban_recipient(recipient),
                    Action::Create => self.ban_codehash(blake2b(&transaction.data)),
                };
                sender_banned || recipient_or_code_banned
            }
            None => false,
        }
    }

    /// Ban given sender.
    /// If bans threshold is reached all subsequent transactions from this sender will be rejected.
    /// Reaching bans threshold also removes all existsing transaction from this sender that are already in the
    /// queue.
    fn ban_sender(&mut self, address: Address) -> bool {
        let count = {
            let count = self.senders_bans.entry(address).or_insert_with(|| 0);
            *count = count.saturating_add(1);
            *count
        };
        match self.ban_threshold {
            Threshold::BanAfter(threshold) if count > threshold => {
                // Banlist the sender.
                // Remove all transactions from the queue.
                self.cull(address, U256::max_value());
                true
            }
            _ => false,
        }
    }

    /// Ban given recipient.
    /// If bans threshold is reached all subsequent transactions to this address will be rejected.
    /// Returns true if bans threshold has been reached.
    fn ban_recipient(&mut self, address: Address) -> bool {
        let count = {
            let count = self.recipients_bans.entry(address).or_insert_with(|| 0);
            *count = count.saturating_add(1);
            *count
        };
        match self.ban_threshold {
            // TODO [ToDr] Consider removing other transactions to the same recipient from the queue?
            Threshold::BanAfter(threshold) if count > threshold => true,
            _ => false,
        }
    }

    /// Ban given codehash.
    /// If bans threshold is reached all subsequent transactions to contracts with this codehash will be rejected.
    /// Returns true if bans threshold has been reached.
    fn ban_codehash(&mut self, code_hash: H256) -> bool {
        let count = self.codes_bans.entry(code_hash).or_insert_with(|| 0);
        *count = count.saturating_add(1);

        match self.ban_threshold {
            // TODO [ToDr] Consider removing other transactions with the same code from the queue?
            Threshold::BanAfter(threshold) if *count > threshold => true,
            _ => false,
        }
    }
}

impl Deref for BanningTransactionQueue {
    type Target = TransactionQueue;

    fn deref(&self) -> &Self::Target { &self.queue }
}
impl DerefMut for BanningTransactionQueue {
    fn deref_mut(&mut self) -> &mut Self::Target { self.queue() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use key::generate_keypair;
    use rustc_hex::FromHex;
    use crate::transaction::transaction_queue::TransactionOrigin;
    use crate::transaction::transaction_queue::test::default_account_details;
    use crate::transaction::DEFAULT_TRANSACTION_TYPE;
    use aion_types::{U256, Address};

    fn queue() -> BanningTransactionQueue {
        BanningTransactionQueue::new(
            TransactionQueue::default(),
            Threshold::BanAfter(1),
            Duration::from_secs(180),
        )
    }

    fn transaction(action: Action, origin: TransactionOrigin) -> VerifiedTransaction {
        let keypair = generate_keypair();

        let signed_transaction = transaction::Transaction::new(
            U256::from(123), // nonce
            U256::from(10),
            U256::from(100_000),
            action,
            U256::from(100),
            "3331600055".from_hex().unwrap(),
            DEFAULT_TRANSACTION_TYPE,
            None,
        )
        .sign(keypair.secret());

        VerifiedTransaction::new(signed_transaction, origin, None, 0, 0)
    }

    fn unwrap_err(
        res: Result<transaction::ImportResult, transaction::Error>,
    ) -> transaction::Error {
        res.unwrap_err()
    }

    #[test]
    fn should_allow_to_borrow_the_queue() {
        // given
        let tx = transaction(Action::Create, TransactionOrigin::External);
        let mut txq = queue();
        let fetch_account = |_: &Address| default_account_details();

        // when
        txq.queue().add(tx, &fetch_account).unwrap();

        // then
        // should also deref to queue
        println!("!!!!!!!!!!!!!!!!!!!!!! {}", txq.status().pending);
        assert_eq!(txq.status().pending, 1);
    }

    #[test]
    fn should_not_accept_transactions_from_banned_sender() {
        // given
        let tx = transaction(Action::Create, TransactionOrigin::External);
        let mut txq = queue();
        let fetch_account = |_: &Address| default_account_details();
        // Banlist once (threshold not reached)
        let banlist1 = txq.ban_sender(tx.sender().clone());
        assert!(!banlist1, "Threshold not reached yet.");
        // Insert once
        let import1 = txq.add_with_banlist(tx.clone(), &fetch_account).unwrap();
        assert_eq!(import1, transaction::ImportResult::Current);

        // when
        let banlist2 = txq.ban_sender(tx.sender().clone());
        let import2 = txq.add_with_banlist(tx.clone(), &fetch_account);

        // then
        assert!(banlist2, "Threshold should be reached - banned.");
        assert_eq!(unwrap_err(import2), transaction::Error::SenderBanned);
        // Should also remove transacion from the queue
        assert_eq!(txq.find(&tx.hash()), None);
    }

    #[test]
    fn should_not_accept_transactions_to_banned_recipient() {
        // given
        let recipient = Address::default();
        let tx = transaction(Action::Call(recipient), TransactionOrigin::External);
        let mut txq = queue();
        let fetch_account = |_: &Address| default_account_details();
        // Banlist once (threshold not reached)
        let banlist1 = txq.ban_recipient(recipient);
        assert!(!banlist1, "Threshold not reached yet.");
        // Insert once
        let import1 = txq.add_with_banlist(tx.clone(), &fetch_account).unwrap();
        assert_eq!(import1, transaction::ImportResult::Current);

        // when
        let banlist2 = txq.ban_recipient(recipient);
        let import2 = txq.add_with_banlist(tx.clone(), &fetch_account);

        // then
        assert!(banlist2, "Threshold should be reached - banned.");
        assert_eq!(unwrap_err(import2), transaction::Error::RecipientBanned);
    }

    #[test]
    fn should_not_accept_transactions_with_banned_code() {
        // given
        let tx = transaction(Action::Create, TransactionOrigin::External);
        let codehash = blake2b(&tx.transaction().data);
        let mut txq = queue();
        let fetch_account = |_: &Address| default_account_details();
        // Banlist once (threshold not reached)
        let banlist1 = txq.ban_codehash(codehash);
        assert!(!banlist1, "Threshold not reached yet.");
        // Insert once
        let import1 = txq.add_with_banlist(tx.clone(), &fetch_account).unwrap();
        assert_eq!(import1, transaction::ImportResult::Current);

        // when
        let banlist2 = txq.ban_codehash(codehash);
        let import2 = txq.add_with_banlist(tx.clone(), &fetch_account);

        // then
        assert!(banlist2, "Threshold should be reached - banned.");
        assert_eq!(unwrap_err(import2), transaction::Error::CodeBanned);
    }
}
