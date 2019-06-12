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

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::{self, Duration, Instant};
use std::thread;

use account_provider::AccountProvider;
use aion_types::{Address, H256, U256};
use ansi_term::Colour;
use bytes::Bytes;
use engines::POWEquihashEngine;
use error::*;
use miner::{MinerService, MinerStatus, NotifyWork};
use parking_lot::{Mutex, RwLock};
use transaction::banning_queue::{BanningTransactionQueue, Threshold};
use transaction::local_transactions::{Status as LocalTransactionStatus, TxIoMessage};
use transaction::transaction_queue::{
    AccountDetails, PrioritizationStrategy, RemovalReason,
    TransactionDetailsProvider as TransactionQueueDetailsProvider, TransactionOrigin,
    TransactionQueue,
};
use transaction::{
    Action, Condition as TransactionCondition, Error as TransactionError,
    ImportResult as TransactionImportResult, PendingTransaction, SignedTransaction,
    UnverifiedTransaction,
};
use using_queue::{GetAction, UsingQueue};
use block::{ClosedBlock, IsBlock, Block};
use client::{MiningBlockChainClient, BlockId, TransactionId};
use executive::contract_address;
use header::{Header, BlockNumber};
use io::IoChannel;
use receipt::{Receipt, RichReceipt};
use spec::Spec;
use state::State;

/// Different possible definitions for pending transaction set.
#[derive(Debug, PartialEq)]
pub enum PendingSet {
    /// Always just the transactions in the queue. These have had only cheap checks.
    AlwaysQueue,
    /// Always just the transactions in the sealing block. These have had full checks but
    /// may be empty if the node is not actively mining or has force_sealing enabled.
    AlwaysSealing,
    /// Try the sealing block, but if it is not currently sealing, fallback to the queue.
    SealingOrElseQueue,
}

/// Transaction queue banning settings.
#[derive(Debug, PartialEq, Clone)]
pub enum Banning {
    /// Banning in transaction queue is disabled
    Disabled,
    /// Banning in transaction queue is enabled
    Enabled {
        /// Upper limit of transaction processing time before banning.
        offend_threshold: Duration,
        /// Number of similar offending transactions before banning.
        min_offends: u16,
        /// Number of seconds the offender is banned for.
        ban_duration: Duration,
    },
}

/// Configures the behaviour of the miner.
#[derive(Debug, PartialEq)]
pub struct MinerOptions {
    /// Force the miner to reseal, even when nobody has asked for work.
    pub force_sealing: bool,
    /// Reseal on receipt of new external transactions.
    pub reseal_on_external_tx: bool,
    /// Reseal on receipt of new local transactions.
    pub reseal_on_own_tx: bool,
    /// Minimum period between transaction-inspired reseals.
    pub reseal_min_period: Duration,
    /// Maximum period between blocks (enables force sealing after that).
    pub reseal_max_period: Duration,
    /// Preparing block interval
    pub prepare_block_interval: Duration,
    /// Maximum amount of gas to bother considering for block insertion.
    pub tx_gas_limit: U256,
    /// Maximum memory usage of transactions in the queue (current / future).
    pub tx_queue_memory_limit: Option<usize>,
    /// Strategy to use for prioritizing transactions in the queue.
    pub tx_queue_strategy: PrioritizationStrategy,
    /// Whether we should fallback to providing all the queue's transactions or just pending.
    pub pending_set: PendingSet,
    /// How many historical work packages can we store before running out?
    pub work_queue_size: usize,
    /// Can we submit two different solutions for the same block and expect both to result in an import?
    pub enable_resubmission: bool,
    /// Banning settings.
    pub tx_queue_banning: Banning,
    /// Create a pending block with maximal possible gas limit.
    /// NOTE: Such block will contain all pending transactions but
    /// will be invalid if mined.
    pub infinite_pending_block: bool,
    /// minimal gas price of a transaction to be accepted by the miner/transaction queue
    pub minimal_gas_price: U256,
    /// minimal gas price of a transaction to be accepted by the miner/transaction queue
    pub maximal_gas_price: U256,
    /// maximal gas price of a new local transaction to be accepted by the miner/transaction queue when using dynamic gas price
    pub local_max_gas_price: U256,
}

impl Default for MinerOptions {
    fn default() -> Self {
        MinerOptions {
            force_sealing: false,
            reseal_on_external_tx: false,
            reseal_on_own_tx: true,
            tx_gas_limit: !U256::zero(),
            tx_queue_memory_limit: Some(2 * 1024 * 1024),
            tx_queue_strategy: PrioritizationStrategy::GasPriceOnly,
            pending_set: PendingSet::AlwaysQueue,
            reseal_min_period: Duration::from_secs(4),
            reseal_max_period: Duration::from_secs(120),
            prepare_block_interval: Duration::from_secs(4),
            work_queue_size: 20,
            enable_resubmission: true,
            tx_queue_banning: Banning::Disabled,
            infinite_pending_block: false,
            minimal_gas_price: 10_000_000_000u64.into(),
            maximal_gas_price: 9_000_000_000_000_000_000u64.into(),
            local_max_gas_price: 100_000_000_000u64.into(),
        }
    }
}

struct SealingWork {
    queue: UsingQueue<ClosedBlock>,
    enabled: bool,
}

/// Keeps track of transactions using priority queue and holds currently mined block.
/// Handles preparing work for "work sealing" or seals "internally" if Engine does not require work.
pub struct Miner {
    // NOTE [ToDr]  When locking always lock in this order!
    transaction_queue: Arc<RwLock<BanningTransactionQueue>>,
    transaction_listener: RwLock<Vec<Box<Fn(&[H256]) + Send + Sync>>>,
    sealing_work: Mutex<SealingWork>,
    next_allowed_reseal: Mutex<Instant>,
    sealing_block_last_request: Mutex<u64>,
    // for sealing...
    options: MinerOptions,
    gas_range_target: RwLock<(U256, U256)>,
    author: RwLock<Address>,
    extra_data: RwLock<Bytes>,
    engine: Arc<POWEquihashEngine>,
    accounts: Option<Arc<AccountProvider>>,
    notifiers: RwLock<Vec<Box<NotifyWork>>>,
    tx_message: Mutex<IoChannel<TxIoMessage>>,
}

