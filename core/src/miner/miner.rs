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
use std::ops::Deref;
use std::hash::{Hash, Hasher};

use rustc_hex::FromHex;
use account_provider::AccountProvider;
use acore_bytes::{Bytes, slice_to_array_80, slice_to_array_64};
use aion_types::{Address, H256, U256};
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
use rcrypto::{ed25519, ecvrf};
use key::Ed25519KeyPair;
use num::Zero;
use num_bigint::BigUint;
use delta_calc::calculate_delta;
use key::public_to_address_ed25519;
use blake2b::blake2b;

const POW_UPDATE_COOLDOWN: Duration = Duration::from_secs(1);

struct Seed([u8; 64]);

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
            work_queue_size: 100,
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

impl Hash for Seed {
    fn hash<H: Hasher>(&self, state: &mut H) { self.0.hash(state); }
}

impl PartialEq for Seed {
    fn eq(&self, other: &Self) -> bool {
        for index in 0..64 {
            if self.0[index] != other.0[index] {
                return false;
            }
        }
        return true;
    }
}

impl Eq for Seed {}

struct ReadyPoSWork {
    block: ClosedBlock,
    ready_time: u64,
}

impl ReadyPoSWork {
    fn require_pos_reseal(&self, expected_ready_time: u64) -> bool {
        expected_ready_time >= self.ready_time
    }
}

impl Deref for ReadyPoSWork {
    type Target = ClosedBlock;

    fn deref(&self) -> &Self::Target { &self.block }
}

/// Keeps track of transactions using priority queue and holds currently mined block.
/// Handles preparing work for "work sealing".
pub struct Miner {
    // NOTE [ToDr]  When locking always lock in this order!
    transaction_pool: TransactionPool,
    // Cache of best block pow block templates
    sealing_work_pow: Mutex<SealingWork>,
    // PoS block queue
    maybe_work: Mutex<HashMap<H256, ReadyPoSWork>>,
    // a seed/block_hash map for resealing
    sealing_work_pos: Mutex<HashMap<Seed, H256>>,
    // the current PoS block with minimum timestamp
    best_pos: Mutex<Option<SealedBlock>>,
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
    pub fn clear(&self) { self.sealing_work_pow.lock().queue.reset(); }

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

    /// Try to prepare a PoW work.
    /// Create a new work if no work exists or update an existing work depending on the
    /// configurations and the current conditions.
    pub fn try_prepare_block_pow(&self, client: &MiningBlockChainClient, is_forced: bool) {
        // Create new PoW work only when the current best is PoS, and cooldown allows.
        if (is_forced || self.reseal_cooldown_reached())
            && self.new_block_allowed_with_seal_type(client, &SealType::PoW)
        {
            self.update_reseal_cooldown();
            trace!(target: "miner", "update_sealing: best_block: {:?}", client.chain_info().best_block_number);
            // TODO: consider remove or simplify this condition
            if self.requires_reseal(client.chain_info().best_block_number) {
                trace!(target: "miner", "update_sealing: preparing a block");
                if let Ok((block, original_work_hash)) =
                    self.prepare_block(client, &Some(SealType::PoW), None, None, None)
                {
                    self.prepare_work(block, original_work_hash)
                }
            }
        }
    }

    pub fn invoke_pos_interval(&self, client: &MiningBlockChainClient) {
        // compete with import_lock, if another is imported, block will be None, or else try importing pending_best
        let block = {
            let mut pending_best = self.best_pos.lock();
            pending_best.clone()
        };

        match block {
            Some(sealed) => {
                // Check if the block is still fresh
                let best_hash = client.chain_info().best_block_hash;
                let parent_hash = sealed.header().parent_hash().clone();
                if best_hash != parent_hash {
                    return;
                }

                // Check if it's time to import the block
                let timestamp_now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                if sealed.header().timestamp() <= timestamp_now {
                    // 4. Import block
                    if let Some(error) = client.import_sealed_block(sealed).err() {
                        debug!(target: "miner", "PoS block imported error: {}", error);
                    }
                }
            }
            None => {}
        }
    }

    /// Try to internally generate PoS block if minimum resealing duration is met
    pub fn try_produce_pos_block_internal(&self, client: &MiningBlockChainClient) {
        // Not before the Unity fork point
        if !self.new_block_allowed_with_seal_type(client, &SealType::PoS) {
            return;
        }

        // Return if no internal staker
        if self.staker.is_none() {
            return;
        }

        // Staker
        let staker: Ed25519KeyPair = self
            .staker()
            .to_owned()
            .expect("Internal staker is null. Should have checked before.");
        let sk: [u8; 64] = staker.secret().0;
        let pk: [u8; 32] = staker.public().0;
        let coinbase: Address = client
            .get_coinbase(staker.address())
            .unwrap_or(Address::default());

        // 1. Get the stake. Stop proceeding if stake is 0.
        // internal staker's coinbase is himself
        let stake: BigUint = client
            .get_stake(&pk.into(), coinbase, BlockId::Latest)
            .unwrap_or(BigUint::from(0u32));

        if stake == BigUint::from(0u32) {
            return;
        }

        // 2. Get the current best block
        let best_block_header = client.best_block_header();

        // 3. Get the previous seed; Get the timestamp, the grand / great grand parents of the best block
        let timestamp = best_block_header.timestamp();
        let grand_parant = client.block_header_data(&best_block_header.parent_hash());
        let great_grand_parent = grand_parant.clone().map_or(None, |header| {
            client.block_header_data(&header.parent_hash())
        });
        let latest_seed: Vec<u8> = match self.latest_seed(client) {
            Ok(result) => result,
            Err(_) => return,
        };

        // 4. Calculate difficulty
        let difficulty = client.calculate_difficulty(
            &best_block_header.decode(),
            grand_parant.clone().map(|header| header.decode()).as_ref(),
            great_grand_parent.map(|header| header.decode()).as_ref(),
        );

        // 5. Calcualte timestamp for the new PoS block
        // Unity-3 ecvrf seed
        let mut proof: Option<[u8; 80]> = None;
        let new_seed: [u8; 64] = if self.unity_ecvrf_seed_update(client) {
            match Self::generate_ecvrf_seed(&latest_seed, &sk) {
                Ok((ecvrf_proof, ecvrf_seed)) => {
                    proof = Some(ecvrf_proof);
                    ecvrf_seed
                }
                Err(_) => {
                    return;
                }
            }
        }
        // Unity-2 hybrid seed
        else if self.unity_hybrid_seed_update(client) {
            Self::generate_hybrid_seed(&latest_seed, &pk, &best_block_header.decode())
        }
        // Unity-1 signature seed
        else {
            ed25519::signature(&latest_seed, &sk)
        };

        let delta_uint = calculate_delta(difficulty, &new_seed, stake.clone());

        let new_timestamp = timestamp + delta_uint;

        // 6. Determine if we can produce a new PoS block or not
        let timestamp_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        trace!(target: "staker", "time now: {}, expected: {}", timestamp_now, new_timestamp);
        if timestamp_now >= new_timestamp {
            if let Some(error) = self
                .produce_pos_block_internal(
                    client,
                    new_timestamp,
                    new_seed,
                    &sk,
                    &pk,
                    &best_block_header.decode(),
                    grand_parant.map(|header| header.decode()).as_ref(),
                    stake,
                    coinbase,
                    proof,
                )
                .err()
            {
                debug!(target: "staker", "Internal PoS block creation failed: {}", error);
            }
        }
    }

