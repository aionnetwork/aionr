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

use super::super::transaction::UnverifiedTransaction;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Weak};
use std::time::Duration;
use time::precise_time_ns;

// util
use blake2b::blake2b;
use bytes::Bytes;
use journaldb;
use kvdb::{DBTransaction, DBValue, KeyValueDB};
use trie::{Trie, TrieFactory, TrieSpec};

// other
use aion_types::{Address, H128, H256, H264, U256};
use block::*;
use blockchain::{BlockChain, BlockProvider, ImportRoute, TransactionAddress, TreeRoute};
use client::Error as ClientError;
use client::{
    BlockChainClient, BlockId, BlockImportError, CallAnalytics, ChainNotify, ClientConfig,
    MiningBlockChainClient, ProvingBlockChainClient, PruningInfo, TransactionId,
};
use encoded;
use engines::{EpochTransition, POWEquihashEngine};
use error::{BlockError, CallError, ExecutionError, ImportError, ImportResult};
use executive::{contract_address, Executed, Executive};
use factory::{Factories, VmFactory};
use header::{BlockNumber, Header, Seal};
use io::*;
use log_entry::LocalizedLogEntry;
use miner::{Miner, MinerService};
use parking_lot::{Mutex, RwLock};
use receipt::{LocalizedReceipt, Receipt};
use rlp::*;
use service::ClientIoMessage;
use spec::Spec;
use state::{self, State};
use state_db::StateDB;
use transaction::{
    Action, LocalizedTransaction, PendingTransaction, SignedTransaction, Transaction,
    DEFAULT_TRANSACTION_TYPE, AVM_TRANSACTION_TYPE
};
use types::filter::Filter;
use types::vms::{EnvInfo, LastHashes};
use verification::queue::BlockQueue;
use verification::{
    PreverifiedBlock,
    verify_block_family,
    verify_block_final
};
use views::BlockView;

// re-export
pub use blockchain::CacheSize as BlockChainCacheSize;
pub use types::block_status::BlockStatus;
pub use types::blockchain_info::BlockChainInfo;
pub use verification::queue::QueueInfo as BlockQueueInfo;

const MIN_HISTORY_SIZE: u64 = 8;

/// Report on the status of a client.
#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub struct ClientReport {
    /// How many blocks have been imported so far.
    pub blocks_imported: usize,
    /// How many transactions have been applied so far.
    pub transactions_applied: usize,
    /// How much gas has been processed so far.
    pub gas_processed: U256,
    /// Memory used by state DB
    pub state_db_mem: usize,
}

impl ClientReport {
    /// Alter internal reporting to reflect the additional `block` has been processed.
    pub fn accrue_block(&mut self, block: &PreverifiedBlock) {
        self.blocks_imported += 1;
        self.transactions_applied += block.transactions.len();
        self.gas_processed = self.gas_processed + block.header.gas_used().clone();
    }
}

impl<'a> ::std::ops::Sub<&'a ClientReport> for ClientReport {
    type Output = Self;

    fn sub(mut self, other: &'a ClientReport) -> Self {
        let higher_mem = ::std::cmp::max(self.state_db_mem, other.state_db_mem);
        let lower_mem = ::std::cmp::min(self.state_db_mem, other.state_db_mem);

        self.blocks_imported -= other.blocks_imported;
        self.transactions_applied -= other.transactions_applied;
        self.gas_processed = self.gas_processed - other.gas_processed;
        self.state_db_mem = higher_mem - lower_mem;

        self
    }
}

/// Blockchain database client backed by a persistent database. Owns and manages a blockchain and a block queue.
/// Call `import_block()` to import a block asynchronously; `flush_queue()` flushes the queue.
pub struct Client {
    enabled: AtomicBool,
    chain: RwLock<Arc<BlockChain>>,
    engine: Arc<POWEquihashEngine>,
    config: ClientConfig,
    db: RwLock<Arc<KeyValueDB>>,
    state_db: RwLock<StateDB>,
    block_queue: BlockQueue,
    report: RwLock<ClientReport>,
    import_lock: Mutex<()>,
    //verifier: Box<Verifier>,
    miner: Arc<Miner>,
    io_channel: Mutex<IoChannel<ClientIoMessage>>,
    notify: RwLock<Vec<Weak<ChainNotify>>>,
    last_hashes: RwLock<VecDeque<H256>>,
    factories: Factories,
    history: u64,
}

impl Client {
    /// Create a new client with given parameters.
    /// The database is assumed to have been initialized with the correct columns.
    pub fn new(
        config: ClientConfig,
        spec: &Spec,
        db: Arc<KeyValueDB>,
        miner: Arc<Miner>,
        message_channel: IoChannel<ClientIoMessage>,
    ) -> Result<Arc<Client>, ::error::Error>
    {
        let trie_spec = match config.fat_db {
            true => TrieSpec::Fat,
            false => TrieSpec::Secure,
        };

        let trie_factory = TrieFactory::new(trie_spec);
        let factories = Factories {
            vm: VmFactory::new(),
            trie: trie_factory,
            accountdb: Default::default(),
        };

        let journal_db = journaldb::new(db.clone(), config.pruning, ::db::COL_STATE);
        let mut state_db = StateDB::new(journal_db, config.state_cache_size);
        if state_db.journal_db().is_empty() {
            // Sets the correct state root.
            state_db = spec.ensure_db_good(state_db, &factories)?;
            let mut batch = DBTransaction::new();
            state_db.journal_under(&mut batch, 0, &spec.genesis_header().hash())?;
            db.write(batch).map_err(ClientError::Database)?;
        }

        let gb = spec.genesis_block();
        let chain = Arc::new(BlockChain::new(config.blockchain.clone(), &gb, db.clone()));

        trace!(
            target: "client",
            "Cleanup journal: DB Earliest = {:?}, Latest = {:?}",
            state_db.journal_db().earliest_era(),
            state_db.journal_db().latest_era()
        );

        let history = if config.history < MIN_HISTORY_SIZE {
            info!(target: "client", "Ignoring pruning history parameter of {}\
                , falling back to minimum of {}",
                config.history, MIN_HISTORY_SIZE);
            MIN_HISTORY_SIZE
        } else {
            config.history
        };

        if !chain
            .block_header(&chain.best_block_hash())
            .map_or(true, |h| state_db.journal_db().contains(h.state_root()))
        {
            warn!(
                target: "client",
                "State root not found for block #{} ({:x})",
                chain.best_block_number(),
                chain.best_block_hash()
            );
        }

        let engine = spec.engine.clone();

        let block_queue = BlockQueue::new(
            config.queue.clone(),
            engine.clone(),
            message_channel.clone(),
        );

        let client = Arc::new(Client {
            enabled: AtomicBool::new(true),
            chain: RwLock::new(chain),
            engine,
            config,
            db: RwLock::new(db),
            state_db: RwLock::new(state_db),
            block_queue,
            report: RwLock::new(Default::default()),
            import_lock: Mutex::new(()),
            miner,
            io_channel: Mutex::new(message_channel),
            notify: RwLock::new(Vec::new()),
            last_hashes: RwLock::new(VecDeque::new()),
            factories,
            history,
        });

        // prune old states.
        {
            let state_db = client.state_db.read().boxed_clone();
            let chain = client.chain.read();
            client.prune_ancient(state_db, &chain)?;
        }

        // ensure genesis epoch proof in the DB.
        {
            let chain = client.chain.read();
            let gh = spec.genesis_header();
            if chain.epoch_transition(0, gh.hash()).is_none() {
                trace!(target: "client", "No genesis transition found.");

                let proof = Vec::new();
                let mut batch = DBTransaction::new();
                chain.insert_epoch_transition(
                    &mut batch,
                    0,
                    EpochTransition {
                        block_hash: gh.hash(),
                        block_number: 0,
                        proof,
                    },
                );

                client.db.read().write_buffered(batch);
            }
        }

        // ensure buffered changes are flushed.
        client.db.read().flush().map_err(ClientError::Database)?;
        Ok(client)
    }