impl Miner {
    /// Push notifier that will handle new jobs
    pub fn push_notifier(&self, notifier: Box<NotifyWork>) {
        self.notifiers.write().push(notifier);
        self.sealing_work.lock().enabled = true;
    }

    /// Creates new instance of miner Arc.
    pub fn new(
        options: MinerOptions,
        spec: &Spec,
        accounts: Option<Arc<AccountProvider>>,
        message_channel: IoChannel<TxIoMessage>,
    ) -> Arc<Miner>
    {
        Arc::new(Miner::new_raw(options, spec, accounts, message_channel))
    }

    /// Creates new instance of miner.
    fn new_raw(
        options: MinerOptions,
        spec: &Spec,
        accounts: Option<Arc<AccountProvider>>,
        message_channel: IoChannel<TxIoMessage>,
    ) -> Miner
    {
        let mem_limit = options
            .tx_queue_memory_limit
            .unwrap_or_else(usize::max_value);

        let txq = TransactionQueue::with_limits(
            options.tx_queue_strategy,
            mem_limit,
            Mutex::new(message_channel.clone()),
        );
        let txq = match options.tx_queue_banning {
            Banning::Disabled => {
                BanningTransactionQueue::new(txq, Threshold::NeverBan, Duration::from_secs(180))
            }
            Banning::Enabled {
                ban_duration,
                min_offends,
                ..
            } => BanningTransactionQueue::new(txq, Threshold::BanAfter(min_offends), ban_duration),
        };

        let notifiers: Vec<Box<NotifyWork>> = Vec::new();

        Miner {
            transaction_queue: Arc::new(RwLock::new(txq)),
            transaction_listener: RwLock::new(vec![]),
            next_allowed_reseal: Mutex::new(Instant::now()),
            sealing_block_last_request: Mutex::new(0),
            sealing_work: Mutex::new(SealingWork {
                queue: UsingQueue::new(options.work_queue_size),
                enabled: false,
            }),
            gas_range_target: RwLock::new((U256::zero(), U256::zero())),
            author: RwLock::new(Address::default()),
            extra_data: RwLock::new(Vec::new()),
            options,
            accounts,
            engine: spec.engine.clone(),
            notifiers: RwLock::new(notifiers),
            tx_message: Mutex::new(message_channel),
        }
    }

    /// get the interval to prepare a new / update an existing block
    pub fn prepare_block_interval(&self) -> Duration { self.options.prepare_block_interval.clone() }

    /// Replace tx message channel. Useful for testing.
    pub fn set_tx_message_channel(&self, tx_message: IoChannel<TxIoMessage>) {
        *self.tx_message.lock() = tx_message;
    }

    /// Creates new instance of miner with accounts and with given spec.
    pub fn with_spec_and_accounts(spec: &Spec, accounts: Option<Arc<AccountProvider>>) -> Miner {
        Miner::new_raw(
            Default::default(),
            spec,
            accounts,
            IoChannel::disconnected(),
        )
    }

    /// Creates new instance of miner without accounts, but with given spec.
    pub fn with_spec(spec: &Spec) -> Miner {
        Miner::new_raw(Default::default(), spec, None, IoChannel::disconnected())
    }

    fn forced_sealing(&self) -> bool {
        self.options.force_sealing || !self.notifiers.read().is_empty()
    }

    /// Clear all pending block states
    pub fn clear(&self) { self.sealing_work.lock().queue.reset(); }

    /// Get `Some` `clone()` of the current pending block's state or `None` if we're not sealing.
    pub fn pending_state(
        &self,
        latest_block_number: BlockNumber,
    ) -> Option<State<::state_db::StateDB>>
    {
        self.map_pending_block(|b| b.state().clone(), latest_block_number)
    }

    /// Get `Some` `clone()` of the current pending block or `None` if we're not sealing.
    pub fn pending_block(&self, latest_block_number: BlockNumber) -> Option<Block> {
        self.map_pending_block(|b| b.to_base(), latest_block_number)
    }

    /// Get `Some` `clone()` of the current pending block header or `None` if we're not sealing.
    pub fn pending_block_header(&self, latest_block_number: BlockNumber) -> Option<Header> {
        self.map_pending_block(|b| b.header().clone(), latest_block_number)
    }

    /// Set a callback to be notified about imported transactions' hashes.
    pub fn add_transactions_listener(&self, f: Box<Fn(&[H256]) + Send + Sync>) {
        self.transaction_listener.write().push(f);
    }

    fn map_pending_block<F, T>(&self, f: F, latest_block_number: BlockNumber) -> Option<T>
    where F: FnOnce(&ClosedBlock) -> T {
        self.from_pending_block(latest_block_number, || None, |block| Some(f(block)))
    }