    // TOREMOVE-Unity: Unity MS1 use only
    /// Generate PoS block
    /// sk/pk is public/private key of signer
    pub fn produce_pos_block_internal(
        &self,
        client: &MiningBlockChainClient,
        timestamp: u64,
        seed: [u8; 64],
        sk: &[u8; 64],
        pk: &[u8; 32],
        parent: &Header,
        grand_parant: Option<&Header>,
        stake: BigUint,
        coinbase: Address,
        proof: Option<[u8; 80]>,
    ) -> Result<(), Error>
    {
        trace!(target: "block", "Generating pos block. Current best block: {:?}", client.chain_info().best_block_number);

        // 1. Create a block with transactions
        let (raw_block, _): (ClosedBlock, Option<H256>) = self
            .prepare_block(
                client,
                &Some(SealType::PoS),
                Some(timestamp),
                Some(coinbase),
                Some(&seed),
            )
            .or_else(|()| {
                debug!(target: "miner", "Current work's seal type equals to best block's seal type");
                Err(Error::Other("PoS mining is not allowed.".to_string()))
            })?;

        // 2. Generate signature
        let mut preseal = Vec::with_capacity(3);
        // Unity-3: ecvrf seal
        if let Some(proof) = proof {
            preseal.push(proof.to_vec());
        } else {
            preseal.push(seed.to_vec());
        }
        preseal.push(vec![0u8; 64]);
        preseal.push(pk.to_vec());
        let presealed_block = raw_block.pre_seal(preseal);
        let mine_hash: H256 = presealed_block.header().mine_hash();
        let signature = ed25519::signature(&mine_hash.0, sk);

        // 3. Seal the block
        let mut seal: Vec<Bytes> = Vec::with_capacity(3);
        // Unity-3: ecvrf seal
        if let Some(proof) = proof {
            seal.push(proof.to_vec());
        } else {
            seal.push(seed.to_vec());
        }
        seal.push(signature.to_vec());
        seal.push(pk.to_vec());
        let sealed_block: SealedBlock = presealed_block
            .lock()
            .try_seal_pos(&*self.engine, seal, parent, grand_parant, Some(stake))
            .or_else(|(e, _)| {
                debug!(target: "miner", "Staking seal rejected: {}", e);
                Err(Error::PosInvalid)
            })?;

        // 4. Import block
        client.import_sealed_block(sealed_block)?;
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
            sealing_work_pow: Mutex::new(SealingWork {
                queue: UsingQueue::new(options.work_queue_size),
                enabled: false,
            }),
            maybe_work: Mutex::new(HashMap::new()),
            sealing_work_pos: Mutex::new(HashMap::new()),
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
        author: Option<Address>,
        seed: Option<&[u8; 64]>,
    ) -> Result<(ClosedBlock, Option<H256>), ()>
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

            let mut sealing_work_pow = self.sealing_work_pow.lock();
            let last_work_hash = sealing_work_pow
                .queue
                .peek_last_ref()
                .map(|pb| pb.block().header().hash());

            let mut open_block = match seal_type {
                Some(SealType::PoS) => {
                    let mut maybe_work = self.maybe_work.lock();
                    let mut resealing_work = self.sealing_work_pos.lock();
                    assert!(seed.is_some());
                    let hash = resealing_work.get(&Seed(*seed.unwrap()));
                    match maybe_work.get(hash.unwrap_or(&H256::default())) {
                        Some(b) => {
                            let timestamp_now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            if !b.require_pos_reseal(
                                timestamp_now - self.options.reseal_min_period.as_secs(),
                            ) {
                                return Ok((b.block.clone(), Some(*hash.unwrap())));
                            }
                            debug!(target: "miner", "reopen pos block");
                            // add transactions to old_block
                            client.reopen_block(b.block.clone())
                        }
                        None => {
                            client.prepare_open_block(
                                author.unwrap_or(Address::default()),
                                (self.gas_floor_target(), self.gas_ceil_target()),
                                self.extra_data(),
                                seal_type.to_owned(),
                                timestamp,
                            )
                        }
                    }
                }
                _ => {
                    // Create new pow block template and rerun all transactions. No longer reopen previous block template (ARK-126)
                    trace!(target: "block", "prepare_block: prepare new pow block template");
                    client.prepare_open_block(
                        self.author(),
                        (self.gas_floor_target(), self.gas_ceil_target()),
                        self.extra_data(),
                        seal_type.to_owned(),
                        timestamp,
                    )
                }
            };