    /// Adds an actor to be notified on certain events
    pub fn add_notify(&self, target: Arc<ChainNotify>) {
        self.notify.write().push(Arc::downgrade(&target));
    }

    /// Returns engine reference.
    pub fn engine(&self) -> &POWEquihashEngine { &*self.engine }

    fn notify<F>(&self, f: F)
    where F: Fn(&ChainNotify) {
        for np in self.notify.read().iter() {
            if let Some(n) = np.upgrade() {
                f(&*n);
            }
        }
    }
    /// Flush the block import queue.
    pub fn flush_queue(&self) {
        self.block_queue.flush();
        while !self.block_queue.queue_info().is_empty() {
            self.import_verified_blocks();
        }
    }

    /// The env info as of the best block.
    pub fn latest_env_info(&self) -> EnvInfo {
        self.env_info(BlockId::Latest)
            .expect("Best block header always stored; qed")
    }

    /// The env info as of a given block.
    /// returns `None` if the block unknown.
    pub fn env_info(&self, id: BlockId) -> Option<EnvInfo> {
        self.block_header(id).map(|header| {
            EnvInfo {
                number: header.number(),
                author: header.author(),
                timestamp: header.timestamp(),
                difficulty: header.difficulty(),
                last_hashes: self.build_last_hashes(header.parent_hash()),
                gas_used: U256::default(),
                gas_limit: header.gas_limit(),
            }
        })
    }

    fn build_last_hashes(&self, parent_hash: H256) -> Arc<LastHashes> {
        {
            let hashes = self.last_hashes.read();
            if hashes.front().map_or(false, |h| h == &parent_hash) {
                let mut res = Vec::from(hashes.clone());
                res.resize(256, H256::default());
                return Arc::new(res);
            }
        }
        let mut last_hashes = LastHashes::new();
        last_hashes.resize(256, H256::default());
        last_hashes[0] = parent_hash;
        let chain = self.chain.read();
        for i in 0..255 {
            match chain.block_details(&last_hashes[i]) {
                Some(details) => {
                    last_hashes[i + 1] = details.parent.clone();
                }
                None => break,
            }
        }
        let mut cached_hashes = self.last_hashes.write();
        *cached_hashes = VecDeque::from(last_hashes.clone());
        Arc::new(last_hashes)
    }

    fn check_and_close_block(&self, block: &PreverifiedBlock) -> Result<LockedBlock, ()> {
        let engine = &*self.engine;
        let header = &block.header;

        let chain = self.chain.read();
        // Check the block isn't so old we won't be able to enact it.
        let best_block_number = chain.best_block_number();
        if self.pruning_info().earliest_state > header.number() {
            warn!(target: "client", "Block import failed for #{} ({})\nBlock is ancient (current best block: #{}).", header.number(), header.hash(), best_block_number);
            return Err(());
        }

        // Check if parent is in chain
        let parent_hash = header.parent_hash().clone();

        let parent = match chain.block_header(&parent_hash) {
            Some(h) => h,
            None => {
                warn!(target: "client", "Block import failed for #{} ({}): Parent not found ({}) ", header.number(), header.hash(), header.parent_hash());
                return Err(());
            }
        };

        let grant_parent = chain.block_header(parent.parent_hash());
        let grant_parent_header = grant_parent.as_ref();

        // Verify Block Family
        let verify_family_result = verify_block_family(
            //let verify_family_result = self.verifier.verify_block_family(
            header,
            &parent,
            grant_parent_header,
            engine,
            Some((&block.bytes, &block.transactions, &**chain, self)),
        );

        if let Err(e) = verify_family_result {
            warn!(target: "client", "Stage 3 block verification failed for #{} ({})\nError: {:?}", header.number(), header.hash(), e);
            return Err(());
        };

        // Enact Verified Block
        let last_hashes = self.build_last_hashes(header.parent_hash().clone());
        let db = self.state_db.read().boxed_clone_canon(&parent_hash);

        let enact_result = enact_verified(
            block,
            engine,
            db,
            &parent,
            grant_parent_header,
            last_hashes,
            self.factories.clone(),
            self.db.read().clone(),
        );
        let locked_block = enact_result.map_err(|e| {
            warn!(target: "client", "Block import failed for #{} ({})\nError: {:?}", header.number(), header.hash(), e);
        })?;

        // Final Verification
        if let Err(e) = verify_block_final(header, locked_block.block().header()) {
            warn!(target: "client", "Stage 5 block verification failed for #{} ({})\nError: {:?}", header.number(), header.hash(), e);
            return Err(());
        }

        Ok(locked_block)
    }

    fn calculate_enacted_retracted(
        &self,
        import_results: &[ImportRoute],
    ) -> (Vec<H256>, Vec<H256>)
    {
        fn map_to_vec(map: Vec<(H256, bool)>) -> Vec<H256> {
            map.into_iter().map(|(k, _v)| k).collect()
        }

        // In ImportRoute we get all the blocks that have been enacted and retracted by single insert.
        // Because we are doing multiple inserts some of the blocks that were enacted in import `k`
        // could be retracted in import `k+1`. This is why to understand if after all inserts
        // the block is enacted or retracted we iterate over all routes and at the end final state
        // will be in the hashmap
        let map = import_results
            .iter()
            .fold(HashMap::new(), |mut map, route| {
                for hash in &route.enacted {
                    map.insert(hash.clone(), true);
                }
                for hash in &route.retracted {
                    map.insert(hash.clone(), false);
                }
                map
            });

        // Split to enacted retracted (using hashmap value)
        let (enacted, retracted) = map.into_iter().partition(|&(_k, v)| v);
        // And convert tuples to keys
        (map_to_vec(enacted), map_to_vec(retracted))
    }