    /// Prepares new block for sealing including top transactions from queue.
    fn prepare_block(&self, client: &MiningBlockChainClient) -> (ClosedBlock, Option<H256>) {
        trace_time!("prepare_block");
        let chain_info = client.chain_info();
        let (transactions, mut open_block, original_work_hash) = {
            let transactions = {
                self.transaction_queue.read().top_transactions_at(
                    chain_info.best_block_number,
                    chain_info.best_block_timestamp,
                )
            };
            let mut sealing_work = self.sealing_work.lock();
            let last_work_hash = sealing_work
                .queue
                .peek_last_ref()
                .map(|pb| pb.block().header().hash());
            let best_hash = chain_info.best_block_hash;

            // check to see if last ClosedBlock in would_seals is actually same parent block.
            // if so
            //   duplicate, re-open and push any new transactions.
            //   if at least one was pushed successfully, close and enqueue new ClosedBlock;
            //   otherwise, leave everything alone.
            // otherwise, author a fresh block.
            let mut open_block = match sealing_work
                .queue
                .pop_if(|b| b.block().header().parent_hash() == &best_hash)
            {
                Some(old_block) => {
                    trace!(target: "block", "prepare_block: Already have previous work; updating and returning");
                    // add transactions to old_block
                    client.reopen_block(old_block)
                }
                None => {
                    // block not found - create it.
                    trace!(target: "block", "prepare_block: No existing work - making new block");
                    client.prepare_open_block(
                        self.author(),
                        (self.gas_floor_target(), self.gas_ceil_target()),
                        self.extra_data(),
                    )
                }
            };

            if self.options.infinite_pending_block {
                open_block.set_gas_limit(U256::max_value());
            }

            (transactions, open_block, last_work_hash)
        };

        let mut invalid_transactions = HashSet::new();
        let mut non_allowed_transactions = HashSet::new();
        let mut transactions_to_penalize = HashSet::new();
        let block_number = open_block.block().header().number();

        trace!(target: "block", "prepare_block: block_number: {:?}, parent_block: {:?}", block_number, client.best_block_header().number());

        let mut tx_count: usize = 0;
        let tx_total = transactions.len();
        for tx in transactions {
            let hash = tx.hash();
            let start = Instant::now();
            // Disable transaction permission verification for now.
            // TODO: remove this functionality or keep it?
            // Check whether transaction type is allowed for sender
            // let result = match self.engine.machine().verify_transaction(
            //     &tx,
            //     open_block.header(),
            //     client.as_block_chain_client(),
            // ) {
            //     Err(Error::Transaction(TransactionError::NotAllowed)) => {
            //         Err(TransactionError::NotAllowed.into())
            //     }
            //     _ => open_block.push_transaction(tx, None),
            // };
            let result = open_block.push_transaction(tx, None);
            let took = start.elapsed();

            // Check for heavy transactions
            match self.options.tx_queue_banning {
                Banning::Enabled {
                    ref offend_threshold,
                    ..
                }
                    if &took > offend_threshold =>
                {
                    match self.transaction_queue.write().ban_transaction(&hash) {
                        true => {
                            warn!(target: "block", "Detected heavy transaction. Banning the sender and recipient/code.");
                        }
                        false => {
                            transactions_to_penalize.insert(hash);
                            debug!(target: "block", "Detected heavy transaction. Penalizing sender.")
                        }
                    }
                }
                _ => {}
            }
            trace!(target: "block", "Adding tx {:?} took {:?}", hash, took);
            match result {
                Err(Error::Execution(ExecutionError::BlockGasLimitReached {
                    gas_limit,
                    gas_used,
                    gas,
                })) => {
                    debug!(target: "block", "Skipping adding transaction to block because of gas limit: {:?} (limit: {:?}, used: {:?}, gas: {:?})", hash, gas_limit, gas_used, gas);

                    // Penalize transaction if it's above current gas limit
                    if gas > gas_limit {
                        transactions_to_penalize.insert(hash);
                    }

                    // Exit early if gas left is smaller then min_tx_gas
                    let min_tx_gas: U256 = 21000.into(); // TODO: figure this out properly.
                    if gas_limit - gas_used < min_tx_gas {
                        break;
                    }
                }
                // Invalid nonce error can happen only if previous transaction is skipped because of gas limit.
                // If there is errornous state of transaction queue it will be fixed when next block is imported.
                Err(Error::Execution(ExecutionError::InvalidNonce {
                    expected,
                    got,
                })) => {
                    debug!(target: "block", "Skipping adding transaction to block because of invalid nonce: {:?} (expected: {:?}, got: {:?})", hash, expected, got);
                }
                // already have transaction - ignore
                Err(Error::Transaction(TransactionError::AlreadyImported)) => {}
                Err(Error::Transaction(TransactionError::NotAllowed)) => {
                    non_allowed_transactions.insert(hash);
                    debug!(target: "block",
                           "Skipping non-allowed transaction for sender {:?}",
                           hash);
                }
                Err(e) => {
                    invalid_transactions.insert(hash);
                    debug!(target: "block",
                           "Error adding transaction to block: number={}. transaction_hash={:?}, Error: {:?}",
                           block_number, hash, e);
                }
                Ok(_) => {
                    tx_count += 1;
                } // imported ok
            }
        }
        trace!(target: "block", "Pushed {}/{} transactions", tx_count, tx_total);

        let block = open_block.close();

        let fetch_nonce = |a: &Address| client.latest_nonce(a);

        {
            let mut queue = self.transaction_queue.write();
            for hash in invalid_transactions {
                queue.remove(&hash, &fetch_nonce, RemovalReason::Invalid);
            }
            for hash in non_allowed_transactions {
                queue.remove(&hash, &fetch_nonce, RemovalReason::NotAllowed);
            }
            for hash in transactions_to_penalize {
                queue.penalize(&hash);
            }
        }
        (block, original_work_hash)
    }

    /// Check is reseal is allowed and necessary.
    fn requires_reseal(&self, best_block: BlockNumber) -> bool {
        let has_local_transactions = self
            .transaction_queue
            .read()
            .has_local_pending_transactions();
        let mut sealing_work = self.sealing_work.lock();
        if sealing_work.enabled {
            trace!(target: "block", "requires_reseal: sealing enabled");
            let last_request = *self.sealing_block_last_request.lock();
            // Reseal when:
            // 1. forced sealing OR
            // 2. has local pending transactions OR
            // 3. engine seals internally OR
            // 4. best block is not higher than the last requested block (last time when a rpc
            //    transaction entered or a miner requested work from rpc or stratum) by
            //    SEALING_TIMEOUT_IN_BLOCKS (hard coded 5)
            let should_disable_sealing = !self.forced_sealing()
                && !has_local_transactions
                && self.engine.seals_internally().is_none()
                && best_block > last_request
                && best_block - last_request > SEALING_TIMEOUT_IN_BLOCKS;

            trace!(target: "block", "requires_reseal: should_disable_sealing={}; best_block={}, last_request={}", should_disable_sealing, best_block, last_request);

            if should_disable_sealing {
                trace!(target: "block", "Miner sleeping (current {}, last {})", best_block, last_request);
                sealing_work.enabled = false;
                sealing_work.queue.reset();
                false
            } else {
                // sealing enabled and we don't want to sleep.
                *self.next_allowed_reseal.lock() = Instant::now() + self.options.reseal_min_period;
                true
            }
        } else {
            trace!(target: "block", "requires_reseal: sealing is disabled");
            false
        }
    }

