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

use std::collections::{BTreeMap, HashSet, HashMap};
use std::sync::Arc;
use std::time::{self, Duration, Instant, SystemTime, UNIX_EPOCH};
use std::thread;
use std::cmp::max;

use rustc_hex::FromHex;
use account_provider::AccountProvider;
use acore_bytes::Bytes;
use aion_types::{Address, H256, U256};
use ansi_term::Colour;
use block::{Block, ClosedBlock, IsBlock, SealedBlock};
use client::{BlockId, MiningBlockChainClient, TransactionId};
use engine::Engine;
use header::{BlockNumber, Header, SealType};
use types::error::*;
use io::IoChannel;
use miner::{MinerService, MinerStatus};
use parking_lot::{Mutex, RwLock};
use receipt::Receipt;
use spec::Spec;
use state::State;
use transaction::{
    Condition as TransactionCondition,
    Error as TransactionError,
    PendingTransaction,
    SignedTransaction,
    UnverifiedTransaction,
};
use transaction::banning_queue::{BanningTransactionQueue, Threshold};
use transaction::local_transactions::TxIoMessage;
use transaction::transaction_pool::TransactionPool;
use transaction::transaction_queue::{
    AccountDetails, PrioritizationStrategy, RemovalReason, TransactionOrigin, TransactionQueue,
};
use using_queue::{GetAction, UsingQueue};
use rcrypto::ed25519;
use key::{Ed25519KeyPair, public_to_address_ed25519};
use num_bigint::BigUint;
use blake2b::blake2b;

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
    /// Minimum period between transaction-inspired reseals.
    pub reseal_min_period: Duration,
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
    /// maximal gas price of a transaction to be accepted by the miner/transaction queue
    pub maximal_gas_price: U256,
    /// maximal gas price of a new local transaction to be accepted by the miner/transaction queue when using dynamic gas price
    pub local_max_gas_price: U256,
    /// Staker private key
    pub staker_private_key: Option<String>,
}

impl Default for MinerOptions {
    fn default() -> Self {
        MinerOptions {
            force_sealing: false,
            tx_gas_limit: !U256::zero(),
            tx_queue_memory_limit: Some(2 * 1024 * 1024),
            tx_queue_strategy: PrioritizationStrategy::GasPriceOnly,
            pending_set: PendingSet::AlwaysQueue,
            reseal_min_period: Duration::from_secs(4),
            prepare_block_interval: Duration::from_secs(4),
            work_queue_size: 20,
            enable_resubmission: true,
            tx_queue_banning: Banning::Disabled,
            infinite_pending_block: false,
            minimal_gas_price: 10_000_000_000u64.into(),
            maximal_gas_price: 9_000_000_000_000_000_000u64.into(),
            local_max_gas_price: 100_000_000_000u64.into(),
            staker_private_key: None,
        }
    }
}

struct SealingWork {
    queue: UsingQueue<ClosedBlock>,
    enabled: bool,
}

/// Keeps track of transactions using priority queue and holds currently mined block.
/// Handles preparing work for "work sealing".
pub struct Miner {
    // NOTE [ToDr]  When locking always lock in this order!
    transaction_pool: TransactionPool,
    sealing_work: Mutex<SealingWork>,
    // PoS block queue
    maybe_work: Mutex<HashMap<H256, ClosedBlock>>,
    // the current PoS block with minimum timestamp
    best_pos: Mutex<Option<ClosedBlock>>,
    next_allowed_reseal: Mutex<Instant>,
    sealing_block_last_request: Mutex<u64>,
    // for sealing...
    options: MinerOptions,
    gas_range_target: RwLock<(U256, U256)>,
    author: RwLock<Address>,
    // TOREMOVE: Unity MS1 use only.
    staker: Option<Ed25519KeyPair>,
    extra_data: RwLock<Bytes>,
    engine: Arc<Engine>,
    accounts: Option<Arc<AccountProvider>>,
    tx_message: Mutex<IoChannel<TxIoMessage>>,
    transaction_pool_update_lock: Mutex<bool>,
}

impl Miner {
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

    /// get the interval to prepare a new / update an existing block
    pub fn prepare_block_interval(&self) -> Duration { self.options.prepare_block_interval.clone() }

    /// Creates new instance of miner without accounts, but with given spec.
    pub fn with_spec(spec: &Spec) -> Miner {
        Miner::new_raw(Default::default(), spec, None, IoChannel::disconnected())
    }

    /// Clear all pending block states
    pub fn clear(&self) { self.sealing_work.lock().queue.reset(); }