    /// This is triggered by a message coming from a block queue when the block is ready for insertion
    pub fn import_verified_blocks(&self) -> usize {
        // Shortcut out if we know we're incapable of syncing the chain.
        if !self.enabled.load(AtomicOrdering::Relaxed) {
            return 0;
        }

        let max_blocks_to_import = 4;
        let (
            imported_blocks,
            import_results,
            invalid_blocks,
            imported,
            proposed_blocks,
            duration,
            is_empty,
        ) = {
            let mut imported_blocks = Vec::with_capacity(max_blocks_to_import);
            let mut invalid_blocks = HashSet::new();
            let mut proposed_blocks = Vec::with_capacity(max_blocks_to_import);
            let mut import_results = Vec::with_capacity(max_blocks_to_import);

            let _import_lock = self.import_lock.lock();
            let blocks = self.block_queue.drain(max_blocks_to_import);
            if blocks.is_empty() {
                return 0;
            }
            trace_time!("import_verified_blocks");
            let start = precise_time_ns();

            for block in blocks {
                let header = &block.header;
                let is_invalid = invalid_blocks.contains(header.parent_hash());
                if is_invalid {
                    invalid_blocks.insert(header.hash());
                    continue;
                }
                if let Ok(closed_block) = self.check_and_close_block(&block) {
                    imported_blocks.push(header.hash());
                    trace!(target: "block", "commit_block() number: {:?}", header.number());
                    trace!(target: "block", "commit_block() header: {:?}", header.hash());
                    let route = self.commit_block(closed_block, &header, &block.bytes);
                    import_results.push(route);
                    self.report.write().accrue_block(&block);
                } else {
                    invalid_blocks.insert(header.hash());
                }
            }

            let imported = imported_blocks.len();
            let invalid_blocks = invalid_blocks.into_iter().collect::<Vec<H256>>();

            if !invalid_blocks.is_empty() {
                self.block_queue.mark_as_bad(&invalid_blocks);
            }
            let is_empty = self.block_queue.mark_as_good(&imported_blocks);
            let duration_ns = precise_time_ns() - start;
            (
                imported_blocks,
                import_results,
                invalid_blocks,
                imported,
                proposed_blocks,
                duration_ns,
                is_empty,
            )
        };

        {
            if !imported_blocks.is_empty() && is_empty {
                let (enacted, retracted) = self.calculate_enacted_retracted(&import_results);
                if is_empty {
                    self.miner.chain_new_blocks(
                        self,
                        &imported_blocks,
                        &invalid_blocks,
                        &enacted,
                        &retracted,
                    );
                }
                self.notify(|notify| {
                    notify.new_blocks(
                        imported_blocks.clone(),
                        invalid_blocks.clone(),
                        enacted.clone(),
                        retracted.clone(),
                        Vec::new(),
                        proposed_blocks.clone(),
                        duration,
                    );
                });
            }
        }

        self.db.read().flush().expect("DB flush failed.");
        imported
    }

    // NOTE: the header of the block passed here is not necessarily sealed, as
    // it is for reconstructing the state transition.
    //
    // The header passed is from the original block data and is sealed.
    fn commit_block<B>(&self, block: B, header: &Header, block_data: &[u8]) -> ImportRoute
    where B: IsBlock + Drain {
        let hash = &header.hash();
        let number = header.number();
        let parent = header.parent_hash();
        let chain = self.chain.read();

        // Commit results
        let receipts = block.receipts().to_owned();

        assert_eq!(
            header.hash(),
            BlockView::new(block_data).header_view().hash()
        );

        let mut batch = DBTransaction::new();

        let mut state = block.drain();

        state
            .journal_under(&mut batch, number, hash)
            .expect("DB commit failed");
        trace!(target: "block", "insert block number: {:?}", number);
        let route = chain.insert_block(&mut batch, block_data, receipts.clone());

        let is_canon = route.enacted.last().map_or(false, |h| h == hash);
        state.sync_cache(&route.enacted, &route.retracted, is_canon);
        // Final commit to the DB
        self.db.read().write_buffered(batch);
        chain.commit();

        self.update_last_hashes(&parent, hash);

        if let Err(e) = self.prune_ancient(state, &chain) {
            warn!(target:"client", "Failed to prune ancient state data: {}", e);
        }

        route
    }

    // prune ancient states until below the memory limit or only the minimum amount remain.
    fn prune_ancient(&self, mut state_db: StateDB, chain: &BlockChain) -> Result<(), ClientError> {
        let number = match state_db.journal_db().latest_era() {
            Some(n) => n,
            None => return Ok(()),
        };

        // prune all ancient eras until we're below the memory target,
        // but have at least the minimum number of states.
        loop {
            let needs_pruning = state_db.journal_db().is_pruned()
                && state_db.journal_db().journal_size() >= self.config.history_mem;

            if !needs_pruning {
                break;
            }
            match state_db.journal_db().earliest_era() {
                Some(era) if era + self.history <= number => {
                    trace!(target: "client", "Pruning state for ancient era {}", era);
                    match chain.block_hash(era) {
                        Some(ancient_hash) => {
                            let mut batch = DBTransaction::new();
                            state_db.mark_canonical(&mut batch, era, &ancient_hash)?;
                            self.db.read().write_buffered(batch);
                            state_db.journal_db().flush();
                        }
                        None => debug!(target: "client", "Missing expected hash for block {}", era),
                    }
                }
                _ => break, // means that every era is kept, no pruning necessary.
            }
        }

        Ok(())
    }

    fn update_last_hashes(&self, parent: &H256, hash: &H256) {
        let mut hashes = self.last_hashes.write();
        if hashes.front().map_or(false, |h| h == parent) {
            if hashes.len() > 255 {
                hashes.pop_back();
            }
            hashes.push_front(hash.clone());
        }
    }

    /// Get shared miner reference.
    pub fn miner(&self) -> Arc<Miner> { self.miner.clone() }

    /// Replace io channel. Useful for testing.
    pub fn set_io_channel(&self, io_channel: IoChannel<ClientIoMessage>) {
        *self.io_channel.lock() = io_channel;
    }

    /// Attempt to get a copy of a specific block's final state.
    ///
    /// This will not fail if given BlockId::Latest.
    /// Otherwise, this can fail (but may not) if the DB prunes state or the block
    /// is unknown.
    pub fn state_at(&self, id: BlockId) -> Option<State<StateDB>> {
        // fast path for latest state.
        match id.clone() {
            BlockId::Pending => {
                return self
                    .miner
                    .pending_state(self.chain.read().best_block_number())
                    .or_else(|| Some(self.state()))
            }
            BlockId::Latest => return Some(self.state()),
            _ => {}
        }

        let block_number = match self.block_number(id) {
            Some(num) => num,
            None => return None,
        };

        self.block_header(id).and_then(|header| {
            let db = self.state_db.read().boxed_clone();

            // early exit for pruned blocks
            if db.is_pruned() && self.pruning_info().earliest_state > block_number {
                return None;
            }

            let root = header.state_root();
            State::from_existing(
                db,
                root,
                self.engine.machine().account_start_nonce(block_number),
                self.factories.clone(),
                self.db.read().clone(),
            )
            .ok()
        })
    }

    /// Attempt to get a copy of a specific block's beginning state.
    ///
    /// This will not fail if given BlockId::Latest.
    /// Otherwise, this can fail (but may not) if the DB prunes state.
    pub fn state_at_beginning(&self, id: BlockId) -> Option<State<StateDB>> {
        // fast path for latest state.
        match id {
            BlockId::Pending => self.state_at(BlockId::Latest),
            id => {
                match self.block_number(id) {
                    None | Some(0) => None,
                    Some(n) => self.state_at(BlockId::Number(n - 1)),
                }
            }
        }
    }