    /// Prepares work which has to be done to seal.
    fn prepare_work(&self, block: ClosedBlock, original_work_hash: Option<H256>) {
        let (work, is_new) = {
            let mut sealing_work = self.sealing_work.lock();
            let last_work_hash = sealing_work
                .queue
                .peek_last_ref()
                .map(|pb| pb.block().header().mine_hash());
            trace!(target: "block", "prepare_work: Checking whether we need to reseal: orig={:?} last={:?}, this={:?}", original_work_hash, last_work_hash, block.block().header().mine_hash());
            let (work, is_new) = if last_work_hash
                .map_or(true, |h| h != block.block().header().mine_hash())
            {
                trace!(target: "block", "prepare_work: Pushing a new, refreshed or borrowed pending {}...", block.block().header().mine_hash());
                let pow_hash = block.block().header().mine_hash();
                let number = block.block().header().number();
                let target = block.block().header().boundary();
                let is_new =
                    original_work_hash.map_or(true, |h| block.block().header().mine_hash() != h);
                sealing_work.queue.push(block);
                // If push notifications are enabled we assume all work items are used.
                if !self.notifiers.read().is_empty() && is_new {
                    sealing_work.queue.use_last_ref();
                }
                (Some((pow_hash, target, number)), is_new)
            } else {
                (None, false)
            };
            trace!(target: "block", "prepare_work: leaving (last={:?})", sealing_work.queue.peek_last_ref().map(|b| b.block().header().mine_hash()));
            (work, is_new)
        };
        if is_new {
            work.map(|(pow_hash, target, _number)| {
                for notifier in self.notifiers.read().iter() {
                    notifier.notify_work(pow_hash, target)
                }
            });
        }
    }

    /// Returns true if we had to prepare new pending block.
    fn prepare_work_sealing(&self, client: &MiningBlockChainClient) -> bool {
        trace!(target: "block", "prepare_work_sealing: entering");
        let prepare_new = {
            let mut sealing_work = self.sealing_work.lock();
            let have_work = sealing_work.queue.peek_last_ref().is_some();
            trace!(target: "block", "prepare_work_sealing: have_work={}", have_work);
            if !have_work {
                sealing_work.enabled = true;
                true
            } else {
                false
            }
        };
        if prepare_new {
            // --------------------------------------------------------------------------
            // | NOTE Code below requires transaction_queue and sealing_work locks.     |
            // | Make sure to release the locks before calling that method.             |
            // --------------------------------------------------------------------------
            let (block, original_work_hash) = self.prepare_block(client);
            self.prepare_work(block, original_work_hash);
        }
        let mut sealing_block_last_request = self.sealing_block_last_request.lock();
        let best_number = client.chain_info().best_block_number;
        if *sealing_block_last_request != best_number {
            trace!(target: "block", "prepare_work_sealing: Miner received request (was {}, now {}) - waking up.", *sealing_block_last_request, best_number);
            *sealing_block_last_request = best_number;
        }

        // Return if we restarted
        prepare_new
    }

    /// Try to prepare a work.
    /// Create a new work if no work exists or update an existing work depending on the
    /// configurations and the current conditions.
    pub fn try_prepare_block(&self, client: &MiningBlockChainClient) {
        // self.prepare_work_sealing()
        self.update_sealing(client);
    }

    /// Verification for mining purpose to determine if a transaction is qualified to
    /// be added into transaction queue.
    fn verify_transaction_miner(
        &self,
        client: &MiningBlockChainClient,
        transaction: SignedTransaction,
    ) -> Result<SignedTransaction, Error>
    {
        // Verify basic gas limit
        let basic_gas: U256 = transaction.gas_required();
        if transaction.gas < basic_gas {
            return Err(Error::Transaction(TransactionError::InsufficientGas {
                minimal: basic_gas,
                got: transaction.gas,
            }));
        }

        // Verify gas price range
        if transaction.gas_price < self.minimal_gas_price()
            || transaction.gas_price > self.maximal_gas_price()
        {
            return Err(Error::Transaction(TransactionError::InvalidGasPriceRange {
                minimal: self.minimal_gas_price(),
                maximal: self.maximal_gas_price(),
                got: transaction.gas_price,
            }));
        }

        // Verify maximal gas limit that this client wishes to mine for a single transaction
        if transaction.gas > self.tx_gas_limit() {
            return Err(Error::Transaction(TransactionError::GasLimitExceeded {
                limit: self.tx_gas_limit(),
                got: transaction.gas,
            }));
        }

        // Verify balance
        let cost: U256 = transaction.value + transaction.gas_price * transaction.gas;
        if client.latest_balance(&transaction.sender()) < cost {
            return Err(Error::Transaction(TransactionError::InsufficientBalance {
                cost,
                balance: client.latest_balance(&transaction.sender()),
            }));
        }

        Ok(transaction)
    }