            if self.options.infinite_pending_block {
                open_block.set_gas_limit(U256::max_value());
            }

            (transactions, open_block, last_work_hash)
        };

        // AION 2.0
        // Only create new block on top of a block with opposite seal type
        if !self.new_block_allowed_with_seal_type(
            client,
            &open_block
                .block()
                .header()
                .seal_type()
                .clone()
                .unwrap_or_default(),
        ) {
            return Err(());
        }

        let mut invalid_transactions = HashSet::new();
        let mut non_allowed_transactions = HashSet::new();
        let mut transactions_to_penalize = HashSet::new();
        let mut transactions_with_invalid_beacon = HashMap::new();
        let block_number = open_block.block().header().number();

        trace!(target: "block", "prepare_block: block_number: {:?}, parent_block: {:?}", block_number, client.best_block_header().number());

        let mut tx_count: usize = 0;
        let update_unity = self.engine.machine().params().unity_update;
        let tx_total = transactions.len();
        for tx in transactions {
            if let Some(num) = update_unity {
                if block_number <= num && tx.beacon.is_some() {
                    invalid_transactions.insert(tx.hash().clone());
                    continue;
                } else if block_number > num {
                    if let Some(hash) = tx.beacon {
                        if client.is_beacon_hash(&hash).is_none() {
                            transactions_with_invalid_beacon.insert(tx.hash().clone(), hash);
                            continue;
                        }
                    }
                }
            }
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

        transactions_with_invalid_beacon
            .iter()
            .for_each(|(hash, beacon)| {
                self.transaction_pool
                    .remove_transaction(*hash, RemovalReason::InvalidBeaconHash(*beacon));
            });

        Ok((block, original_work_hash))
    }

    /// Check if reseal is allowed and necessary.
    fn requires_reseal(&self, best_block: BlockNumber) -> bool {
        let has_local_transactions = self.transaction_pool.has_local_pending_transactions();
        let mut sealing_work_pow = self.sealing_work_pow.lock();
        if sealing_work_pow.enabled {
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
                sealing_work_pow.enabled = false;
                sealing_work_pow.queue.reset();
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
        let mut sealing_work_pow = self.sealing_work_pow.lock();
        let last_work_hash = sealing_work_pow
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
            sealing_work_pow.queue.push(block);
            // If push notifications are enabled we assume all work items are used.
            if is_new {
                sealing_work_pow.queue.use_last_ref();
            }
        };
        trace!(target: "block", "prepare_work: leaving (last={:?})", sealing_work_pow.queue.peek_last_ref().map(|b| b.block().header().mine_hash()));
    }

    /// Returns true if we had to prepare new pending block.
    fn prepare_work_sealing(&self, client: &MiningBlockChainClient) -> bool {
        trace!(target: "block", "prepare_work_sealing: entering");
        let prepare_new = {
            let mut sealing_work_pow = self.sealing_work_pow.lock();
            let have_work = sealing_work_pow.queue.peek_last_ref().is_some();
            trace!(target: "block", "prepare_work_sealing: have_work={}", have_work);
            if !have_work {
                sealing_work_pow.enabled = true;
                true
            } else {
                false
            }
        };
        if prepare_new {
            if let Ok((block, original_work_hash)) =
                self.prepare_block(client, &Some(SealType::PoW), None, None, None)
            {
                self.prepare_work(block, original_work_hash);
            } else {
                return false;
            }
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
        let best_block_header = client.best_block_header().decode();
        match self.engine.machine().params().unity_update {
            Some(update_num) if best_block_header.number() >= update_num => {
                if let Some(ref hash) = transaction.beacon {
                    if client.is_beacon_hash(hash).is_none() {
                        return Err(Error::Transaction(TransactionError::InvalidBeaconHash(
                            *hash,
                        )));
                    }
                }
            }
            _ => {
                if transaction.beacon.is_some() {
                    return Err(Error::Transaction(TransactionError::BeaconBanned));
                }
            }
        }

        let hash = transaction.hash().clone();
        if client
            .transaction_block(TransactionId::Hash(hash.clone()))
            .is_some()
        {
            debug!(target: "block", "Rejected tx {:?}: already in the blockchain", &hash);
            return Err(Error::Transaction(TransactionError::AlreadyImported));
        }
        // TIP: enable fork property right after fork point block
        match self
            .engine
            .machine()
            .verify_transaction_basic(&transaction, Some(best_block_header.number() + 1))
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

    /// Check if it's allowed to do a non-mandatory reseal
    fn reseal_cooldown_reached(&self) -> bool {
        Instant::now() >= (*self.next_allowed_reseal.lock() - Duration::from_millis(100))
    }

    /// Update cooldown time for the next non-mandatory reseal
    fn update_reseal_cooldown(&self) {
        *self.next_allowed_reseal.lock() = Instant::now() + POW_UPDATE_COOLDOWN;
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
        let sealing_work_pow = self.sealing_work_pow.lock();
        sealing_work_pow.queue.peek_last_ref().map_or_else(
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

    /// Clear pending blocks and block templates
    fn clear_pending_blocks(&self) {
        // Clear pending PoS blocks and block templates
        let mut queue = self.maybe_work.lock();
        let mut best_pos = self.best_pos.lock();
        self.sealing_work_pos.lock().clear();
        queue.clear();
        *best_pos = None;
        // Clear pening PoW block templates
        self.sealing_work_pow.lock().queue.reset();
    }

    // Unity-2
    // Generate hybrid seed for pos block
    fn generate_hybrid_seed(
        grand_parent_seed: &[u8],
        pk: &[u8],
        parent_header: &Header,
    ) -> [u8; 64]
    {
        let mut hybrid_seed: Vec<u8> = Vec::new();
        let signing_address: Address = public_to_address_ed25519(&H256::from(pk));
        let parent_mine_hash: H256 = parent_header.mine_hash();
        let parent_nonce: &[u8] = &parent_header.seal()[0];
        // X = PoS-seed_n-1 || Signing-addr || Pow-HeaderHashForMiners_n-1 || Pow-nonce_n-1
        hybrid_seed.extend(grand_parent_seed);
        hybrid_seed.extend(&signing_address.to_vec());
        hybrid_seed.extend(&parent_mine_hash.to_vec());
        hybrid_seed.extend(parent_nonce);
        // left = X || 0
        let mut hybrid_left: Vec<u8> = Vec::new();
        hybrid_left.extend(&hybrid_seed);
        hybrid_left.extend(&[0u8]);
        // right = X || 1
        let mut hybrid_right: Vec<u8> = Vec::new();
        hybrid_right.extend(&hybrid_seed);
        hybrid_right.extend(&[1u8]);
        // PoS-seed_n = Blake2b(X || 0) || Blake2b(X || 1)
        let seed_left: H256 = blake2b(&hybrid_left);
        let seed_right: H256 = blake2b(&hybrid_right);
        let mut new_seed: Vec<u8> = Vec::new();
        new_seed.extend(&seed_left.to_vec());
        new_seed.extend(&seed_right.to_vec());
        debug!(target: "miner", "block {:?}, hybrid_left {:?}, hybrid_right {:?}, seed_left {:?}, 
            seed_right {:?}, new_seed {:?}", 
            parent_header.number() + 1, hybrid_left, hybrid_right, seed_left,
            seed_right, new_seed);
        let mut seed: [u8; 64] = [0u8; 64];
        seed.copy_from_slice(&new_seed.as_slice()[..64]);
        seed
    }

    // Unity-3
    // Generate ecvrf seed for pos block
    fn generate_ecvrf_seed(
        grand_parent_seed: &[u8],
        sk: &[u8; 64],
    ) -> Result<([u8; 80], [u8; 64]), ()>
    {
        let proof: [u8; 80] = ecvrf::prove(sk, grand_parent_seed)?;
        let new_seed: [u8; 64] = ecvrf::proof_to_hash(&proof)?;
        Ok((proof, new_seed))
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
        self.try_prepare_block_pow(client, true);
    }

    /// MinerStatus     -   pending transaction number
    ///                 -   future transaction number
    ///                 -   transaction number in pending block
    fn status(&self) -> MinerStatus {
        let status = self.transaction_pool.status();
        let sealing_work_pow = self.sealing_work_pow.lock();
        MinerStatus {
            transactions_in_pending_queue: status.pending,
            transactions_in_future_queue: status.future,
            transactions_in_pending_block: sealing_work_pow
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

    fn add_sealing_pos(
        &self,
        hash: &H256,
        b: ClosedBlock,
        s: [u8; 64],
        t: u64,
        _client: &MiningBlockChainClient,
    ) -> Result<(), Error>
    {
        self.maybe_work.lock().insert(
            *hash,
            ReadyPoSWork {
                block: b,
                ready_time: t,
            },
        );
        self.sealing_work_pos.lock().insert(Seed(s), *hash);
        Ok(())
    }

    fn get_ready_pos(&self, hash: &H256) -> Option<(ClosedBlock, Vec<Bytes>)> {
        match self.maybe_work.lock().get(hash) {
            Some(b) => {
                let seal = b.header().seal();
                Some((b.block.clone(), seal.clone().to_vec()))
            }
            _ => None,
        }
    }

    /// Generate PoS block template
    fn get_pos_template(
        &self,
        client: &MiningBlockChainClient,
        seed_or_proof: Vec<u8>,
        pk: H256,
        coinbase: H256,
    ) -> Option<H256>
    {
        // AION 2.0
        if !self.new_block_allowed_with_seal_type(client, &SealType::PoS) {
            return None;
        }

        let stake = client
            .get_stake(&pk, coinbase, BlockId::Latest)
            .unwrap_or(BigUint::zero());
        if stake.is_zero() {
            return None;
        }

        let best_block_header = client.best_block_header();
        let timestamp = best_block_header.timestamp();
        let grand_parent = client.block_header_data(&best_block_header.parent_hash());
        let great_grand_parent = grand_parent.clone().map_or(None, |header| {
            client.block_header_data(&header.parent_hash())
        });
        let latest_seed: Vec<u8> = match self.latest_seed(client) {
            Ok(result) => result,
            Err(()) => return None,
        };

        let difficulty = client.calculate_difficulty(
            &best_block_header.decode(),
            grand_parent.map(|header| header.decode()).as_ref(),
            great_grand_parent.map(|header| header.decode()).as_ref(),
        );

        debug!(target: "miner", "new block difficulty = {:?}", difficulty);

        // Unity-3 ECVRF
        let new_seed: [u8; 64] = if self.unity_ecvrf_seed_update(client) {
            match slice_to_array_80(seed_or_proof.as_slice()) {
                Some(proof) => {
                    match ecvrf::proof_to_hash(proof) {
                        Ok(result) => result,
                        Err(_) => return None,
                    }
                }
                None => return None,
            }
        }
        // Unity-2 Hybrid seed
        else if self.unity_hybrid_seed_update(client) {
            if seed_or_proof.len() == 64 {
                Self::generate_hybrid_seed(&latest_seed, &pk.to_vec(), &best_block_header.decode())
            } else {
                return None;
            }
        }
        // Unity-1
        else {
            match slice_to_array_64(seed_or_proof.as_slice()) {
                Some(result) => *result,
                None => return None,
            }
        };

        let delta_uint = calculate_delta(difficulty, &new_seed, stake.clone());

        let new_timestamp = timestamp + delta_uint;

        if let Ok((raw_block, _)) = self.prepare_block(
            client,
            &Some(SealType::PoS),
            Some(new_timestamp),
            Some(coinbase),
            Some(&new_seed),
        ) {
            let mut seal = Vec::with_capacity(3);
            // Unity-3 ECVRF seed
            // Store proof instead of seed in the pos seal
            if self.unity_ecvrf_seed_update(client) {
                seal.push(seed_or_proof);
            } else {
                seal.push(new_seed.to_vec());
            }
            seal.push(vec![0u8; 64]);
            seal.push(pk.to_vec());

            let block = raw_block.pre_seal(seal);

            let hash = block.header().mine_hash();
            let timestamp_now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            match self.add_sealing_pos(&hash, block, new_seed, timestamp_now, client) {
                Ok(_) => Some(hash),
                _ => None,
            }
        } else {
            None
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
        // Check if the block is fresh (if the block's parent is the current best block)
        let best_block_hash = client.chain_info().best_block_hash;
        let parent_hash = block.header().parent_hash().clone();
        if parent_hash != best_block_hash {
            return Err(Error::Other(
                "The submited PoS block is outdated.".to_string(),
            ));
        }

        // Get information for PoS block sealing
        let parent = match client.block_header_data(&parent_hash) {
            Some(header) => header,
            None => {
                return Err(Error::from(BlockError::UnknownParent(parent_hash)));
            }
        };
        let grand_parent = client.block_header_data(&parent.parent_hash());
        let stake = client.get_stake(
            &seal[2][..].into(),
            block.header().author().clone(),
            BlockId::Latest,
        );

        debug!(target: "miner", "start sealing");

        // Try to seal block (validate seal)
        match block.lock().try_seal_pos(
            &*self.engine,
            seal,
            &parent.decode(),
            grand_parent.map(|header| header.decode()).as_ref(),
            stake,
        ) {
            // Sealing(validation) succeeded
            Ok(sealed_block) => {
                debug!(target: "miner", "sealing succeeded");
                let timestamp_now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let timestamp_block = sealed_block.header().timestamp();
                // Check if we can import the block now
                if timestamp_now >= timestamp_block {
                    client.import_sealed_block(sealed_block)?;
                } else {
                    // If it's not yet the time to import the block, compare it to the current best and save it if it's better
                    let mut best_pos = self.best_pos.lock();
                    let mut need_update = false;
                    match *best_pos {
                        Some(ref best_block) => {
                            if best_block.header().timestamp() >= timestamp_block {
                                need_update = true;
                            }
                        }
                        None => {
                            need_update = true;
                        }
                    }
                    if need_update {
                        *best_pos = Some(sealed_block);
                    }
                }
                return Ok(());
            }
            // Sealing(validation) failed
            Err((e, _)) => {
                debug!(target: "miner", "pos sealing validation error: {:?}", e);
                return Err(Error::from(e));
            }
        }
    }

    /// RPC
    fn is_currently_sealing(&self) -> bool { self.sealing_work_pow.lock().queue.is_in_use() }

    // Stratum server receives a finished job, and updates sealing work
    fn map_sealing_work<F, T>(&self, client: &MiningBlockChainClient, f: F) -> Option<T>
    where F: FnOnce(&ClosedBlock) -> T {
        // AION 2.0
        // Return if PoW mining is not allowed
        if !self.new_block_allowed_with_seal_type(client, &SealType::PoW) {
            trace!(target: "miner", "PoW mining not allowed because the current best is a PoW");
            return None;
        }
        // Get the latest PoW block
        trace!(target: "miner", "map_sealing_work: entering");
        self.prepare_work_sealing(client);
        trace!(target: "miner", "map_sealing_work: sealing prepared");
        let mut sealing_work_pow = self.sealing_work_pow.lock();
        let ret = sealing_work_pow.queue.use_last_ref();
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
        let result = if let Some(b) = self.sealing_work_pow.lock().queue.get_used_if(
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
            debug!(target: "miner", "Submitted solution rejected: Block unknown or out of date.");
            Err(Error::PowHashInvalid)
        };
        result.and_then(|sealed| {
            client.import_sealed_block(sealed)?;
            Ok(())
        })
    }

    /// Client
    fn chain_new_blocks(
        &self,
        client: &MiningBlockChainClient,
        _imported: &[H256],
        _invalid: &[H256],
        enacted: &[H256],
        retracted: &[H256],
    )
    {
        trace!(target: "block", "chain_new_blocks");

        if let Some(update_num) = self.engine.machine().params().unity_update {
            let best_num = client
                .block_number(BlockId::Latest)
                .expect("should not be none");
            if best_num >= update_num && !retracted.is_empty() {
                for tx in self
                    .pending_transactions()
                    .iter()
                    .chain(self.future_transactions().iter())
                {
                    if let Some(ref hash) = tx.beacon {
                        if client.is_beacon_hash(hash).is_none() {
                            self.transaction_pool.remove_transaction(
                                *tx.hash(),
                                RemovalReason::InvalidBeaconHash(*hash),
                            )
                        }
                    }
                }
            }
        }

        // Import all transactions in retracted routes...
        {
            for hash in retracted {
                let block = client.block(BlockId::Hash(*hash)).expect(
                    "Client is sending message after commit to db and inserting to chain; the \
                     block is available; qed",
                );
                let transactions = block.transactions();
                transactions.into_iter().for_each(|unverified_transaction| {
                    let _r = self
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

        // Actions to do when new block imported in the main chain
        if !enacted.is_empty() {
            self.transaction_pool.record_transaction_sealed();
            self.clear_pending_blocks();
            client.new_block_chained();
        }
    }

    // AION 2.0
    // Check if next block is on the unity hard fork
    fn unity_update(&self, client: &MiningBlockChainClient) -> bool {
        self.engine
            .machine()
            .params()
            .unity_update
            .map_or(false, |fork_number| {
                client.chain_info().best_block_number >= fork_number
            })
    }

    // AION Unity hybrid seed update
    // Check if the next block is on the unity hybrid seed hard fork
    fn unity_hybrid_seed_update(&self, client: &MiningBlockChainClient) -> bool {
        self.engine
            .machine()
            .params()
            .unity_hybrid_seed_update
            .map_or(false, |fork_number| {
                client.chain_info().best_block_number >= fork_number
            })
    }

    // AION Unity exvrf seed update
    // Check if the next block is on the unity ecvrf seed hard fork
    fn unity_ecvrf_seed_update(&self, client: &MiningBlockChainClient) -> bool {
        self.engine
            .machine()
            .params()
            .unity_ecvrf_seed_update
            .map_or(false, |fork_number| {
                client.chain_info().best_block_number + 1 >= fork_number
            })
    }

    // AION 2.0
    // Check if it's allowed to produce a new block with given seal type.
    // A block's seal type must be different than its parent's seal type.
    fn new_block_allowed_with_seal_type(
        &self,
        client: &MiningBlockChainClient,
        seal_type: &SealType,
    ) -> bool
    {
        // This rule only applies after unity fork
        if !self.unity_update(client) {
            if seal_type == &SealType::PoS {
                false
            } else {
                true
            }
        } else {
            seal_type != &client.best_block_header().seal_type().unwrap_or_default()
        }
    }

    // Unity
    /// Get the latest seed from the last pos block
    fn latest_seed(&self, client: &MiningBlockChainClient) -> Result<Vec<u8>, ()> {
        if self.unity_update(client) {
            let best_block = client.best_block_header();
            let grand_parent = client.block_header_data(&best_block.parent_hash());
            let latest_seed: Vec<u8> = match grand_parent {
                Some(ref header) if header.seal_type() == Some(SealType::PoS) => {
                    // header.seal()[0] is guaranteed since it's already in chain
                    header.seal()[0].clone()
                }
                _ => vec![0; 64], // Empty previous seed for the first pos block
            };

            // ecvrf seed
            if self.unity_ecvrf_seed_update(client) {
                if let Some(latest_proof) = slice_to_array_80(&latest_seed) {
                    return Ok(ecvrf::proof_to_hash(latest_proof)?.to_vec());
                }
            }

            Ok(latest_seed)
        } else {
            Err(())
        }
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
    use super::{Banning, MinerOptions, PendingSet, SealType};
    use types::error::*;
    use client::{BlockChainClient, BlockId};
    use tests::common::{EachBlockWith, TestBlockChainClient};
    use transaction::{PendingTransaction, SignedTransaction, Error as TransactionError};
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
        client.add_blocks(1, EachBlockWith::Nothing, SealType::PoW);
        miner.map_sealing_work(&client, |b| b.block().header().mine_hash());

        client.add_blocks(1, EachBlockWith::Nothing, SealType::PoW);
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
                work_queue_size: 50,
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

    fn miner_with_spec(spec: &Spec) -> Miner {
        Arc::try_unwrap(Miner::new(
            MinerOptions {
                force_sealing: false,
                reseal_min_period: Duration::from_secs(1),
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
            spec,
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
            beacon: None,
        }
        .sign(keypair.secret())
    }

    fn transaction_with_beacon(beacon: H256) -> SignedTransaction {
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
            beacon: Some(beacon),
        }
        .sign(keypair.secret())
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
        miner.prepare_work_sealing(&client);
        assert_eq!(miner.pending_transactions().len(), 1);
        assert_eq!(miner.ready_transactions(best_block, 0).len(), 1);
        assert_eq!(miner.pending_transactions_hashes(best_block).len(), 1);
        assert_eq!(miner.pending_receipts(best_block).len(), 1);
        // This method will let us know if pending block was created (before calling that method)
        assert!(!miner.prepare_work_sealing(&client));
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
        miner.prepare_work_sealing(&client);
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
        // miner.prepare_work_sealing(&client,&None);
        assert_eq!(miner.pending_transactions().len(), 1);
        assert_eq!(miner.pending_transactions_hashes(best_block).len(), 0);
        assert_eq!(miner.ready_transactions(best_block, 0).len(), 0);
        assert_eq!(miner.pending_receipts(best_block).len(), 0);
        // This method will let us know if pending block was created (before calling that method)
        assert!(miner.prepare_work_sealing(&client));
    }

    #[test]
    fn should_not_import_transaction_with_beacon_hash_before_unity_update() {
        // given
        let client = TestBlockChainClient::new_with_spec(Spec::new_unity(None));
        let miner = miner_with_spec(&Spec::new_unity(None));
        let transaction = transaction_with_beacon(H256::from(37472u64));
        let best_block = 0;
        // external
        let res = miner
            .import_external_transactions(&client, vec![transaction.clone().into()])
            .pop()
            .unwrap();
        assert_eq!(
            format!("{}", res.err().unwrap()),
            format!("{}", Error::Transaction(TransactionError::BeaconBanned))
        );
        // internal
        let res = miner
            .import_own_transaction(&client, PendingTransaction::new(transaction.clone(), None));
        assert_eq!(
            format!("{}", res.err().unwrap()),
            format!("{}", Error::Transaction(TransactionError::BeaconBanned))
        );
        // then
        miner.update_transaction_pool(&client, true);
        // miner.prepare_work_sealing(&client);
        assert_eq!(miner.pending_transactions().len(), 0);
        assert_eq!(miner.pending_transactions_hashes(best_block).len(), 0);
        assert_eq!(miner.ready_transactions(best_block, 0).len(), 0);
        assert_eq!(miner.pending_receipts(best_block).len(), 0);
        assert!(miner.prepare_work_sealing(&client));

        client.add_blocks(9, EachBlockWith::Nothing, SealType::PoW);

        let res = miner
            .import_external_transactions(&client, vec![transaction.clone().into()])
            .pop()
            .unwrap();
        assert_eq!(
            format!("{}", res.err().unwrap()),
            format!(
                "{}",
                Error::Transaction(TransactionError::InvalidBeaconHash(H256::from(37472u64)))
            )
        );
        // internal
        let res = miner
            .import_own_transaction(&client, PendingTransaction::new(transaction.clone(), None));
        assert_eq!(
            format!("{}", res.err().unwrap()),
            format!(
                "{}",
                Error::Transaction(TransactionError::InvalidBeaconHash(H256::from(37472u64)))
            )
        );
    }

    #[test]
    fn should_not_import_transaction_with_invalid_beacon_hash_after_unity_update() {
        // given
        let client = TestBlockChainClient::new_with_spec(Spec::new_unity(None));
        let miner = miner_with_spec(&Spec::new_unity(None));
        let transaction = transaction_with_beacon(H256::from(37472u64));
        client.add_blocks(9, EachBlockWith::Nothing, SealType::PoW);
        let best_block = 9;

        let res = miner
            .import_external_transactions(&client, vec![transaction.clone().into()])
            .pop()
            .unwrap();
        assert_eq!(
            format!("{}", res.err().unwrap()),
            format!(
                "{}",
                Error::Transaction(TransactionError::InvalidBeaconHash(H256::from(37472u64)))
            )
        );
        // internal
        let res = miner
            .import_own_transaction(&client, PendingTransaction::new(transaction.clone(), None));
        assert_eq!(
            format!("{}", res.err().unwrap()),
            format!(
                "{}",
                Error::Transaction(TransactionError::InvalidBeaconHash(H256::from(37472u64)))
            )
        );

        // then
        miner.update_transaction_pool(&client, true);
        // miner.prepare_work_sealing(&client);
        assert_eq!(miner.pending_transactions().len(), 0);
        assert_eq!(miner.pending_transactions_hashes(best_block).len(), 0);
        assert_eq!(miner.ready_transactions(best_block, 0).len(), 0);
        assert_eq!(miner.pending_receipts(best_block).len(), 0);
    }

    #[test]
    fn should_import_transaction_with_valid_beacon_hash_after_unity_update() {
        // given
        let client = TestBlockChainClient::new_with_spec(Spec::new_unity(None));
        let miner = miner_with_spec(&Spec::new_unity(None));
        client.add_blocks(9, EachBlockWith::Nothing, SealType::PoW);
        let best_block = 9;
        let beacon = client.block(BlockId::Number(8)).unwrap();

        let transaction = transaction_with_beacon(beacon.hash());

        let res = miner
            .import_external_transactions(&client, vec![transaction.clone().into()])
            .pop()
            .unwrap();
        assert!(res.is_ok());

        // internal
        let res = miner
            .import_own_transaction(&client, PendingTransaction::new(transaction.clone(), None));
        assert!(res.is_ok());

        // then
        miner.update_transaction_pool(&client, true);

        let res = miner
            .import_external_transactions(&client, vec![transaction.clone().into()])
            .pop()
            .unwrap();
        assert_eq!(
            format!("{}", res.err().unwrap()),
            format!("{}", Error::Transaction(TransactionError::AlreadyImported))
        );

        // internal
        let res = miner
            .import_own_transaction(&client, PendingTransaction::new(transaction.clone(), None));
        assert_eq!(
            format!("{}", res.err().unwrap()),
            format!("{}", Error::Transaction(TransactionError::AlreadyImported))
        );

        //        miner.prepare_work_sealing(&client,&None);
        assert_eq!(miner.pending_transactions().len(), 1);
        assert_eq!(miner.pending_transactions_hashes(best_block).len(), 0);
        assert_eq!(miner.ready_transactions(best_block, 0).len(), 0);
        assert_eq!(miner.pending_receipts(best_block).len(), 0);
    }

    #[test]
    fn test_tx_pool_with_beacon_when_chain_reorg() {
        // given
        let client = TestBlockChainClient::new_with_spec(Spec::new_unity(None));
        let miner = miner_with_spec(&Spec::new_unity(None));
        client.add_blocks(9, EachBlockWith::Nothing, SealType::PoW);
        let best_block = 9;
        let hash7 = client.block(BlockId::Number(7)).unwrap().hash();
        let hash8 = client.block(BlockId::Number(8)).unwrap().hash();
        client.add_blocks(1, EachBlockWith::Transaction(Some(hash8)), SealType::PoS);
        let hash10 = client.chain_info().best_block_hash;
        //        let block10 = client.block(BlockId::Hash(hash10)).unwrap();
        //        block10.transaction_views()[]
        client.add_blocks(1, EachBlockWith::Transaction(Some(hash10)), SealType::PoW);
        let hash11 = client.chain_info().best_block_hash;
        let hash9 = client.block(BlockId::Number(9)).unwrap().hash();

        let best = client.best_block_header();
        assert_eq!(best.number(), 11);
        assert_eq!(best.parent_hash(), hash10);

        let (hash10b, block10b) = client.generate_block_rlp(
            EachBlockWith::Nothing,
            12u64.into(),
            hash9,
            10,
            SealType::PoS,
        );
        let r10b = client.import_block(block10b.as_raw().to_vec());
        let r10 = client.numbers.write().insert(10, hash10b.clone());
        assert_eq!(r10.unwrap(), hash10);
        assert!(r10b.is_ok());
        let (hash11b, block11b) = client.generate_block_rlp(
            EachBlockWith::Nothing,
            13u64.into(),
            hash10b,
            11,
            SealType::PoW,
        );
        let r11b = client.import_block(block11b.as_raw().to_vec());
        let r11 = client.numbers.write().insert(11, hash11b.clone());
        assert_eq!(r11.unwrap(), hash11);
        assert!(r11b.is_ok());
        assert_eq!(client.chain_info().best_block_hash, hash11);
        assert!(client.block(BlockId::Hash(hash10)).is_some());

        client.nonces.write().clear();

        miner.chain_new_blocks(&client, &[], &[], &[hash10b, hash11b], &[hash10, hash11]);
        miner.update_transaction_pool(&client, true);
        assert_eq!(miner.pending_transactions().len(), 1);
        assert_eq!(miner.pending_transactions_hashes(best_block).len(), 0);
        assert_eq!(miner.ready_transactions(best_block, 0).len(), 0);
        assert_eq!(miner.pending_receipts(best_block).len(), 0);

        let transaction_b7 = transaction_with_beacon(hash7);
        let transaction_b11 = transaction_with_beacon(hash11b);

        let res = miner
            .import_external_transactions(&client, vec![transaction_b7.clone().into()])
            .pop()
            .unwrap();
        assert!(res.is_ok());

        // internal
        let res = miner.import_own_transaction(
            &client,
            PendingTransaction::new(transaction_b11.clone(), None),
        );
        assert!(res.is_ok());

        // then
        miner.update_transaction_pool(&client, true);
        assert_eq!(miner.pending_transactions().len(), 3);
        assert_eq!(miner.pending_transactions_hashes(best_block).len(), 0);
        assert_eq!(miner.ready_transactions(best_block, 0).len(), 0);
        assert_eq!(miner.pending_receipts(best_block).len(), 0);

        let (hash11c, block11c) = client.generate_block_rlp(
            EachBlockWith::Nothing,
            14u64.into(),
            hash10b,
            11,
            SealType::PoW,
        );
        let r11c = client.import_block(block11c.as_raw().to_vec());
        let r11 = client.numbers.write().insert(11, hash11c.clone());
        assert_eq!(r11.unwrap(), hash11b);
        assert_eq!(client.chain_info().best_block_hash, hash11);
        assert!(r11c.is_ok());

        miner.chain_new_blocks(&client, &[], &[], &[hash11c], &[hash11b]);
        miner.update_transaction_pool(&client, true);
        assert_eq!(miner.pending_transactions().len(), 2);
        assert_eq!(miner.pending_transactions_hashes(best_block).len(), 0);
        assert_eq!(miner.ready_transactions(best_block, 0).len(), 0);
        assert_eq!(miner.pending_receipts(best_block).len(), 0);
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
        assert!(miner.prepare_work_sealing(&client));
        // Unless asked to prepare work.
        assert!(miner.requires_reseal(1u8.into()));
    }

    use aion_types::{H256, H512};
    #[test]
    fn generate_pos_block() {
        // 1. get seed
        let spec = Spec::new_unity(Some(0));
        let miner = miner_with_spec(&spec);
        let client = TestBlockChainClient::new_with_spec(spec);

        let seed = H512::zero();
        println!("seed = {:?}", seed);

        let staker: H256 = "da13c5e00eefa13b58292b9083c04559b77c5859bc764b47e2aa5ecfe9ea3bab"
            .from_hex()
            .unwrap()
            .as_slice()
            .into();

        // 2. submit seed
        let seed: H512 = "d1c02f4679b4a022f2d843bd750c34c94cd08a2b6fc2def298653b81b88245a345d8d3e2d8bbce3fdb3ab2918459633f4496d5609ac13d9710ddcede8957cc0c".
            from_hex().unwrap().as_slice().into();
        let template = miner.get_pos_template(&client, seed.to_vec(), staker, staker);

        assert!(template.is_some());
        println!("new block = {:?}, staker = {:?}", template.unwrap(), staker);

        // 3. submit signature
        let signature: H512 = "8e6151cd613c07fae0aaf521461111c8281c0715dc6bdc5620682efd52540c4040d48dc067b88b1bc225294f4a7412c46f49bdcb9271b6b18022149663eb3108"
            .from_hex().unwrap().as_slice().into();
        let (block, mut seal) = miner.get_ready_pos(&template.unwrap()).unwrap();
        seal[1] = signature[..].into();
        let result = miner.try_seal_pos(&client, seal, block);
        assert!(result.is_ok());
    }

    #[test]
    fn pos_reseal() {
        let spec = Spec::new_unity(Some(0));
        let miner = miner_with_spec(&spec);
        let client = TestBlockChainClient::new_with_spec(spec);

        let seed = H512::zero();
        println!("seed = {:?}", seed);

        let staker: H256 = "da13c5e00eefa13b58292b9083c04559b77c5859bc764b47e2aa5ecfe9ea3bab"
            .from_hex()
            .unwrap()
            .as_slice()
            .into();

        // 2. submit seed
        let seed: H512 = "d1c02f4679b4a022f2d843bd750c34c94cd08a2b6fc2def298653b81b88245a345d8d3e2d8bbce3fdb3ab2918459633f4496d5609ac13d9710ddcede8957cc0c".
            from_hex().unwrap().as_slice().into();
        let template = miner.get_pos_template(&client, seed.to_vec(), staker, staker);

        assert!(template.is_some());
        println!("new block = {:?}, staker = {:?}", template.unwrap(), staker);
        ::std::thread::sleep(Duration::from_secs(1));
        let template_1 = miner.get_pos_template(&client, seed.to_vec(), staker, staker);
        assert_eq!(template, template_1);

        // 3. submit signature
        let signature: H512 = "8e6151cd613c07fae0aaf521461111c8281c0715dc6bdc5620682efd52540c4040d48dc067b88b1bc225294f4a7412c46f49bdcb9271b6b18022149663eb3108"
            .from_hex().unwrap().as_slice().into();
        let (block, mut seal) = miner.get_ready_pos(&template.unwrap()).unwrap();
        seal[1] = signature[..].into();
        let result = miner.try_seal_pos(&client, seal, block);
        assert!(result.is_ok());
    }
}