    /// Get a copy of the best block's state.
    pub fn state(&self) -> State<StateDB> {
        let header = self.best_block_header();
        State::from_existing(
            self.state_db.read().boxed_clone_canon(&header.hash()),
            header.state_root(),
            self.engine.machine().account_start_nonce(header.number()),
            self.factories.clone(),
            self.db.read().clone(),
        )
        .expect("State root of best block header always valid.")
    }

    /// Get info on the cache.
    pub fn blockchain_cache_info(&self) -> BlockChainCacheSize { self.chain.read().cache_size() }

    /// Get the report.
    pub fn report(&self) -> ClientReport {
        let mut report = self.report.read().clone();
        report.state_db_mem = self.state_db.read().mem_used();
        report
    }

    /// Tick the client.
    // TODO: manage by real events.
    pub fn tick(&self) { self.check_garbage(); }

    fn check_garbage(&self) {
        self.chain.read().collect_garbage();
        self.block_queue.collect_garbage();
    }

    /// Ask the client what the history parameter is.
    pub fn pruning_history(&self) -> u64 { self.history }

    fn block_hash(chain: &BlockChain, miner: &Miner, id: BlockId) -> Option<H256> {
        match id {
            BlockId::Hash(hash) => Some(hash),
            BlockId::Number(number) => chain.block_hash(number),
            BlockId::Earliest => chain.block_hash(0),
            BlockId::Latest => Some(chain.best_block_hash()),
            BlockId::Pending => {
                miner
                    .pending_block_header(chain.best_block_number())
                    .map(|header| header.hash())
            }
        }
    }

    fn transaction_address(&self, id: TransactionId) -> Option<TransactionAddress> {
        match id {
            TransactionId::Hash(ref hash) => self.chain.read().transaction_address(hash),
            TransactionId::Location(id, index) => {
                Self::block_hash(&self.chain.read(), &self.miner, id).map(|hash| {
                    TransactionAddress {
                        block_hash: hash,
                        index,
                    }
                })
            }
        }
    }

    // transaction for calling contracts from services like engine.
    // from the null sender, with 50M gas.
    fn contract_call_tx(
        &self,
        block_id: BlockId,
        address: Address,
        data: Bytes,
    ) -> SignedTransaction
    {
        let from = Address::default();
        Transaction::new(
            self.nonce(&from, block_id)
                .unwrap_or_else(|| self.engine.machine().account_start_nonce(0)),
            U256::default(),
            U256::from(50_000_000),
            Action::Call(address),
            U256::default(),
            data,
            DEFAULT_TRANSACTION_TYPE,
        )
        .fake_sign(from)
    }

    fn do_virtual_call(
        machine: &::machine::EthereumMachine,
        env_info: &EnvInfo,
        state: &mut State<StateDB>,
        t: &SignedTransaction,
        analytics: CallAnalytics,
    ) -> Result<Executed, CallError>
    {
        fn for_local_avm(state: &mut State<StateDB>, transaction: &SignedTransaction) -> bool {
            if transaction.tx_type() == AVM_TRANSACTION_TYPE {
                return true;
            } else if let Action::Call(a) = transaction.action {
                let code = state.code(&a).unwrap_or(None);
                if let Some(c) = code {
                    return c[0..2] != [0x60u8, 0x50];
                }
            }

            return false;
        }

        fn call(
            state: &mut State<StateDB>,
            env_info: &EnvInfo,
            machine: &::machine::EthereumMachine,
            state_diff: bool,
            transaction: &SignedTransaction,
        ) -> Result<Executed, CallError>
        {
            let original_state = if state_diff {
                Some(state.clone())
            } else {
                None
            };

            let mut ret;
            let aion040fork = machine
                .params()
                .monetary_policy_update
                .map_or(false, |v| env_info.number >= v);
            if aion040fork && for_local_avm(state, transaction) {
                let avm_result = Executive::new(state, env_info, machine)
                    .transact_virtual_bulk(&[transaction.clone()], false);
                match avm_result[0].clone() {
                    Err(x) => return Err(x.into()),
                    Ok(_) => ret = avm_result[0].clone().unwrap(),
                }
            } else {
                ret = Executive::new(state, env_info, machine)
                    .transact_virtual(transaction, false)?;
            }

            debug!(target: "vm", "local call result = {:?}", ret);

            if let Some(original) = original_state {
                ret.state_diff = Some(state.diff_from(original).map_err(ExecutionError::from)?);
            }
            Ok(ret)
        }

        let state_diff = analytics.state_diffing;

        call(state, env_info, machine, state_diff, t)
    }

    fn block_number_ref(&self, id: &BlockId) -> Option<BlockNumber> {
        match *id {
            BlockId::Number(number) => Some(number),
            BlockId::Hash(ref hash) => self.chain.read().block_number(hash),
            BlockId::Earliest => Some(0),
            BlockId::Latest => Some(self.chain.read().best_block_number()),
            BlockId::Pending => Some(self.chain.read().best_block_number() + 1),
        }
    }

    /// revert database to blocknumber, only used in revert db
    pub fn revert_block(&self, to: BlockNumber) -> Result<(), String> {
        let state_db = self.state_db.read().boxed_clone();
        if !self.config.pruning.is_stable() {
            warn!(target:"revert","your pruning strategy is unstable! revert db may cause db crash");
        }
        match state_db.journal_db().earliest_era() {
            Some(earliest_era) if to < earliest_era => {
                return Err(format!(
                    "In pruning mode: {} , journal for blk #{} has been pruned, the paramater \
                     'to' must grater or equal than {}",
                    self.config.pruning.as_str(),
                    to,
                    earliest_era,
                ));
            }
            _ => {}
        };
        let latest = self
            .block_number(BlockId::Latest)
            .expect("can not found latest block, db may crashed");
        if to > latest {
            return Err(format!(
                "The block #{} is greater than the current best block #{} stored in the database. \
                 Cannot move to that block.",
                to, latest
            ));
        }

        let target_block_header = self
            .block(BlockId::Number(to))
            .expect("can not found block , db may crashed")
            .header()
            .decode();
        let mut batch = DBTransaction::with_capacity(10000);
        let mut target_block_details = self
            .chain
            .read()
            .block_details(&target_block_header.hash())
            .expect("can not found block , db may crashed");
        target_block_details.children.clear();

        for blk in (to + 1..=latest).rev() {
            // delete col_headers && col_boides
            let hash = self
                .block_hash(BlockId::Number(blk))
                .expect("can not found block , db may crashed");
            batch.delete(::db::COL_HEADERS, &hash);
            batch.delete(::db::COL_BODIES, &hash);
            // delete col_extra
            let mut key_blk_detail = H264::default();
            let mut key_blk_recipts = H264::default();
            key_blk_detail[0] = 0u8;
            key_blk_recipts[0] = 4u8;
            (*key_blk_detail)[1..].clone_from_slice(&hash);
            (*key_blk_recipts)[1..].clone_from_slice(&hash);
            let mut blk_number = [0u8; 5];
            blk_number[0] = 1u8;
            blk_number[1] = (blk >> 24) as u8;
            blk_number[2] = (blk >> 16) as u8;
            blk_number[3] = (blk >> 8) as u8;
            blk_number[4] = blk as u8;
            batch.delete(::db::COL_EXTRA, &hash);
            batch.delete(::db::COL_EXTRA, &key_blk_detail);
            batch.delete(::db::COL_EXTRA, &key_blk_recipts);
            batch.delete(::db::COL_EXTRA, &blk_number);
            let header = self
                .chain
                .read()
                .block_header(&hash)
                .expect("can not found block , db may crashed");
            let state_root = header.state_root();
            batch.delete(::db::COL_STATE, &state_root);
            // flush dbtransaction
            if blk % 1000 == 0 {
                info!(target:"revert","#{}", blk);
                self.db
                    .write()
                    .write(batch.clone())
                    .map_err(|e| format!("db revert failed for: {:?}", e))?;
                batch.ops.clear();
            }
        }

        let new_block = to;
        let new_best_hash = self
            .block_hash(BlockId::Number(new_block))
            .expect("can not found block , db may crashed");
        // revert last block detail
        {
            let details = target_block_details;
            use db::Writable;
            batch.write(::db::COL_EXTRA, &new_best_hash, &details);
        }
        // update new best hash
        let new_best_hash = self
            .block_hash(BlockId::Number(new_block))
            .expect("can not found block , db may crashed");
        batch.put(::db::COL_EXTRA, b"best", &new_best_hash);
        // reset state
        let latest_era_key = [b'l', b'a', b's', b't', 0, 0, 0, 0, 0, 0, 0, 0];
        batch.put(::db::COL_STATE, &latest_era_key, &encode(&new_block));
        self.db
            .write()
            .write(batch)
            .map_err(|e| format!("db revert failed for: {:?}", e))?;

        Ok(())
    }
}