    /// Verify transaction
    fn verify_transaction(
        &self,
        client: &MiningBlockChainClient,
        transaction: UnverifiedTransaction,
    ) -> Result<SignedTransaction, Error>
    {
        let hash = transaction.hash();
        let best_block_header = client.best_block_header().decode();
        if client
            .transaction_block(TransactionId::Hash(hash))
            .is_some()
        {
            debug!(target: "block", "Rejected tx {:?}: already in the blockchain", hash);
            return Err(Error::Transaction(TransactionError::AlreadyImported));
        }
        match self
            .engine
            .machine()
            .verify_transaction_basic(&transaction)
            .and_then(|_| {
                self.engine
                    .machine()
                    .verify_transaction_signature(transaction, &best_block_header)
            })
            .and_then(|transaction| self.verify_transaction_miner(client, transaction))
        {
            Err(e) => {
                debug!(target: "block", "Rejected tx {:?} with invalid signature: {:?}", hash, e);
                Err(e)
            }
            Ok(transaction) => {
                debug!(target: "rpc_tx", "{:?} tx finished validation [{:?}]", thread::current().id(), time::Instant::now());
                Ok(transaction)
            }
        }
    }

    // fn verify_transactions(
    //     &self,
    //     client: &MiningBlockChainClient,
    //     transactions: Vec<UnverifiedTransaction>,
    // ) -> Vec<Result<SignedTransaction, Error>> {
    //     transactions
    //         .into_iter()
    //         .map(|transaction| {
    //             self.verify_transaction(client, transaction)
    //         })
    //         .collect()
    // }

    fn add_transaction_to_queue(
        &self,
        client: &MiningBlockChainClient,
        transaction: SignedTransaction,
        default_origin: TransactionOrigin,
        condition: Option<TransactionCondition>,
    ) -> Result<TransactionImportResult, Error>
    {
        let insertion_time = client.chain_info().best_block_number;
        let origin = self
            .accounts
            .as_ref()
            .and_then(|accounts| {
                match accounts.has_account(transaction.sender()).unwrap_or(false) {
                    true => Some(TransactionOrigin::Local),
                    false => None,
                }
            })
            .unwrap_or(default_origin);

        let details_provider = TransactionDetailsProvider::new(client);
        let hash = transaction.hash();
        // Be sure to release the lock before we call prepare_work_sealing
        let mut transaction_queue = self.transaction_queue.write();
        let result = match origin {
            TransactionOrigin::Local | TransactionOrigin::RetractedBlock => {
                transaction_queue.add(
                    transaction,
                    origin,
                    insertion_time,
                    condition,
                    &details_provider,
                )?
            }
            TransactionOrigin::External => {
                transaction_queue.add_with_banlist(
                    transaction,
                    insertion_time,
                    &details_provider,
                )?
            }
        };
        debug!(target: "rpc_tx", "{:?} tx finished importing [{:?}]", thread::current().id(), time::Instant::now());

        for listener in &*self.transaction_listener.read() {
            debug!(target: "rpc_tx", "{:?} tx pubsub listener begins [{:?}]", thread::current().id(), time::Instant::now());
            listener(&vec![hash]);
            debug!(target: "rpc_tx", "{:?} tx pubsub listener ends [{:?}]", thread::current().id(), time::Instant::now());
        }
        debug!(target: "rpc_tx", "{:?} tx import ends [{:?}]", thread::current().id(), time::Instant::now());

        Ok(result)
    }

    /// Are we allowed to do a non-mandatory reseal?
    fn tx_reseal_allowed(&self) -> bool { Instant::now() > *self.next_allowed_reseal.lock() }

    fn from_pending_block<H, F, G>(
        &self,
        latest_block_number: BlockNumber,
        from_chain: F,
        map_block: G,
    ) -> H
    where
        F: Fn() -> H,
        G: FnOnce(&ClosedBlock) -> H,
    {
        let sealing_work = self.sealing_work.lock();
        sealing_work.queue.peek_last_ref().map_or_else(
            || from_chain(),
            |b| {
                if b.block().header().number() > latest_block_number {
                    map_block(b)
                } else {
                    from_chain()
                }
            },
        )
    }
}

const SEALING_TIMEOUT_IN_BLOCKS: u64 = 5;

impl MinerService for Miner {
    fn clear_and_reset(&self, client: &MiningBlockChainClient) {
        self.transaction_queue.write().clear();
        // --------------------------------------------------------------------------
        // | NOTE Code below requires transaction_queue and sealing_work locks.     |
        // | Make sure to release the locks before calling that method.             |
        // --------------------------------------------------------------------------
        self.update_sealing(client);
    }

    fn status(&self) -> MinerStatus {
        let status = self.transaction_queue.read().status();
        let sealing_work = self.sealing_work.lock();
        MinerStatus {
            transactions_in_pending_queue: status.pending,
            transactions_in_future_queue: status.future,
            transactions_in_pending_block: sealing_work
                .queue
                .peek_last_ref()
                .map_or(0, |b| b.transactions().len()),
        }
    }

    fn set_author(&self, author: Address) {
        if self.engine.seals_internally().is_some() {
            let mut sealing_work = self.sealing_work.lock();
            sealing_work.enabled = true;
        }
        *self.author.write() = author;
    }

    fn set_extra_data(&self, extra_data: Bytes) { *self.extra_data.write() = extra_data; }

    /// Set the gas limit we wish to target when sealing a new block.
    fn set_gas_floor_target(&self, target: U256) { self.gas_range_target.write().0 = target; }

    fn set_gas_ceil_target(&self, target: U256) { self.gas_range_target.write().1 = target; }

    fn set_minimal_gas_price(&mut self, min_gas_price: U256) {
        self.options.minimal_gas_price = min_gas_price;
    }

    fn minimal_gas_price(&self) -> U256 { self.options.minimal_gas_price }

    fn set_maximal_gas_price(&mut self, max_gas_price: U256) {
        self.options.maximal_gas_price = max_gas_price;
    }

    fn maximal_gas_price(&self) -> U256 { self.options.maximal_gas_price }

    fn local_maximal_gas_price(&self) -> U256 { self.options.local_max_gas_price }

    fn set_local_maximal_gas_price(&mut self, local_max_gas_price: U256) {
        self.options.local_max_gas_price = local_max_gas_price;
    }