    /// Get `Some` `clone()` of the current pending block's state or `None` if we're not sealing.
    pub fn pending_state(&self, latest_block_number: BlockNumber) -> Option<State<::db::StateDB>> {
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

    /// Try to prepare a work.
    /// Create a new work if no work exists or update an existing work depending on the
    /// configurations and the current conditions.
    pub fn try_prepare_block(&self, client: &MiningBlockChainClient, is_forced: bool) {
        if is_forced || self.tx_reseal_allowed() {
            *self.next_allowed_reseal.lock() = Instant::now() + self.options.reseal_min_period;
            self.update_sealing(client);
        }
    }

    pub fn invoke_pos_interval(&self, client: &MiningBlockChainClient) -> Result<(), Error> {
        let chain_best_pos_block = client
            .best_block_header_with_seal_type(&SealType::PoS)
            .map(|header| header.decode());
        
        let mut queue = self.maybe_work.lock();
        let mut pending_best = self.best_pos.lock();
        
        let timestamp_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
            let block = pending_best.clone();
            match block {
                Some(b) => {
                     if b.header().timestamp() <= timestamp_now {
                        let seal = b.header().seal();
                        let stake = client.get_stake(b.header().author());
                        if let Ok(sealed) = b.clone().lock().try_seal_pos(
                            &*self.engine,
                            seal.to_owned(),
                            chain_best_pos_block.as_ref(),
                            stake,
                        ) {
                            if chain_best_pos_block.as_ref().unwrap().hash()
                                == b.header().parent_hash().to_owned()
                            {
                                let n = sealed.header().number();
                                let d = sealed.header().difficulty().clone();
                                let h = sealed.header().hash();
                                let t = sealed.header().timestamp();

                                // 4. Import block
                                client.import_sealed_block(sealed)?;

                                // Log
                                info!(target: "miner", "PoS block reimported OK. #{}: diff: {}, hash: {}, timestamp: {}",
                                    Colour::White.bold().paint(format!("{}", n)),
                                    Colour::White.bold().paint(format!("{}", d)),
                                    Colour::White.bold().paint(format!("{:x}", h)),
                                    Colour::White.bold().paint(format!("{:x}", t)));
                            }
                            queue.clear();
                            *pending_best = None;
                        }
                    }
                },
                None => {},
            }

        Ok(())
    }

    /// Try to generate PoS block if minimum resealing duration is met
    pub fn try_prepare_block_pos(&self, client: &MiningBlockChainClient) -> Result<(), Error> {
        // Not before the Unity fork point
        if self
            .engine
            .machine()
            .params()
            .unity_update
            .map_or(true, |fork_number| {
                client.chain_info().best_block_number + 1 < fork_number
            }) {
            return Ok(());
        }

        // Return if no internal staker
        if self.staker.is_none() {
            return Ok(());
        }

        // Staker
        let staker: Ed25519KeyPair = self
            .staker()
            .to_owned()
            .expect("Internal staker is null. Should have checked before.");
        let sk: [u8; 64] = staker.secret().0;
        let pk: [u8; 32] = staker.public().0;
        let address: Address = staker.address();

        // 1. Get the stake. Stop proceeding if stake is 0.
        let stake: u64 = match client.get_stake(&address) {
            Some(stake) if stake > 0 => stake,
            _ => return Ok(()),
        };

        // 2. Get the current best PoS block
        let best_block_header = client.best_block_header_with_seal_type(&SealType::PoS);

        // 3. Get the timestamp, the seed and the seal parent of the best PoS block
        let timestamp_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let (timestamp, seed, seal_parent) = match &best_block_header {
            Some(header) => {
                let seed: Bytes = header
                    .seal()
                    .get(0)
                    .expect("A pos block has to contain a seed")
                    .to_owned();
                (
                    header.timestamp(),
                    seed,
                    client.seal_parent_header(&header.parent_hash(), &header.seal_type()),
                )
            }
            None => (timestamp_now - 1u64, Vec::new(), None), // TODO-Unity: To handle the first PoS block better
        };

        // 4. Calculate difficulty
        let difficulty = client.calculate_difficulty(
            best_block_header
                .clone()
                .map(|header| header.decode())
                .as_ref(),
            seal_parent.map(|header| header.decode()).as_ref(),
        );

        // 5. Calcualte timestamp for the new PoS block
        // TODO-Unity: don't use floating number to calculate this
        // \Delta = \frac{d_s \cdot ln({2^{256}}/{hash(seed)})}{V}.
        let new_seed = ed25519::signature(&seed, &sk);
        let hash_of_seed = blake2b(&new_seed[..]);
        let a = BigUint::parse_bytes(
            b"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            16,
        )
        .unwrap();
        let b = BigUint::from_bytes_be(&hash_of_seed[..]);
        let u = ln(&a).unwrap() - ln(&b).unwrap();
        let delta = (difficulty.as_u64() as f64) * u / (stake as f64);

        trace!(target: "staker", "Staking...difficulty: {}, u: {}, stake: {}, delta: {}",
               difficulty.as_u64(), u, stake, delta);
        let new_timestamp = timestamp + max(1u64, delta as u64);

        // 6. Determine if we can produce a new PoS block or not
        if timestamp_now >= new_timestamp {
            self.prepare_block_pos(
                client,
                timestamp_now, // TODO-Unity: Or use new_timestamp?
                new_seed,
                &sk,
                &pk,
                best_block_header.map(|header| header.decode()).as_ref(),
                stake,
            )
        } else {
            Ok(())
        }
    }

    // TOREMOVE-Unity: Unity MS1 use only
    /// Generate PoS block
    pub fn prepare_block_pos(
        &self,
        client: &MiningBlockChainClient,
        timestamp: u64,
        seed: [u8; 64],
        sk: &[u8; 64],
        pk: &[u8; 32],
        seal_parent: Option<&Header>,
        stake: u64,
    ) -> Result<(), Error>
    {
        trace!(target: "block", "Generating pos block. Current best block: {:?}", client.chain_info().best_block_number);

        // 1. Create a block with transactions
        let (raw_block, _): (ClosedBlock, Option<H256>) =
            self.prepare_block(client, &Some(SealType::PoS), Some(timestamp));

        // 2. Generate signature
        let mine_hash: H256 = raw_block.header().mine_hash();
        let signature = ed25519::signature(&mine_hash.0, sk);

        // 3. Seal the block
        let mut seal: Vec<Bytes> = Vec::new();
        seal.push(seed.to_vec());
        seal.push(signature.to_vec());
        seal.push(pk.to_vec());
        let sealed_block: SealedBlock = raw_block
            .lock()
            .try_seal_pos(&*self.engine, seal, seal_parent, Some(stake))
            .or_else(|(e, _)| {
                warn!(target: "miner", "Staking seal rejected: {}", e);
                Err(Error::PosInvalid)
            })?;

        // Log
        let n = sealed_block.header().number();
        let d = sealed_block.header().difficulty().clone();
        let h = sealed_block.header().hash();
        let t = sealed_block.header().timestamp();

        // 4. Import block
        client.import_sealed_block(sealed_block)?;

        // Log
        info!(target: "miner", "PoS block imported OK. #{}: diff: {}, hash: {}, timestamp: {}",
            Colour::White.bold().paint(format!("{}", n)),
            Colour::White.bold().paint(format!("{}", d)),
            Colour::White.bold().paint(format!("{:x}", h)),
            Colour::White.bold().paint(format!("{:x}", t)));
        Ok(())
    }

    /// Update transaction pool
    pub fn update_transaction_pool(&self, client: &MiningBlockChainClient, is_forced: bool) {
        let update_lock = self.transaction_pool_update_lock.try_lock();
        if !is_forced && update_lock.is_none() {
            return;
        }
        let fetch_account = |a: &Address| {
            AccountDetails {
                nonce: client.latest_nonce(a),
                balance: client.latest_balance(a),
            }
        };
        let best_block = || client.chain_info().best_block_number;
        self.transaction_pool.update(&fetch_account, &best_block);
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

        let transaction_queue = TransactionQueue::with_limits(
            options.tx_queue_strategy,
            mem_limit,
            Mutex::new(message_channel.clone()),
        );
        let transaction_queue = match options.tx_queue_banning {
            Banning::Disabled => {
                BanningTransactionQueue::new(
                    transaction_queue,
                    Threshold::NeverBan,
                    Duration::from_secs(180),
                )
            }
            Banning::Enabled {
                ban_duration,
                min_offends,
                ..
            } => {
                BanningTransactionQueue::new(
                    transaction_queue,
                    Threshold::BanAfter(min_offends),
                    ban_duration,
                )
            }
        };

        let transaction_pool: TransactionPool =
            TransactionPool::new(RwLock::new(transaction_queue));

        // TOREMOVE: Unity MS1 use only
        let staker: Option<Ed25519KeyPair> = match options.staker_private_key.to_owned() {
            Some(key) => parse_staker(key).ok(),
            None => None,
        };

        Miner {
            transaction_pool,
            next_allowed_reseal: Mutex::new(Instant::now()),
            sealing_block_last_request: Mutex::new(0),
            sealing_work: Mutex::new(SealingWork {
                queue: UsingQueue::new(options.work_queue_size),
                enabled: false,
            }),
            maybe_work: Mutex::new(HashMap::new()),
            best_pos: Mutex::new(None),
            gas_range_target: RwLock::new((U256::zero(), U256::zero())),
            author: RwLock::new(Address::default()),
            staker: staker,
            extra_data: RwLock::new(Vec::new()),
            options,
            accounts,
            engine: spec.engine.clone(),
            tx_message: Mutex::new(message_channel),
            transaction_pool_update_lock: Mutex::new(true),
        }
    }

    fn forced_sealing(&self) -> bool { self.options.force_sealing }

    fn map_pending_block<F, T>(&self, f: F, latest_block_number: BlockNumber) -> Option<T>
    where F: FnOnce(&ClosedBlock) -> T {
        self.from_pending_block(latest_block_number, || None, |block| Some(f(block)))
    }

    /// Prepares new block for sealing including top transactions from queue.
    fn prepare_block(
        &self,
        client: &MiningBlockChainClient,
        seal_type: &Option<SealType>,
        timestamp: Option<u64>,
    ) -> (ClosedBlock, Option<H256>)
    {
        trace_time!("prepare_block");
        let chain_info = client.chain_info();
        let (transactions, mut open_block, original_work_hash) = {
            let transactions = {
                self.transaction_pool.top_transactions(
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

            let mut open_block = match seal_type {
                Some(SealType::PoS) => {
                    // let author: Address = self
                    //     .staker()
                    //     .to_owned()
                    //     .expect(
                    //         "staker key not specified in configuration. Should have checked \
                    //          before.",
                    //     )
                    //     .address();
                    let author = self.author();
                    client.prepare_open_block(
                        author,
                        (self.gas_floor_target(), self.gas_ceil_target()),
                        self.extra_data(),
                        seal_type.to_owned(),
                        timestamp,
                    )
                }
                _ => {
                    match sealing_work
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
                            let author: Address = self.author();
                            client.prepare_open_block(
                                author,
                                (self.gas_floor_target(), self.gas_ceil_target()),
                                self.extra_data(),
                                seal_type.to_owned(),
                                timestamp,
                            )
                        }
                    }
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
            let hash = tx.hash().clone();
            let start = Instant::now();
            let result = open_block.push_transaction(tx, None, true);
            let took = start.elapsed();

            // Check for heavy transactions
            match self.options.tx_queue_banning {
                Banning::Enabled {
                    ref offend_threshold,
                    ..
                }
                    if &took > offend_threshold =>
                {
                    match self.transaction_pool.ban_transaction(&hash) {
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
            trace!(target: "block", "Adding tx {:?} took {:?}", &hash, took);
            match result {
                Err(Error::Execution(ExecutionError::BlockGasLimitReached {
                    gas_limit,
                    gas_used,
                    gas,
                })) => {
                    debug!(target: "block", "Skipping adding transaction to block because of gas limit: {:?} (limit: {:?}, used: {:?}, gas: {:?})", &hash, gas_limit, gas_used, gas);

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
                    debug!(target: "block", "Skipping adding transaction to block because of invalid nonce: {:?} (expected: {:?}, got: {:?})", &hash, expected, got);
                }
                // already have transaction - ignore
                Err(Error::Transaction(TransactionError::AlreadyImported)) => {}
                Err(Error::Transaction(TransactionError::NotAllowed)) => {
                    non_allowed_transactions.insert(hash);
                    debug!(target: "block",
                           "Skipping non-allowed transaction for sender {:?}",
                           &hash);
                }
                Err(e) => {
                    invalid_transactions.insert(hash);
                    debug!(target: "block",
                           "Error adding transaction to block: number={}. transaction_hash={:?}, Error: {:?}",
                           block_number, &hash, e);
                }
                Ok(_) => {
                    tx_count += 1;
                } // imported ok
            }
        }
        debug!(target: "block", "Pushed {}/{} transactions", tx_count, tx_total);

        let block = open_block.close();

        invalid_transactions.iter().for_each(|hash| {
            self.transaction_pool
                .remove_transaction(*hash, RemovalReason::Invalid);
        });

        non_allowed_transactions.iter().for_each(|hash| {
            self.transaction_pool
                .remove_transaction(*hash, RemovalReason::NotAllowed);
        });

        transactions_to_penalize.iter().for_each(|hash| {
            self.transaction_pool.penalize(hash);
        });

        (block, original_work_hash)
    }

    /// Check if reseal is allowed and necessary.
    fn requires_reseal(&self, best_block: BlockNumber) -> bool {
        let has_local_transactions = self.transaction_pool.has_local_pending_transactions();
        let mut sealing_work = self.sealing_work.lock();
        if sealing_work.enabled {
            trace!(target: "block", "requires_reseal: sealing enabled");
            let last_request = *self.sealing_block_last_request.lock();
            // Reseal when:
            // 1. forced sealing OR
            // 2. has local pending transactions OR
            // 3. best block is not higher than the last requested block (last time when a rpc
            //    transaction entered or a miner requested work from rpc or stratum) by
            //    SEALING_TIMEOUT_IN_BLOCKS (hard coded 5)
            let should_disable_sealing = !self.forced_sealing()
                && !has_local_transactions
                && best_block > last_request
                && best_block - last_request > SEALING_TIMEOUT_IN_BLOCKS;

            trace!(target: "block", "requires_reseal: should_disable_sealing={}; best_block={}, last_request={}", should_disable_sealing, best_block, last_request);

            if should_disable_sealing {
                trace!(target: "block", "Miner sleeping (current {}, last {})", best_block, last_request);
                sealing_work.enabled = false;
                sealing_work.queue.reset();
                false
            } else {
                true
            }
        } else {
            trace!(target: "block", "requires_reseal: sealing is disabled");
            false
        }
    }

    /// Prepares work which has to be done to seal.
    fn prepare_work(&self, block: ClosedBlock, original_work_hash: Option<H256>) {
        let mut sealing_work = self.sealing_work.lock();
        let last_work_hash = sealing_work
            .queue
            .peek_last_ref()
            .map(|pb| pb.block().header().mine_hash());
        trace!(target: "block", "prepare_work: Checking whether we need to reseal: orig={:?} last={:?}, this={:?}", original_work_hash, last_work_hash, block.block().header().mine_hash());
        if last_work_hash.map_or(true, |h| h != block.block().header().mine_hash()) {
            trace!(target: "block", "prepare_work: Pushing a new, refreshed or borrowed pending {}...", block.block().header().mine_hash());
            let _pow_hash = block.block().header().mine_hash();
            let _number = block.block().header().number();
            let _target = block.block().header().boundary();
            let is_new =
                original_work_hash.map_or(true, |h| block.block().header().mine_hash() != h);
            sealing_work.queue.push(block);
            // If push notifications are enabled we assume all work items are used.
            if is_new {
                sealing_work.queue.use_last_ref();
            }
        };
        trace!(target: "block", "prepare_work: leaving (last={:?})", sealing_work.queue.peek_last_ref().map(|b| b.block().header().mine_hash()));
    }

    /// Returns true if we had to prepare new pending block.
    fn prepare_work_sealing(
        &self,
        client: &MiningBlockChainClient,
        seal_type: &Option<SealType>,
    ) -> bool
    {
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
            let (block, original_work_hash) = self.prepare_block(client, seal_type, None);
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

    /// Verification for mining purpose to determine if a transaction is qualified to
    /// be added into transaction queue.
    fn verify_transaction_miner(
        &self,
        client: &MiningBlockChainClient,
        transaction: SignedTransaction,
    ) -> Result<SignedTransaction, Error>
    {
        // Verify nonce
        if transaction.nonce < client.latest_nonce(&transaction.sender()) {
            return Err(Error::Transaction(TransactionError::Old));
        }

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
        let hash = transaction.hash().clone();
        let best_block_header = client.best_block_header().decode();
        if client
            .transaction_block(TransactionId::Hash(hash.clone()))
            .is_some()
        {
            debug!(target: "block", "Rejected tx {:?}: already in the blockchain", &hash);
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
                debug!(target: "block", "Rejected tx {:?} with invalid signature: {:?}", &hash, e);
                Err(e)
            }
            Ok(transaction) => {
                debug!(target: "rpc_tx", "{:?} tx finished validation [{:?}]", thread::current().id(), time::Instant::now());
                Ok(transaction)
            }
        }
    }

    fn add_transaction_to_queue(
        &self,
        client: &MiningBlockChainClient,
        transaction: SignedTransaction,
        default_origin: TransactionOrigin,
        condition: Option<TransactionCondition>,
    ) -> Result<(), Error>
    {
        let insertion_block = client.chain_info().best_block_number;
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

        let result = self.transaction_pool.add_transaction(
            transaction,
            origin,
            condition,
            insertion_block,
        )?;
        debug!(target: "rpc_tx", "{:?} tx finished importing [{:?}]", thread::current().id(), time::Instant::now());

        Ok(result)
    }

    /// Are we allowed to do a non-mandatory reseal?
    fn tx_reseal_allowed(&self) -> bool {
        Instant::now() >= (*self.next_allowed_reseal.lock() - Duration::from_millis(100))
    }

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

    #[cfg(test)]
    /// Replace tx message channel. Useful for testing.
    pub fn set_tx_message_channel(&self, tx_message: IoChannel<TxIoMessage>) {
        *self.tx_message.lock() = tx_message;
    }

    #[cfg(test)]
    /// Creates new instance of miner with accounts and with given spec.
    pub fn with_spec_and_accounts(spec: &Spec, accounts: Option<Arc<AccountProvider>>) -> Miner {
        Miner::new_raw(
            Default::default(),
            spec,
            accounts,
            IoChannel::disconnected(),
        )
    }
}

const SEALING_TIMEOUT_IN_BLOCKS: u64 = 5;

impl MinerService for Miner {
    fn clear_and_reset(&self, client: &MiningBlockChainClient) {
        self.transaction_pool.clear();
        self.update_sealing(client);
    }

    /// MinerStatus     -   pending transaction number
    ///                 -   future transaction number
    ///                 -   transaction number in pending block
    fn status(&self) -> MinerStatus {
        let status = self.transaction_pool.status();
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

    fn set_author(&self, author: Address) { *self.author.write() = author; }

    fn set_staker(&mut self, staker: Ed25519KeyPair) { self.staker = Some(staker); }

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

    fn default_gas_limit(&self) -> U256 { 2_000_000.into() }

    fn tx_gas_limit(&self) -> U256 { self.options.tx_gas_limit }

    /// Get the author that we will seal blocks as.
    fn author(&self) -> Address { *self.author.read() }

    /// Get the PoS staker that we will seal PoS blocks.
    fn staker(&self) -> &Option<Ed25519KeyPair> { &self.staker }

    /// Get the extra_data that we will seal blocks with.
    fn extra_data(&self) -> Bytes { self.extra_data.read().clone() }

    /// Get the gas limit we wish to target when sealing a new block.
    fn gas_floor_target(&self) -> U256 { self.gas_range_target.read().0 }

    /// Get the gas limit we wish to target when sealing a new block.
    fn gas_ceil_target(&self) -> U256 { self.gas_range_target.read().1 }

    /// Verify and import external transactions to transaction queue, from client
    fn import_external_transactions(
        &self,
        client: &MiningBlockChainClient,
        transactions: Vec<UnverifiedTransaction>,
    ) -> Vec<Result<(), Error>>
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

        results
    }

    /// Verify and import own transaction to transaction queue, tx from rpc
    fn import_own_transaction(
        &self,
        client: &MiningBlockChainClient,
        pending: PendingTransaction,
    ) -> Result<(), Error>
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
                    txhash: pending.hash().clone(),
                    error: format!("Invalid Tx: {}", e),
                });
                warn!(target: "own_tx", "Error importing transaction: {:?}", e);
            }
        }

        debug!(target: "rpc_tx", "{:?} tx ready to return [{:?}]", thread::current().id(), time::Instant::now());
        result
    }

    /// Get all Pending Transactions
    fn pending_transactions(&self) -> Vec<PendingTransaction> {
        self.transaction_pool
            .pending_transactions(BlockNumber::max_value(), u64::max_value())
    }

    //    fn local_transactions(&self) -> HashMap<H256, LocalTransactionStatus> {
    //        self.transaction_pool.local_transactions()
    //    }

    // Return all future transactions, and transfer them to Pending
    fn future_transactions(&self) -> Vec<PendingTransaction> {
        self.transaction_pool.future_transactions()
    }

    /// Called by client
    fn ready_transactions(
        &self,
        best_block: BlockNumber,
        best_block_timestamp: u64,
    ) -> Vec<PendingTransaction>
    {
        match self.options.pending_set {
            PendingSet::AlwaysQueue => {
                self.transaction_pool
                    .pending_transactions(best_block, best_block_timestamp)
            }
            PendingSet::SealingOrElseQueue => {
                self.from_pending_block(
                    best_block,
                    || {
                        self.transaction_pool
                            .pending_transactions(best_block, best_block_timestamp)
                    },
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

    /// part of eth filter
    fn pending_transactions_hashes(&self, best_block: BlockNumber) -> Vec<H256> {
        match self.options.pending_set {
            PendingSet::AlwaysQueue => self.transaction_pool.pending_hashes(),
            PendingSet::SealingOrElseQueue => {
                self.from_pending_block(
                    best_block,
                    || self.transaction_pool.pending_hashes(),
                    |sealing| {
                        sealing
                            .transactions()
                            .iter()
                            .map(|t| t.hash().clone())
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
                            .map(|t| t.hash().clone())
                            .collect()
                    },
                )
            }
        }
    }

    /// try to find transaction util best_block, rpc uses this
    fn transaction(&self, best_block: BlockNumber, hash: &H256) -> Option<PendingTransaction> {
        match self.options.pending_set {
            PendingSet::AlwaysQueue => self.transaction_pool.find_transaction(hash),
            PendingSet::SealingOrElseQueue => {
                self.from_pending_block(
                    best_block,
                    || self.transaction_pool.find_transaction(hash),
                    |sealing| {
                        sealing
                            .transactions()
                            .iter()
                            .find(|t| t.hash() == hash)
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
                            .find(|t| t.hash() == hash)
                            .cloned()
                            .map(Into::into)
                    },
                )
            }
        }
    }

    //    fn remove_pending_transaction(&self, hash: H256) {
    //        self.transaction_pool
    //            .remove_transaction(hash, RemovalReason::Canceled);
    //    }

    //    fn pending_receipt(&self, best_block: BlockNumber, hash: &H256) -> Option<RichReceipt> {
    //        self.from_pending_block(
    //            best_block,
    //            || None,
    //            |pending| {
    //                let txs = pending.transactions();
    //                txs.iter()
    //                    .map(|t| t.hash())
    //                    .position(|t| t == hash)
    //                    .map(|index| {
    //                        let prev_gas = if index == 0 {
    //                            Default::default()
    //                        } else {
    //                            pending.receipts()[index - 1].gas_used
    //                        };
    //                        let tx = &txs[index];
    //                        let receipt = &pending.receipts()[index];
    //                        RichReceipt {
    //                            transaction_hash: hash.clone(),
    //                            transaction_index: index,
    //                            cumulative_gas_used: receipt.gas_used,
    //                            gas_used: receipt.gas_used - prev_gas,
    //                            contract_address: match tx.action {
    //                                Action::Call(_) => None,
    //                                Action::Create => {
    //                                    let sender = tx.sender();
    //                                    Some(contract_address(&sender, &tx.nonce).0)
    //                                }
    //                            },
    //                            logs: receipt.logs().clone(),
    //                            log_bloom: receipt.log_bloom().clone(),
    //                            state_root: receipt.state_root().clone(),
    //                        }
    //                    })
    //            },
    //        )
    //    }

    // rpc related
    fn pending_receipts(&self, best_block: BlockNumber) -> BTreeMap<H256, Receipt> {
        self.from_pending_block(best_block, BTreeMap::new, |pending| {
            let hashes = pending.transactions().iter().map(|t| t.hash().clone());

            let receipts = pending.receipts().iter().cloned();

            hashes.zip(receipts).collect()
        })
    }

    // rpc related
    fn last_nonce(&self, address: &Address) -> Option<U256> {
        self.transaction_pool.last_nonce(address)
    }

    /// Client: Prepare new best block or update existing best block if required.
    fn update_sealing(&self, client: &MiningBlockChainClient) {
        trace!(target: "block", "update_sealing: best_block: {:?}", client.chain_info().best_block_number);
        if self.requires_reseal(client.chain_info().best_block_number) {
            trace!(target: "block", "update_sealing: preparing a block");
            let (block, original_work_hash) =
                self.prepare_block(client, &Some(SealType::PoW), None);
            self.prepare_work(block, original_work_hash)
        }
    }

    fn add_sealing_pos(
        &self,
        hash: &H256,
        b: ClosedBlock,
        _client: &MiningBlockChainClient,
    ) -> Result<(), Error>
    {
        self.maybe_work.lock().insert(*hash, b);
        Ok(())
    }

    fn get_ready_pos(&self, hash: &H256) -> Option<(ClosedBlock, Vec<Bytes>)> {
        match self.maybe_work.lock().get(hash) {
            Some(b) => {
                let seal = b.header().seal();
                Some((b.clone(), seal.clone().to_vec()))
            }
            _ => None,
        }
    }

    fn clear_pos_pending(&self) {
        // let mut queue = self.maybe_work.lock();
        // let mut best_pos = self.best_pos.lock();
        // queue.clear();
        // *best_pos = None;
    }

    fn get_pos_template(
        &self,
        client: &MiningBlockChainClient,
        seed: [u8; 64],
        pk: H256,
    ) -> Option<H256>
    {
        let address = public_to_address_ed25519(&pk);
        let stake = client.get_stake(&address).unwrap_or(0);
        if stake == 0 {
            return None;
        }

        let best_block_header = client.best_block_header_with_seal_type(&SealType::PoS);

        let timestamp_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let (timestamp, seal_parent) = match &best_block_header {
            Some(header) => {
                (
                    header.timestamp(),
                    client.seal_parent_header(&header.parent_hash(), &header.seal_type()),
                )
            }
            None => (timestamp_now - 1u64, None), // TODO-Unity: To handle the first PoS block better
        };

        let difficulty = client.calculate_difficulty(
            best_block_header
                .clone()
                .map(|header| header.decode())
                .as_ref(),
            seal_parent.map(|header| header.decode()).as_ref(),
        );

        debug!(target: "miner", "new block difficulty = {:?}", difficulty);

        let hash_of_seed = blake2b(&seed[..]);
        let a = BigUint::parse_bytes(
            b"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            16,
        )
        .unwrap();
        let b = BigUint::from_bytes_be(&hash_of_seed[..]);
        let u = ln(&a).unwrap() - ln(&b).unwrap();
        let delta = (difficulty.as_u64() as f64) * u / (stake as f64);

        trace!(target: "staker", "Staking...difficulty: {}, u: {}, stake: {}, delta: {}",
               difficulty.as_u64(), u, stake, delta);
        let new_timestamp = timestamp + max(1u64, delta as u64);

        self.set_author(address);

        let (raw_block, _): (ClosedBlock, Option<H256>) =
            self.prepare_block(client, &Some(SealType::PoS), Some(new_timestamp));

        let mut seal = Vec::with_capacity(3);
        seal.push(seed.to_vec());
        seal.push(vec![0u8; 64]);
        seal.push(pk.to_vec());

        let block = raw_block.pre_seal(seal);

        let hash = block.header().mine_hash();

        match self.add_sealing_pos(&hash, block, client) {
            Ok(_) => Some(hash),
            _ => None,
        }
    }

    // public key must be in seal[2]
    fn try_seal_pos(
        &self,
        client: &MiningBlockChainClient,
        seal: Vec<Bytes>,
        block: ClosedBlock,
    ) -> Result<(), Error>
    {
        let address = public_to_address_ed25519(&seal[2][..].into());
        let best_block_header = client.best_block_header_with_seal_type(&SealType::PoS);

        let mut queue = self.maybe_work.lock();
        let stake = client.get_stake(&address);
        let mut best_pos = self.best_pos.lock();

        debug!(target: "miner", "start sealing");

        // try seal block
        // TODO: avoid clone
        match block.clone().lock().try_seal_pos(
            &*self.engine,
            seal,
            best_block_header.map(|header| header.decode()).as_ref(),
            stake,
        ) {
            Ok(s) => {
                let n = s.header().number();
                let d = s.header().difficulty().clone();
                let h = s.header().hash();
                let t = s.header().timestamp();

                client.import_sealed_block(s)?;

                // Log
                info!(target: "miner", "PoS block imported OK. #{}: diff: {}, hash: {}, timestamp: {}",
                Colour::White.bold().paint(format!("{}", n)),
                Colour::White.bold().paint(format!("{}", d)),
                Colour::White.bold().paint(format!("{:x}", h)),
                Colour::White.bold().paint(format!("{:x}", t)));

                *best_pos = None;
                queue.clear();
                return Ok(());
            }
            Err((e, _)) => {
                debug!(target: "miner", "{:?}", e);
                match e {
                    Error::Block(BlockError::InvalidPoSTimestamp(t1, _, _)) => {
                        if best_pos.is_some() && best_pos.clone().unwrap().header().timestamp() > t1
                        {
                            *best_pos = Some(block.clone());
                        }
                    }
                    _ => { }
                }
                return Err(Error::from(e));
            }
        }
    }

    /// RPC
    fn is_currently_sealing(&self) -> bool { self.sealing_work.lock().queue.is_in_use() }

    // Stratum server receives a finished job, and updates sealing work
    fn map_sealing_work<F, T>(&self, client: &MiningBlockChainClient, f: F) -> Option<T>
    where F: FnOnce(&ClosedBlock) -> T {
        trace!(target: "miner", "map_sealing_work: entering");
        self.prepare_work_sealing(client, &None); // TODO-Unity-Rpc staking: handle PoW and PoS better
        trace!(target: "miner", "map_sealing_work: sealing prepared");
        let mut sealing_work = self.sealing_work.lock();
        let ret = sealing_work.queue.use_last_ref();
        trace!(target: "miner", "map_sealing_work: leaving use_last_ref={:?}", ret.as_ref().map(|b| b.block().header().hash()));
        ret.map(f)
    }

    /// stratum server receives a finished job, import as a sealed block
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
            b.lock().try_seal_pow(&*self.engine, seal).or_else(|(e, _)| {
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
            let d = sealed.header().difficulty().clone();
            client.import_sealed_block(sealed)?;
            info!(target: "miner", "Submitted block imported OK. #{}: {}: {}", Colour::White.bold().paint(format!("{}", n)), Colour::White.bold().paint(format!("{:x}", h)), Colour::White.bold().paint(format!("{:x}", d)));
            Ok(())
        })
    }

    /// Client
    fn chain_new_blocks(
        &self,
        client: &MiningBlockChainClient,
        _imported: &[H256],
        _invalid: &[H256],
        _enacted: &[H256],
        retracted: &[H256],
    )
    {
        trace!(target: "block", "chain_new_blocks");

        // Import all transactions in retracted routes...
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

        self.transaction_pool.record_transaction_sealed();
        self.clear_pos_pending();
        client.new_block_chained();
    }
}

// TOREMOVE-Unity: Unity MS1 use only
fn parse_staker(key: String) -> Result<Ed25519KeyPair, String> {
    let bytes: Vec<u8>;
    if key.starts_with("0x") {
        bytes = String::from(&key[2..])
            .from_hex()
            .map_err(|_| "Private key is not hex string.")?;
    } else {
        bytes = key
            .from_hex()
            .map_err(|_| "Private key is not hex string.")?;
    }
    if bytes.len() != 32 {
        return Err("Private key length is not 32.".to_owned());
    }
    let mut sk = [0; 32];
    sk.copy_from_slice(&bytes[..]);
    let (secret, public): ([u8; 64], [u8; 32]) = ed25519::keypair(&sk);
    let mut keypair: Vec<u8> = secret.to_vec();
    keypair.extend(public.to_vec());
    Ok(keypair.into())
}

// TODO-Unity: To do this better
fn ln(x: &BigUint) -> Result<f64, String> {
    let x: Vec<u8> = x.to_bytes_le();

    const BYTES: usize = 12;
    let start = if x.len() < BYTES { 0 } else { x.len() - BYTES };

    let mut n: f64 = 0.0;
    for i in start..x.len() {
        n = n / 256f64 + (x[i] as f64);
    }
    let ln_256: f64 = (256f64).ln();

    Ok(n.ln() + ln_256 * ((x.len() - 1) as f64))
}

#[cfg(test)]
mod tests {
    use aion_types::U256;
    use block::IsBlock;
    use io::IoChannel;
    use keychain;
    use miner::{Miner, MinerService};
    use rustc_hex::FromHex;
    use spec::Spec;
    use std::sync::Arc;
    use std::time::Duration;
    use super::{Banning, MinerOptions, PendingSet};
    use tests::common::{EachBlockWith, TestBlockChainClient};
    use transaction::{PendingTransaction, SignedTransaction};
    use transaction::Action;
    use transaction::Transaction;
    use transaction::transaction_queue::PrioritizationStrategy;

    #[test]
    fn should_prepare_block_to_seal() {
        // given
        let client = TestBlockChainClient::default();
        let miner = Miner::with_spec(&Spec::new_test());

        // when
        let sealing_work = miner.map_sealing_work(&client, |_| ());
        assert!(sealing_work.is_some(), "Expected closed block");
    }

    #[test]
    fn should_still_work_after_a_couple_of_blocks() {
        // given
        let client = TestBlockChainClient::default();
        let miner = Miner::with_spec(&Spec::new_test());

        let res = miner.map_sealing_work(&client, |b| b.block().header().mine_hash());
        assert!(res.is_some());
        assert!(miner.submit_seal(&client, res.unwrap(), vec![]).is_ok());

        // two more blocks mined, work requested.
        client.add_blocks(1, EachBlockWith::Nothing);
        miner.map_sealing_work(&client, |b| b.block().header().mine_hash());

        client.add_blocks(1, EachBlockWith::Nothing);
        miner.map_sealing_work(&client, |b| b.block().header().mine_hash());

        // solution to original work submitted.
        assert!(miner.submit_seal(&client, res.unwrap(), vec![]).is_ok());
    }

    fn miner() -> Miner {
        Arc::try_unwrap(Miner::new(
            MinerOptions {
                force_sealing: false,
                reseal_min_period: Duration::from_secs(5),
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
                staker_private_key: None,
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
        .sign(keypair.secret(), None)
    }

    fn default_gas_price() -> U256 { 0u64.into() }

    #[test]
    fn should_make_pending_block_when_importing_own_transaction() {
        // given
        let client = TestBlockChainClient::default();
        let miner = miner();
        let transaction = transaction();
        let best_block = 0;
        // when
        let res = miner.import_own_transaction(&client, PendingTransaction::new(transaction, None));
        // then
        assert!(res.is_ok());
        miner.update_transaction_pool(&client, true);
        miner.prepare_work_sealing(&client, &None);
        assert_eq!(miner.pending_transactions().len(), 1);
        assert_eq!(miner.ready_transactions(best_block, 0).len(), 1);
        assert_eq!(miner.pending_transactions_hashes(best_block).len(), 1);
        assert_eq!(miner.pending_receipts(best_block).len(), 1);
        // This method will let us know if pending block was created (before calling that method)
        assert!(!miner.prepare_work_sealing(&client, &None));
    }

    #[test]
    fn should_not_use_pending_block_if_best_block_is_higher() {
        // given
        let client = TestBlockChainClient::default();
        let miner = miner();
        let transaction = transaction();
        let best_block = 10;
        // when
        let res = miner.import_own_transaction(&client, PendingTransaction::new(transaction, None));
        // then
        assert!(res.is_ok());
        miner.update_transaction_pool(&client, true);
        miner.prepare_work_sealing(&client, &None);
        assert_eq!(miner.pending_transactions().len(), 1);
        assert_eq!(miner.ready_transactions(best_block, 0).len(), 0);
        assert_eq!(miner.pending_transactions_hashes(best_block).len(), 0);
        assert_eq!(miner.pending_receipts(best_block).len(), 0);
    }

    //
    #[test]
    fn should_import_external_transaction() {
        // given
        let client = TestBlockChainClient::default();
        let miner = miner();
        let transaction = transaction().into();
        let best_block = 0;
        // when
        let res = miner
            .import_external_transactions(&client, vec![transaction])
            .pop()
            .unwrap();
        // then
        assert!(res.is_ok());
        miner.update_transaction_pool(&client, true);
        // miner.prepare_work_sealing(&client);
        assert_eq!(miner.pending_transactions().len(), 1);
        assert_eq!(miner.pending_transactions_hashes(best_block).len(), 0);
        assert_eq!(miner.ready_transactions(best_block, 0).len(), 0);
        assert_eq!(miner.pending_receipts(best_block).len(), 0);
        // This method will let us know if pending block was created (before calling that method)
        assert!(miner.prepare_work_sealing(&client, &None));
    }

    #[test]
    fn should_not_seal_unless_enabled() {
        let miner = miner();
        let client = TestBlockChainClient::default();
        // By default resealing is not required.
        assert!(!miner.requires_reseal(1u8.into()));

        miner
            .import_external_transactions(&client, vec![transaction().into()])
            .pop()
            .unwrap()
            .unwrap();
        assert!(miner.prepare_work_sealing(&client, &None));
        // Unless asked to prepare work.
        assert!(miner.requires_reseal(1u8.into()));
    }
}