impl BlockChainClient for Client {
    fn call(
        &self,
        transaction: &SignedTransaction,
        analytics: CallAnalytics,
        block: BlockId,
    ) -> Result<Executed, CallError>
    {
        let mut env_info = self.env_info(block).ok_or(CallError::StatePruned)?;
        env_info.gas_limit = U256::max_value();

        // that's just a copy of the state.
        let mut state = self.state_at(block).ok_or(CallError::StatePruned)?;
        let machine = self.engine.machine();

        debug!(target: "vm", "fake transaction = {:?}", transaction);

        Self::do_virtual_call(machine, &env_info, &mut state, transaction, analytics)
    }

    fn call_many(
        &self,
        transactions: &[(SignedTransaction, CallAnalytics)],
        block: BlockId,
    ) -> Result<Vec<Executed>, CallError>
    {
        let mut env_info = self.env_info(block).ok_or(CallError::StatePruned)?;
        env_info.gas_limit = U256::max_value();

        // that's just a copy of the state.
        let mut state = self.state_at(block).ok_or(CallError::StatePruned)?;
        let mut results = Vec::with_capacity(transactions.len());
        let machine = self.engine.machine();

        for &(ref t, analytics) in transactions {
            let ret = Self::do_virtual_call(machine, &env_info, &mut state, t, analytics)?;
            env_info.gas_used = ret.cumulative_gas_used;
            results.push(ret);
        }

        Ok(results)
    }

    fn estimate_gas(&self, t: &SignedTransaction, block: BlockId) -> Result<U256, CallError> {
        let (mut upper, max_upper, env_info) = {
            let mut env_info = self.env_info(block).ok_or(CallError::StatePruned)?;
            let init = env_info.gas_limit;
            let max = init * U256::from(10);
            env_info.gas_limit = max;
            (init, max, env_info)
        };

        // that's just a copy of the state.
        let original_state = self.state_at(block).ok_or(CallError::StatePruned)?;
        let sender = t.sender();

        let cond = |gas| {
            let mut tx = t.as_unsigned().clone();
            tx.gas = gas;
            let tx = tx.fake_sign(sender);

            let mut state = original_state.clone();
            Ok(Executive::new(&mut state, &env_info, self.engine.machine())
                .transact_virtual(&tx, false)
                .map(|r| r.exception.as_str() == "")
                .unwrap_or(false))
        };

        if !cond(upper)? {
            upper = max_upper;
            if !cond(upper)? {
                trace!(target: "estimate_gas", "estimate_gas failed with {}", upper);
                let err = ExecutionError::Internal(format!(
                    "Requires higher than upper limit of {}",
                    upper
                ));
                return Err(err.into());
            }
        }
        let lower = t.gas_required();
        if cond(lower)? {
            trace!(target: "estimate_gas", "estimate_gas succeeded with {}", lower);
            return Ok(lower);
        }

        /// Find transition point between `lower` and `upper` where `cond` changes from `false` to `true`.
        /// Returns the lowest value between `lower` and `upper` for which `cond` returns true.
        /// We assert: `cond(lower) = false`, `cond(upper) = true`
        fn binary_chop<F, E>(mut lower: U256, mut upper: U256, mut cond: F) -> Result<U256, E>
        where F: FnMut(U256) -> Result<bool, E> {
            while upper - lower > 1.into() {
                let mid = (lower + upper) / 2.into();
                trace!(target: "estimate_gas", "{} .. {} .. {}", lower, mid, upper);
                let c = cond(mid)?;
                match c {
                    true => upper = mid,
                    false => lower = mid,
                };
                trace!(target: "estimate_gas", "{} => {} .. {}", c, lower, upper);
            }
            Ok(upper)
        }

        // binary chop to non-excepting call with gas somewhere between 21000 and block gas limit
        trace!(target: "estimate_gas", "estimate_gas chopping {} .. {}", lower, upper);
        binary_chop(lower, upper, cond)
    }

    fn replay(&self, id: TransactionId, analytics: CallAnalytics) -> Result<Executed, CallError> {
        let address = self
            .transaction_address(id)
            .ok_or(CallError::TransactionNotFound)?;
        let block = BlockId::Hash(address.block_hash);

        const PROOF: &'static str =
            "The transaction address contains a valid index within block; qed";
        Ok(self
            .replay_block_transactions(block, analytics)?
            .nth(address.index)
            .expect(PROOF))
    }

    fn replay_block_transactions(
        &self,
        block: BlockId,
        analytics: CallAnalytics,
    ) -> Result<Box<Iterator<Item = Executed>>, CallError>
    {
        let mut env_info = self.env_info(block).ok_or(CallError::StatePruned)?;
        let body = self.block_body(block).ok_or(CallError::StatePruned)?;
        let mut state = self
            .state_at_beginning(block)
            .ok_or(CallError::StatePruned)?;
        let txs = body.transactions();
        let engine = self.engine.clone();

        const PROOF: &'static str =
            "Transactions fetched from blockchain; blockchain transactions are valid; qed";
        const EXECUTE_PROOF: &'static str = "Transaction replayed; qed";

        Ok(Box::new(txs.into_iter().map(move |t| {
            let t = SignedTransaction::new(t).expect(PROOF);
            let machine = engine.machine();
            let x = Self::do_virtual_call(machine, &env_info, &mut state, &t, analytics)
                .expect(EXECUTE_PROOF);
            env_info.gas_used = env_info.gas_used + x.gas_used;
            x
        })))
    }