    // fn sensible_gas_price(&self) -> U256 {
    //     // 10% above our minimum.
    //     self.minimal_gas_price * 110u32 / 100.into()
    // }

    // fn sensible_gas_limit(&self) -> U256 { self.gas_range_target.read().0 / 5.into() }

    fn default_gas_limit(&self) -> U256 { 2_000_000.into() }

    fn set_tx_gas_limit(&mut self, limit: U256) { self.options.tx_gas_limit = limit; }

    fn tx_gas_limit(&self) -> U256 { self.options.tx_gas_limit }

    /// Get the author that we will seal blocks as.
    fn author(&self) -> Address { *self.author.read() }

    /// Get the extra_data that we will seal blocks with.
    fn extra_data(&self) -> Bytes { self.extra_data.read().clone() }

    /// Get the gas limit we wish to target when sealing a new block.
    fn gas_floor_target(&self) -> U256 { self.gas_range_target.read().0 }

    /// Get the gas limit we wish to target when sealing a new block.
    fn gas_ceil_target(&self) -> U256 { self.gas_range_target.read().1 }

    /// Verify and import external transactions to transaction queue
    fn import_external_transactions(
        &self,
        client: &MiningBlockChainClient,
        transactions: Vec<UnverifiedTransaction>,
    ) -> Vec<Result<TransactionImportResult, Error>>
    {
        trace!(target: "client", "Importing external transactions");
        let mut is_imported: bool = false;

        let results = transactions
            .into_iter()
            .map(|unverified_transaction| {
                self.verify_transaction(client, unverified_transaction)
                    .and_then(|transaction| {
                        let import_result = self.add_transaction_to_queue(
                            client,
                            transaction,
                            TransactionOrigin::External,
                            None,
                        );
                        if !is_imported && import_result.is_ok() {
                            is_imported = true;
                        }
                        import_result
                    })
            })
            .collect();

        if is_imported && self.options.reseal_on_external_tx && self.tx_reseal_allowed() {
            self.update_sealing(client);
        }
        results
    }

    /// Verify and import own transaction to transaction queue
    fn import_own_transaction(
        &self,
        client: &MiningBlockChainClient,
        pending: PendingTransaction,
    ) -> Result<TransactionImportResult, Error>
    {
        trace!(target: "own_tx", "Importing transaction: {:?}", pending);
        debug!(target: "rpc_tx", "{:?} tx start importing [{:?}]", thread::current().id(), time::Instant::now());

        let result = self
            .verify_transaction(client, pending.transaction.clone().into())
            .and_then(|transaction| {
                self.add_transaction_to_queue(
                    client,
                    transaction.into(),
                    TransactionOrigin::Local,
                    pending.condition.clone(),
                )
            });

        match result {
            Ok(_) => {
                debug!(target: "rpc_tx", "{:?} tx start broadcast [{:?}]", thread::current().id(), time::Instant::now());
                client.broadcast_transaction(::rlp::encode(&pending.transaction).into_vec());
            }
            Err(ref e) => {
                let _ = self.tx_message.lock().send(TxIoMessage::Dropped {
                    txhash: pending.hash(),
                    error: format!("Invalid Tx: {}", e),
                });
                warn!(target: "own_tx", "Error importing transaction: {:?}", e);
            }
        }

        // --------------------------------------------------------------------------
        // | NOTE Code below requires transaction_queue and sealing_work locks.     |
        // | Make sure to release the locks before calling that method.             |
        // --------------------------------------------------------------------------
        if result.is_ok() && self.options.reseal_on_own_tx && self.tx_reseal_allowed() {
            // Make sure to do it after transaction is imported and lock is droped.
            // We need to create pending block and enable sealing.
            if self.engine.seals_internally().unwrap_or(false) || !self.prepare_work_sealing(client)
            {
                // If new block has not been prepared (means we already had one)
                // or Engine might be able to seal internally,
                // we need to update sealing.
                debug!(target: "rpc_tx", "{:?} tx start resealing [{:?}]", thread::current().id(), time::Instant::now());
                self.update_sealing(client);
            }
        }
        debug!(target: "rpc_tx", "{:?} tx ready to return [{:?}]", thread::current().id(), time::Instant::now());
        result
    }

    fn pending_transactions(&self) -> Vec<PendingTransaction> {
        let queue = self.transaction_queue.read();
        queue.pending_transactions(BlockNumber::max_value(), u64::max_value())
    }

    fn local_transactions(&self) -> BTreeMap<H256, LocalTransactionStatus> {
        let queue = self.transaction_queue.read();
        queue
            .local_transactions()
            .iter()
            .map(|(hash, status)| (*hash, status.clone()))
            .collect()
    }

    fn future_transactions(&self) -> Vec<PendingTransaction> {
        self.transaction_queue.read().future_transactions()
    }

    fn ready_transactions(
        &self,
        best_block: BlockNumber,
        best_block_timestamp: u64,
    ) -> Vec<PendingTransaction>
    {
        let queue = self.transaction_queue.read();
        match self.options.pending_set {
            PendingSet::AlwaysQueue => queue.pending_transactions(best_block, best_block_timestamp),
            PendingSet::SealingOrElseQueue => {
                self.from_pending_block(
                    best_block,
                    || queue.pending_transactions(best_block, best_block_timestamp),
                    |sealing| {
                        sealing
                            .transactions()
                            .iter()
                            .map(|t| t.clone().into())
                            .collect()
                    },
                )
            }
            PendingSet::AlwaysSealing => {
                self.from_pending_block(
                    best_block,
                    || vec![],
                    |sealing| {
                        sealing
                            .transactions()
                            .iter()
                            .map(|t| t.clone().into())
                            .collect()
                    },
                )
            }
        }
    }

    fn pending_transactions_hashes(&self, best_block: BlockNumber) -> Vec<H256> {
        let queue = self.transaction_queue.read();
        match self.options.pending_set {
            PendingSet::AlwaysQueue => queue.pending_hashes(),
            PendingSet::SealingOrElseQueue => {
                self.from_pending_block(
                    best_block,
                    || queue.pending_hashes(),
                    |sealing| sealing.transactions().iter().map(|t| t.hash()).collect(),
                )
            }
            PendingSet::AlwaysSealing => {
                self.from_pending_block(
                    best_block,
                    || vec![],
                    |sealing| sealing.transactions().iter().map(|t| t.hash()).collect(),
                )
            }
        }
    }

    fn transaction(&self, best_block: BlockNumber, hash: &H256) -> Option<PendingTransaction> {
        let queue = self.transaction_queue.read();
        match self.options.pending_set {
            PendingSet::AlwaysQueue => queue.find(hash),
            PendingSet::SealingOrElseQueue => {
                self.from_pending_block(
                    best_block,
                    || queue.find(hash),
                    |sealing| {
                        sealing
                            .transactions()
                            .iter()
                            .find(|t| &t.hash() == hash)
                            .cloned()
                            .map(Into::into)
                    },
                )
            }
            PendingSet::AlwaysSealing => {
                self.from_pending_block(
                    best_block,
                    || None,
                    |sealing| {
                        sealing
                            .transactions()
                            .iter()
                            .find(|t| &t.hash() == hash)
                            .cloned()
                            .map(Into::into)
                    },
                )
            }
        }
    }

    fn remove_pending_transaction(
        &self,
        chain: &MiningBlockChainClient,
        hash: &H256,
    ) -> Option<PendingTransaction>
    {
        let mut queue = self.transaction_queue.write();
        let tx = queue.find(hash);
        if tx.is_some() {
            let fetch_nonce = |a: &Address| chain.latest_nonce(a);
            queue.remove(hash, &fetch_nonce, RemovalReason::Canceled);
        }
        tx
    }

    fn pending_receipt(&self, best_block: BlockNumber, hash: &H256) -> Option<RichReceipt> {
        self.from_pending_block(
            best_block,
            || None,
            |pending| {
                let txs = pending.transactions();
                txs.iter()
                    .map(|t| t.hash())
                    .position(|t| t == *hash)
                    .map(|index| {
                        let prev_gas = if index == 0 {
                            Default::default()
                        } else {
                            pending.receipts()[index - 1].gas_used
                        };
                        let tx = &txs[index];
                        let receipt = &pending.receipts()[index];
                        RichReceipt {
                            transaction_hash: hash.clone(),
                            transaction_index: index,
                            cumulative_gas_used: receipt.gas_used,
                            gas_used: receipt.gas_used - prev_gas,
                            contract_address: match tx.action {
                                Action::Call(_) => None,
                                Action::Create => {
                                    let sender = tx.sender();
                                    Some(contract_address(&sender, &tx.nonce).0)
                                }
                            },
                            logs: receipt.logs().clone(),
                            log_bloom: receipt.log_bloom().clone(),
                            state_root: receipt.state_root().clone(),
                        }
                    })
            },
        )
    }

    fn pending_receipts(&self, best_block: BlockNumber) -> BTreeMap<H256, Receipt> {
        self.from_pending_block(best_block, BTreeMap::new, |pending| {
            let hashes = pending.transactions().iter().map(|t| t.hash());

            let receipts = pending.receipts().iter().cloned();

            hashes.zip(receipts).collect()
        })
    }

    fn last_nonce(&self, address: &Address) -> Option<U256> {
        self.transaction_queue.read().last_nonce(address)
    }

    fn can_produce_work_package(&self) -> bool { self.engine.seals_internally().is_none() }

    /// Update sealing if required.
    /// Prepare the block and work if the Engine does not seal internally.
    fn update_sealing(&self, client: &MiningBlockChainClient) {
        trace!(target: "block", "update_sealing() best_block: {:?}", client.chain_info().best_block_number);

        if self.requires_reseal(client.chain_info().best_block_number) {
            // --------------------------------------------------------------------------
            // | NOTE Code below requires transaction_queue and sealing_work locks.     |
            // | Make sure to release the locks before calling that method.             |
            // --------------------------------------------------------------------------
            trace!(target: "block", "update_sealing: preparing a block");
            let (block, original_work_hash) = self.prepare_block(client);

            trace!(target: "block", "update_sealing: engine does not seal internally, preparing work");
            self.prepare_work(block, original_work_hash)
        }
    }

    fn is_currently_sealing(&self) -> bool { self.sealing_work.lock().queue.is_in_use() }

    fn map_sealing_work<F, T>(&self, client: &MiningBlockChainClient, f: F) -> Option<T>
    where F: FnOnce(&ClosedBlock) -> T {
        trace!(target: "miner", "map_sealing_work: entering");
        self.prepare_work_sealing(client);
        trace!(target: "miner", "map_sealing_work: sealing prepared");
        let mut sealing_work = self.sealing_work.lock();
        let ret = sealing_work.queue.use_last_ref();
        trace!(target: "miner", "map_sealing_work: leaving use_last_ref={:?}", ret.as_ref().map(|b| b.block().header().hash()));
        ret.map(f)
    }

    fn submit_seal(
        &self,
        client: &MiningBlockChainClient,
        block_hash: H256,
        seal: Vec<Bytes>,
    ) -> Result<(), Error>
    {
        let result = if let Some(b) = self.sealing_work.lock().queue.get_used_if(
            if self.options.enable_resubmission {
                GetAction::Clone
            } else {
                GetAction::Take
            },
            |b| {
                trace!(target: "miner", "Comparing current pending block hash: {:?}, submit hash: {:?}", &block_hash, &b.header().mine_hash());
                &b.header().mine_hash() == &block_hash
            },
        ) {
            trace!(target: "miner", "Submitted block {}={} with seal {:?}", block_hash, b.header().mine_hash(), seal);
            b.lock().try_seal(&*self.engine, seal).or_else(|(e, _)| {
                warn!(target: "miner", "Mined solution rejected: {}", e);
                Err(Error::PowInvalid)
            })
        } else {
            warn!(target: "miner", "Submitted solution rejected: Block unknown or out of date.");
            Err(Error::PowHashInvalid)
        };
        result.and_then(|sealed| {
            let n = sealed.header().number();
            let h = sealed.header().hash();
            client.import_sealed_block(sealed)?;
            info!(target: "miner", "Submitted block imported OK. #{}: {}", Colour::White.bold().paint(format!("{}", n)), Colour::White.bold().paint(format!("{:x}", h)));
            Ok(())
        })
    }