    fn disable(&self) {
        self.enabled.store(false, AtomicOrdering::Relaxed);
        self.clear_queue();
    }

    fn spec_name(&self) -> String { self.config.spec_name.clone() }

    fn best_block_header(&self) -> encoded::Header { self.chain.read().best_block_header() }

    fn block_header(&self, id: BlockId) -> Option<::encoded::Header> {
        let chain = self.chain.read();

        if let BlockId::Pending = id {
            if let Some(block) = self.miner.pending_block(chain.best_block_number()) {
                return Some(encoded::Header::new(block.header.rlp(Seal::Without)));
            }
            // fall back to latest
            return self.block_header(BlockId::Latest);
        }

        Self::block_hash(&chain, &self.miner, id).and_then(|hash| chain.block_header_data(&hash))
    }

    fn block_number(&self, id: BlockId) -> Option<BlockNumber> { self.block_number_ref(&id) }

    fn block_body(&self, id: BlockId) -> Option<encoded::Body> {
        let chain = self.chain.read();

        if let BlockId::Pending = id {
            if let Some(block) = self.miner.pending_block(chain.best_block_number()) {
                return Some(encoded::Body::new(BlockChain::block_to_body(
                    &block.rlp_bytes(Seal::Without),
                )));
            }
            // fall back to latest
            return self.block_body(BlockId::Latest);
        }

        Self::block_hash(&chain, &self.miner, id).and_then(|hash| chain.block_body(&hash))
    }

    fn block(&self, id: BlockId) -> Option<encoded::Block> {
        let chain = self.chain.read();

        if let BlockId::Pending = id {
            if let Some(block) = self.miner.pending_block(chain.best_block_number()) {
                return Some(encoded::Block::new(block.rlp_bytes(Seal::Without)));
            }
            // fall back to latest
            return self.block(BlockId::Latest);
        }

        Self::block_hash(&chain, &self.miner, id).and_then(|hash| chain.block(&hash))
    }

    fn block_status(&self, id: BlockId) -> BlockStatus {
        if let BlockId::Pending = id {
            return BlockStatus::Pending;
        }

        let chain = self.chain.read();
        match Self::block_hash(&chain, &self.miner, id) {
            Some(ref hash) if chain.is_known(hash) => BlockStatus::InChain,
            Some(hash) => self.block_queue.status(&hash).into(),
            None => BlockStatus::Unknown,
        }
    }

    fn block_total_difficulty(&self, id: BlockId) -> Option<U256> {
        let chain = self.chain.read();
        if let BlockId::Pending = id {
            let latest_difficulty = self
                .block_total_difficulty(BlockId::Latest)
                .expect("blocks in chain have details; qed");
            let pending_difficulty = self
                .miner
                .pending_block_header(chain.best_block_number())
                .map(|header| *header.difficulty());
            if let Some(difficulty) = pending_difficulty {
                return Some(difficulty + latest_difficulty);
            }
            // fall back to latest
            return Some(latest_difficulty);
        }

        Self::block_hash(&chain, &self.miner, id)
            .and_then(|hash| chain.block_details(&hash))
            .map(|d| d.total_difficulty)
    }

    fn nonce(&self, address: &Address, id: BlockId) -> Option<U256> {
        self.state_at(id).and_then(|s| s.nonce(address).ok())
    }

    //TODO: update account type
    fn storage_root(&self, address: &Address, id: BlockId) -> Option<H256> {
        self.state_at(id)
            .and_then(|s| s.storage_root(address).ok())
            .and_then(|x| x)
    }

    fn block_hash(&self, id: BlockId) -> Option<H256> {
        let chain = self.chain.read();
        Self::block_hash(&chain, &self.miner, id)
    }

    fn code(&self, address: &Address, id: BlockId) -> Option<Option<Bytes>> {
        self.state_at(id)
            .and_then(|s| s.code(address).ok())
            .map(|c| c.map(|c| (&*c).clone()))
    }

    fn code_hash(&self, address: &Address, id: BlockId) -> Option<H256> {
        self.state_at(id).and_then(|s| s.code_hash(address).ok())
    }

    fn balance(&self, address: &Address, id: BlockId) -> Option<U256> {
        self.state_at(id).and_then(|s| s.balance(address).ok())
    }

    fn storage_at(&self, address: &Address, position: &H128, id: BlockId) -> Option<H128> {
        let value = self
            .state_at(id)
            .and_then(|s| s.storage_at(address, &position[..].to_vec()).ok());
        if let Some(v1) = value {
            if let Some(v) = v1 {
                let mut ret = vec![0u8; 16];
                for idx in 0..v.len() {
                    ret[16 - v.len() + idx] = v[idx];
                }
                Some(ret[..].into())
            } else {
                None
            }
        } else {
            None
        }
    }

    fn list_accounts(
        &self,
        id: BlockId,
        after: Option<&Address>,
        count: u64,
    ) -> Option<Vec<Address>>
    {
        if !self.factories.trie.is_fat() {
            trace!(target: "fatdb", "list_accounts: Not a fat DB");
            return None;
        }

        let state = match self.state_at(id) {
            Some(state) => state,
            _ => return None,
        };

        let (root, db) = state.drop();
        let trie = match self.factories.trie.readonly(db.as_hashstore(), &root) {
            Ok(trie) => trie,
            _ => {
                trace!(target: "fatdb", "list_accounts: Couldn't open the DB");
                return None;
            }
        };

        let mut iter = match trie.iter() {
            Ok(iter) => iter,
            _ => return None,
        };

        if let Some(after) = after {
            if let Err(e) = iter.seek(after) {
                trace!(target: "fatdb", "list_accounts: Couldn't seek the DB: {:?}", e);
            }
        }

        let accounts = iter
            .filter_map(|item| item.ok().map(|(addr, _)| Address::from_slice(&addr)))
            .take(count as usize)
            .collect();

        Some(accounts)
    }

    fn list_storage(
        &self,
        id: BlockId,
        account: &Address,
        after: Option<&H128>,
        count: u64,
    ) -> Option<Vec<H128>>
    {
        if !self.factories.trie.is_fat() {
            trace!(target: "fatdb", "list_stroage: Not a fat DB");
            return None;
        }

        let state = match self.state_at(id) {
            Some(state) => state,
            _ => return None,
        };

        let root = match state.storage_root(account) {
            Ok(Some(root)) => root,
            _ => return None,
        };

        let (_, db) = state.drop();
        let account_db = self
            .factories
            .accountdb
            .readonly(db.as_hashstore(), blake2b(account));
        let trie = match self
            .factories
            .trie
            .readonly(account_db.as_hashstore(), &root)
        {
            Ok(trie) => trie,
            _ => {
                trace!(target: "fatdb", "list_storage: Couldn't open the DB");
                return None;
            }
        };

        let mut iter = match trie.iter() {
            Ok(iter) => iter,
            _ => return None,
        };

        if let Some(after) = after {
            if let Err(e) = iter.seek(after) {
                trace!(target: "fatdb", "list_accounts: Couldn't seek the DB: {:?}", e);
            }
        }

        let keys = iter
            .filter_map(|item| item.ok().map(|(key, _)| H128::from_slice(&key)))
            .take(count as usize)
            .collect();

        Some(keys)
    }

    fn transaction(&self, id: TransactionId) -> Option<LocalizedTransaction> {
        self.transaction_address(id)
            .and_then(|address| self.chain.read().transaction(&address))
    }

    fn transaction_block(&self, id: TransactionId) -> Option<H256> {
        self.transaction_address(id).map(|addr| addr.block_hash)
    }

    fn transaction_receipt(&self, id: TransactionId) -> Option<LocalizedReceipt> {
        let chain = self.chain.read();
        self.transaction_address(id).and_then(|address| {
            chain
                .block_number(&address.block_hash)
                .and_then(|block_number| {
                    let transaction = chain.block_body(&address.block_hash).and_then(|body| {
                        body.view().localized_transaction_at(
                            &address.block_hash,
                            block_number,
                            address.index,
                        )
                    });

                    let previous_receipts = (0..address.index + 1)
                        .map(|index| {
                            let mut address = address.clone();
                            address.index = index;
                            chain.transaction_receipt(&address)
                        })
                        .collect();
                    match (transaction, previous_receipts) {
                        (Some(transaction), Some(previous_receipts)) => {
                            Some(transaction_receipt(transaction, previous_receipts))
                        }
                        _ => None,
                    }
                })
        })
    }

    fn tree_route(&self, from: &H256, to: &H256) -> Option<TreeRoute> {
        let chain = self.chain.read();
        match chain.is_known(from) && chain.is_known(to) {
            true => chain.tree_route(from.clone(), to.clone()),
            false => None,
        }
    }

    fn state_data(&self, hash: &H256) -> Option<Bytes> {
        self.state_db.read().journal_db().state(hash)
    }

    fn block_receipts(&self, hash: &H256) -> Option<Bytes> {
        self.chain
            .read()
            .block_receipts(hash)
            .map(|receipts| ::rlp::encode(&receipts).into_vec())
    }

    fn import_block(&self, bytes: Bytes) -> Result<H256, BlockImportError> {
        use verification::queue::kind::blocks::Unverified;
        use verification::queue::kind::BlockLike;

        // create unverified block here so the `blake2b` calculation can be cached.
        let unverified = Unverified::new(bytes);

        {
            if self.chain.read().is_known(&unverified.hash()) {
                return Err(BlockImportError::Import(ImportError::AlreadyInChain));
            }

            let parent_hash = unverified.parent_hash();
            debug!(target: "block","parent_hash: {}", parent_hash);

            let status = self.block_status(BlockId::Hash(parent_hash));
            if status == BlockStatus::Unknown || status == BlockStatus::Pending {
                return Err(BlockImportError::Block(BlockError::UnknownParent(
                    unverified.parent_hash(),
                )));
            }
        }
        Ok(self.block_queue.import(unverified)?)
    }

    fn queue_info(&self) -> BlockQueueInfo { self.block_queue.queue_info() }

    fn clear_queue(&self) { self.block_queue.clear(); }

    fn clear_bad(&self) { self.block_queue.clear_bad(); }

    fn chain_info(&self) -> BlockChainInfo {
        let mut chain_info = self.chain.read().chain_info();
        chain_info.pending_total_difficulty =
            chain_info.total_difficulty + self.block_queue.total_difficulty();
        chain_info
    }

    fn logs(&self, filter: Filter) -> Vec<LocalizedLogEntry> {
        let (from, to) = match (
            self.block_number_ref(&filter.from_block),
            self.block_number_ref(&filter.to_block),
        ) {
            (Some(from), Some(to)) => (from, to),
            _ => return Vec::new(),
        };

        let chain = self.chain.read();
        let blocks = filter
            .bloom_possibilities()
            .iter()
            .map(move |bloom| chain.blocks_with_bloom(bloom, from, to))
            .flat_map(|m| m)
            // remove duplicate elements
            .collect::<HashSet<u64>>()
            .into_iter()
            .collect::<Vec<u64>>();

        self.chain
            .read()
            .logs(blocks, |entry| filter.matches(entry), filter.limit)
    }

    fn last_hashes(&self) -> LastHashes {
        (*self.build_last_hashes(self.chain.read().best_block_hash())).clone()
    }

    fn import_queued_transactions(&self, transactions: Vec<UnverifiedTransaction>) {
        trace_time!("import_queued_transactions");

        self.miner.import_external_transactions(self, transactions);
    }

    fn ready_transactions(&self) -> Vec<PendingTransaction> {
        let (number, timestamp) = {
            let chain = self.chain.read();
            (chain.best_block_number(), chain.best_block_timestamp())
        };
        self.miner.ready_transactions(number, timestamp)
    }

    fn queue_consensus_message(&self, message: Bytes) {
        let channel = self.io_channel.lock().clone();
        if let Err(e) = channel.send(ClientIoMessage::NewMessage(message)) {
            debug!(target:"client","Ignoring the message, error queueing: {}", e);
        }
    }

    fn pruning_info(&self) -> PruningInfo {
        PruningInfo {
            earliest_chain: self.chain.read().first_block_number().unwrap_or(1),
            earliest_state: self
                .state_db
                .read()
                .journal_db()
                .earliest_era()
                .unwrap_or(0),
        }
    }

    fn call_contract(
        &self,
        block_id: BlockId,
        address: Address,
        data: Bytes,
    ) -> Result<Bytes, String>
    {
        let transaction = self.contract_call_tx(block_id, address, data);

        self.call(&transaction, Default::default(), block_id)
            .map_err(|e| format!("{:?}", e))
            .map(|executed| executed.output)
    }
}

impl MiningBlockChainClient for Client {
    fn as_block_chain_client(&self) -> &BlockChainClient { self }

    fn prepare_open_block(
        &self,
        author: Address,
        gas_range_target: (U256, U256),
        extra_data: Bytes,
    ) -> OpenBlock
    {
        let engine = &*self.engine;
        let chain = self.chain.read();
        let h = chain.best_block_hash();
        let best_header = &chain
            .block_header(&h)
            .expect("h is best block hash: so its header must exist: qed");
        let grant_parent = chain.block_header(best_header.parent_hash());
        let grant_parent_header = grant_parent.as_ref();

        let open_block = OpenBlock::new(
            engine,
            self.factories.clone(),
            self.state_db.read().boxed_clone_canon(&h),
            best_header,
            grant_parent_header,
            self.build_last_hashes(h.clone()),
            author,
            gas_range_target,
            extra_data,
            self.db.read().clone(),
        )
        .expect(
            "OpenBlock::new only fails if parent state root invalid; state root of best block's \
             header is never invalid; qed",
        );

        open_block
    }

    fn reopen_block(&self, block: ClosedBlock) -> OpenBlock {
        let engine = &*self.engine;
        let block = block.reopen(engine);
        block
    }

    fn vm_factory(&self) -> &VmFactory { &self.factories.vm }

    fn broadcast_transaction(&self, transaction: Bytes) {
        self.notify(|notify| {
            notify.transactions_received(&vec![transaction.clone()]);
        });
    }