    fn chain_new_blocks(
        &self,
        client: &MiningBlockChainClient,
        imported: &[H256],
        _invalid: &[H256],
        enacted: &[H256],
        retracted: &[H256],
    )
    {
        trace!(target: "block", "chain_new_blocks");

        // 1. We ignore blocks that were `imported` unless resealing on new uncles is enabled.
        // 2. We ignore blocks that are `invalid` because it doesn't have any meaning in terms of the transactions that
        //    are in those blocks

        // Then import all transactions in retracted routes...
        {
            for hash in retracted {
                let block = client.block(BlockId::Hash(*hash)).expect(
                    "Client is sending message after commit to db and inserting to chain; the \
                     block is available; qed",
                );
                let transactions = block.transactions();
                transactions.into_iter().for_each(|unverified_transaction| {
                    let _ = self
                        .verify_transaction(client, unverified_transaction)
                        .and_then(|transaction| {
                            self.add_transaction_to_queue(
                                client,
                                transaction,
                                TransactionOrigin::RetractedBlock,
                                None,
                            )
                        });
                });
            }
        }

        // ...and at the end remove the old ones
        {
            let fetch_account = |a: &Address| {
                AccountDetails {
                    nonce: client.latest_nonce(a),
                    balance: client.latest_balance(a),
                }
            };
            let time = client.chain_info().best_block_number;
            self.transaction_queue
                .write()
                .remove_old(&fetch_account, time);
        }

        if enacted.len() > 0 || (imported.len() > 0 && false) {
            // --------------------------------------------------------------------------
            // | NOTE Code below requires transaction_queue and sealing_work locks.     |
            // | Make sure to release the locks before calling that method.             |
            // --------------------------------------------------------------------------
            self.update_sealing(client);
        }
    }
}

struct TransactionDetailsProvider<'a> {
    client: &'a MiningBlockChainClient,
}

impl<'a> TransactionDetailsProvider<'a> {
    pub fn new(client: &'a MiningBlockChainClient) -> Self {
        TransactionDetailsProvider {
            client,
        }
    }
}

impl<'a> TransactionQueueDetailsProvider for TransactionDetailsProvider<'a> {
    fn fetch_account(&self, address: &Address) -> AccountDetails {
        AccountDetails {
            nonce: self.client.latest_nonce(address),
            balance: self.client.latest_balance(address),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aion_types::U256;
    use keychain;
    use rustc_hex::FromHex;
    use transaction::transaction_queue::PrioritizationStrategy;
    use transaction::Transaction;
    use transaction::Action;
    use client::BlockChainClient;
    use miner::MinerService;
    use tests::helpers::generate_dummy_client;

    fn miner() -> Miner {
        Arc::try_unwrap(Miner::new(
            MinerOptions {
                force_sealing: false,
                reseal_on_external_tx: false,
                reseal_on_own_tx: true,
                reseal_min_period: Duration::from_secs(5),
                reseal_max_period: Duration::from_secs(120),
                prepare_block_interval: Duration::from_secs(5),
                tx_gas_limit: !U256::zero(),
                tx_queue_memory_limit: None,
                tx_queue_strategy: PrioritizationStrategy::GasFactorAndGasPrice,
                pending_set: PendingSet::AlwaysSealing,
                work_queue_size: 5,
                enable_resubmission: true,
                tx_queue_banning: Banning::Disabled,
                infinite_pending_block: false,
                minimal_gas_price: 0u64.into(),
                maximal_gas_price: 9_000_000_000_000_000_000u64.into(),
                local_max_gas_price: 100_000_000_000u64.into(),
            },
            &Spec::new_test(),
            None, // accounts provider
            IoChannel::disconnected(),
        ))
        .ok()
        .expect("Miner was just created.")
    }

    fn transaction() -> SignedTransaction {
        let keypair = keychain::ethkey::generate_keypair();
        Transaction {
            action: Action::Create,
            value: U256::zero(),
            data: "3331600055".from_hex().unwrap(),
            gas: U256::from(300_000),
            gas_price: default_gas_price(),
            nonce: U256::zero(),
            transaction_type: ::transaction::DEFAULT_TRANSACTION_TYPE,
            nonce_bytes: Vec::new(),
            gas_price_bytes: Vec::new(),
            gas_bytes: Vec::new(),
            value_bytes: Vec::new(),
        }
        .sign(keypair.secret().into(), None)
    }

    fn default_gas_price() -> U256 { 0u64.into() }

    #[test]
    fn internal_seals_without_work() {
        let spec = Spec::new_instant();
        let mut miner = Miner::with_spec(&spec);
        miner.set_minimal_gas_price(0.into());

        let client = generate_dummy_client(2);

        assert_eq!(
            miner
                .import_external_transactions(&*client, vec![transaction().into()])
                .pop()
                .unwrap()
                .unwrap(),
            TransactionImportResult::Current
        );

        miner.update_sealing(&*client);
        client.flush_queue();
        assert!(miner.pending_block(0).is_none());
        assert_eq!(client.chain_info().best_block_number, 3 as BlockNumber);

        assert_eq!(
            miner
                .import_own_transaction(
                    &*client,
                    PendingTransaction::new(transaction().into(), None)
                )
                .unwrap(),
            TransactionImportResult::Current
        );

        miner.update_sealing(&*client);
        client.flush_queue();
        assert!(miner.pending_block(0).is_none());
        assert_eq!(client.chain_info().best_block_number, 4 as BlockNumber);
    }
}