    fn broadcast_proposal_block(&self, block: SealedBlock) {
        self.notify(|notify| {
            notify.new_blocks(
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![block.rlp_bytes()],
                0,
            );
        });
    }

    fn import_sealed_block(&self, block: SealedBlock) -> ImportResult {
        let h = block.header().hash();
        let start = precise_time_ns();
        let route = {
            // scope for self.import_lock
            let _import_lock = self.import_lock.lock();
            trace_time!("import_sealed_block");

            let number = block.header().number();
            let block_data = block.rlp_bytes();
            let header = block.header().clone();

            let route = self.commit_block(block, &header, &block_data);
            trace!(target: "client", "Imported sealed block #{} ({})", number, h);
            self.state_db
                .write()
                .sync_cache(&route.enacted, &route.retracted, false);
            route
        };
        let (enacted, retracted) = self.calculate_enacted_retracted(&[route]);
        self.miner
            .chain_new_blocks(self, &[h.clone()], &[], &enacted, &retracted);
        self.notify(|notify| {
            notify.new_blocks(
                vec![h.clone()],
                vec![],
                enacted.clone(),
                retracted.clone(),
                vec![h.clone()],
                vec![],
                precise_time_ns() - start,
            );
        });
        self.db.read().flush().expect("DB flush failed.");
        Ok(h)
    }

    fn prepare_block_interval(&self) -> Duration { self.miner.prepare_block_interval() }
}

impl super::traits::EngineClient for Client {
    fn update_sealing(&self) { self.miner.update_sealing(self) }

    fn submit_seal(&self, block_hash: H256, seal: Vec<Bytes>) {
        if self.miner.submit_seal(self, block_hash, seal).is_err() {
            warn!(target: "poa", "Wrong internal seal submission!")
        }
    }

    fn broadcast_consensus_message(&self, message: Bytes) {
        self.notify(|notify| notify.broadcast(message.clone()));
    }

    fn epoch_transition_for(&self, parent_hash: H256) -> Option<::engines::EpochTransition> {
        self.chain.read().epoch_transition_for(parent_hash)
    }

    fn chain_info(&self) -> BlockChainInfo { BlockChainClient::chain_info(self) }

    fn as_full_client(&self) -> Option<&BlockChainClient> { Some(self) }

    fn block_number(&self, id: BlockId) -> Option<BlockNumber> {
        BlockChainClient::block_number(self, id)
    }

    fn block_header(&self, id: BlockId) -> Option<::encoded::Header> {
        BlockChainClient::block_header(self, id)
    }
}

impl ProvingBlockChainClient for Client {
    fn prove_storage(&self, key1: H256, key2: H256, id: BlockId) -> Option<(Vec<Bytes>, H256)> {
        self.state_at(id)
            .and_then(move |state| state.prove_storage(key1, key2).ok())
    }

    fn prove_account(
        &self,
        key1: H256,
        id: BlockId,
    ) -> Option<(Vec<Bytes>, ::types::basic_account::BasicAccount)>
    {
        self.state_at(id)
            .and_then(move |state| state.prove_account(key1).ok())
    }

    fn prove_transaction(
        &self,
        transaction: SignedTransaction,
        id: BlockId,
    ) -> Option<(Bytes, Vec<DBValue>)>
    {
        let (header, mut env_info) = match (self.block_header(id), self.env_info(id)) {
            (Some(s), Some(e)) => (s, e),
            _ => return None,
        };

        env_info.gas_limit = transaction.gas.clone();
        let mut jdb = self.state_db.read().journal_db().boxed_clone();

        state::prove_transaction(
            jdb.as_hashstore_mut(),
            header.state_root().clone(),
            &transaction,
            self.engine.machine(),
            &env_info,
            self.factories.clone(),
            false,
            self.db.read().clone(),
        )
    }

    fn epoch_signal(&self, hash: H256) -> Option<Vec<u8>> {
        // pending transitions are never deleted, and do not contain
        // finality proofs by definition.
        self.chain
            .read()
            .get_pending_transition(hash)
            .map(|pending| pending.proof)
    }
}

/// Returns `LocalizedReceipt` given `LocalizedTransaction`
/// and a vector of receipts from given block up to transaction index.
fn transaction_receipt(
    mut tx: LocalizedTransaction,
    mut receipts: Vec<Receipt>,
) -> LocalizedReceipt
{
    assert_eq!(
        receipts.len(),
        tx.transaction_index + 1,
        "All previous receipts are provided."
    );

    let sender = tx.sender();
    let receipt = receipts.pop().expect("Current receipt is provided; qed");
    let prior_gas_used = receipts.iter().fold(0.into(), |b, r| b + r.gas_used);
    let no_of_logs = receipts
        .into_iter()
        .map(|receipt| receipt.logs().len())
        .sum::<usize>();
    let transaction_hash = tx.hash();
    let block_hash = tx.block_hash;
    let block_number = tx.block_number;
    let transaction_index = tx.transaction_index;

    LocalizedReceipt {
        transaction_hash: transaction_hash,
        transaction_index: transaction_index,
        block_hash: block_hash,
        block_number: block_number,
        cumulative_gas_used: receipt.gas_used + prior_gas_used,
        gas_used: receipt.gas_used,
        contract_address: match tx.action {
            Action::Call(_) => None,
            Action::Create => Some(contract_address(&sender, &tx.nonce).0),
        },
        logs: receipt
            .logs()
            .into_iter()
            .enumerate()
            .map(|(i, log)| {
                LocalizedLogEntry {
                    entry: log.clone(),
                    block_hash: block_hash,
                    block_number: block_number,
                    transaction_hash: transaction_hash,
                    transaction_index: transaction_index,
                    transaction_log_index: i,
                    log_index: no_of_logs + i,
                }
            })
            .collect(),
        log_bloom: receipt.log_bloom().clone(),
        state_root: receipt.state_root().clone(),
        from: Some(sender),
        to: match tx.action {
            Action::Create => None,
            Action::Call(ref address) => Some(address.clone().into()),
        },
        gas_price: tx.gas_price,
        gas_limit: tx.gas,
        output: receipt.output,
        error_message: receipt.error_message,
    }
}

/*#[cfg(test)]
mod tests {

    #[test]
    fn should_not_cache_details_before_commit() {
        use client::BlockChainClient;
        use tests::helpers::*;

        use std::thread;
        use std::time::Duration;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        use kvdb::DBTransaction;

        let client = generate_dummy_client(0);
        let genesis = client.chain_info().best_block_hash;
        let (new_hash, new_block) = get_good_dummy_block_hash();

        let go = {
            // Separate thread uncommited transaction
            let go = Arc::new(AtomicBool::new(false));
            let go_thread = go.clone();
            let another_client = client.clone();
            thread::spawn(move || {
                let mut batch = DBTransaction::new();
                another_client
                    .chain
                    .read()
                    .insert_block(&mut batch, &new_block, Vec::new());
                go_thread.store(true, Ordering::SeqCst);
            });
            go
        };

        while !go.load(Ordering::SeqCst) {
            thread::park_timeout(Duration::from_millis(5));
        }

        assert!(client.tree_route(&genesis, &new_hash).is_none());
    }
}*/
