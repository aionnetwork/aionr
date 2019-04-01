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

//! Blockchain block.

use std::cmp;
use std::sync::Arc;
use std::collections::HashSet;
use blake2b::BLAKE2B_NULL_RLP;
use triehash::ordered_trie_root;

use rlp::{UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};
use aion_types::{H256, U256, Address};
use ethbloom::Bloom;
use bytes::Bytes;
use unexpected::Mismatch;

use vms::{EnvInfo, LastHashes};
use engines::EthEngine;
use error::{Error, BlockError};
use factory::Factories;
use header::{Header, Seal};
use receipt::Receipt;
use state::State;
use state_db::StateDB;
use transaction::{UnverifiedTransaction, SignedTransaction, Error as TransactionError, AVM_TRANSACTION_TYPE, Action};
use verification::PreverifiedBlock;
use kvdb::KeyValueDB;

/// A block, encoded as it is on the block chain.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Block {
    /// The header of this block.
    pub header: Header,
    /// The transactions in this block.
    pub transactions: Vec<UnverifiedTransaction>,
}

impl Block {
    /// Returns true if the given bytes form a valid encoding of a block in RLP.
    pub fn is_good(b: &[u8]) -> bool { UntrustedRlp::new(b).as_val::<Block>().is_ok() }

    /// Get the RLP-encoding of the block with or without the seal.
    pub fn rlp_bytes(&self, seal: Seal) -> Bytes {
        let mut block_rlp = RlpStream::new_list(2);
        self.header.stream_rlp(&mut block_rlp, seal);
        block_rlp.append_list(&self.transactions);
        block_rlp.out()
    }
}

impl Decodable for Block {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.as_raw().len() != rlp.payload_info()?.total() {
            return Err(DecoderError::RlpIsTooBig);
        }
        if rlp.item_count()? != 2 {
            return Err(DecoderError::RlpIncorrectListLen);
        }
        Ok(Block {
            header: rlp.val_at(0)?,
            transactions: rlp.list_at(1)?,
        })
    }
}

/// An internal type for a block's common elements.
#[derive(Clone)]
pub struct ExecutedBlock {
    header: Header,
    transactions: Vec<SignedTransaction>,
    receipts: Vec<Receipt>,
    transactions_set: HashSet<H256>,
    state: State<StateDB>,
    last_hashes: Arc<LastHashes>,
}

impl ExecutedBlock {
    /// Create a new block from the given `state`.
    fn new(state: State<StateDB>, last_hashes: Arc<LastHashes>) -> ExecutedBlock {
        ExecutedBlock {
            header: Default::default(),
            transactions: Default::default(),
            receipts: Default::default(),
            transactions_set: Default::default(),
            state: state,
            last_hashes: last_hashes,
        }
    }

    /// Get the environment info concerning this block.
    pub fn env_info(&self) -> EnvInfo {
        // TODO: memoise.
        EnvInfo {
            number: self.header.number(),
            author: self.header.author().clone(),
            timestamp: self.header.timestamp(),
            difficulty: self.header.difficulty().clone(),
            last_hashes: self.last_hashes.clone(),
            gas_used: self
                .receipts
                .iter()
                .fold(U256::zero(), |b, r| b + r.gas_used),
            gas_limit: self.header.gas_limit().clone(),
        }
    }

    /// Get mutable access to a state.
    pub fn state_mut(&mut self) -> &mut State<StateDB> { &mut self.state }

    /// Get mutable reference to header.
    pub fn header_mut(&mut self) -> &mut Header { &mut self.header }
}

/// Trait for a object that is a `ExecutedBlock`.
pub trait IsBlock {
    /// Get the `ExecutedBlock` associated with this object.
    fn block(&self) -> &ExecutedBlock;

    /// Get the base `Block` object associated with this.
    fn to_base(&self) -> Block {
        Block {
            header: self.header().clone(),
            transactions: self
                .transactions()
                .iter()
                .cloned()
                .map(Into::into)
                .collect(),
        }
    }

    /// Get the header associated with this object's block.
    fn header(&self) -> &Header { &self.block().header }

    /// Get the final state associated with this object's block.
    fn state(&self) -> &State<StateDB> { &self.block().state }

    /// Get all information on transactions in this block.
    fn transactions(&self) -> &[SignedTransaction] { &self.block().transactions }

    /// Get all information on receipts in this block.
    fn receipts(&self) -> &[Receipt] { &self.block().receipts }
}

/// Trait for a object that has a state database.
pub trait Drain {
    /// Drop this object and return the underlying database.
    fn drain(self) -> StateDB;
}

impl IsBlock for ExecutedBlock {
    fn block(&self) -> &ExecutedBlock { self }
}

impl ::aion_machine::LiveBlock for ExecutedBlock {
    type Header = Header;

    fn header(&self) -> &Header { &self.header }
}

impl ::aion_machine::Transactions for ExecutedBlock {
    type Transaction = SignedTransaction;

    fn transactions(&self) -> &[SignedTransaction] { &self.transactions }
}

/// Block that is ready for transactions to be added.
///
/// It's a bit like a Vec<Transaction>, except that whenever a transaction is pushed, we execute it and
/// maintain the system `state()`. We also archive execution receipts in preparation for later block creation.
pub struct OpenBlock<'x> {
    block: ExecutedBlock,
    engine: &'x EthEngine,
}

/// Just like `OpenBlock`, except that we've applied `Engine::on_close_block`, finished up the non-seal header fields,
/// and collected the uncles.
///
/// There is no function available to push a transaction.
#[derive(Clone)]
pub struct ClosedBlock {
    block: ExecutedBlock,
    unclosed_state: State<StateDB>,
}

/// Just like `ClosedBlock` except that we can't reopen it and it's faster.
///
/// We actually store the post-`Engine::on_close_block` state, unlike in `ClosedBlock` where it's the pre.
#[derive(Clone)]
pub struct LockedBlock {
    block: ExecutedBlock,
}

/// A block that has a valid seal.
///
/// The block's header has valid seal arguments. The block cannot be reversed into a `ClosedBlock` or `OpenBlock`.
pub struct SealedBlock {
    block: ExecutedBlock,
}

impl<'x> OpenBlock<'x> {
    /// Create a new `OpenBlock` ready for transaction pushing.
    pub fn new(
        engine: &'x EthEngine,
        factories: Factories,
        db: StateDB,
        parent: &Header,
        grant_parent: Option<&Header>,
        last_hashes: Arc<LastHashes>,
        author: Address,
        gas_range_target: (U256, U256),
        extra_data: Bytes,
        is_epoch_begin: bool,
        kvdb: Arc<KeyValueDB>,
    ) -> Result<Self, Error>
    {
        let number = parent.number() + 1;

        let state = State::from_existing(
            db,
            parent.state_root().clone(),
            engine.account_start_nonce(number),
            factories,
            kvdb.clone(),
        )?;
        let mut r = OpenBlock {
            block: ExecutedBlock::new(state, last_hashes),
            engine: engine,
        };

        r.block.header.set_parent_hash(parent.hash());
        r.block.header.set_number(number);
        r.block.header.set_author(author);
        r.block.header.set_timestamp_now(parent.timestamp());
        r.set_extra_data(extra_data);
        r.block.header.note_dirty();

        let gas_floor_target = cmp::max(gas_range_target.0, engine.params().min_gas_limit);
        let gas_ceil_target = cmp::max(gas_range_target.1, gas_floor_target);

        engine.machine().populate_from_parent(
            &mut r.block.header,
            parent,
            gas_floor_target,
            gas_ceil_target,
        );
        engine.populate_from_parent(&mut r.block.header, parent, grant_parent);

        engine.machine().on_new_block(&mut r.block)?;
        engine.on_new_block(&mut r.block, is_epoch_begin)?;

        Ok(r)
    }

    /// Alter the author for the block.
    pub fn set_author(&mut self, author: Address) { self.block.header.set_author(author); }

    /// Alter the timestamp of the block.
    pub fn set_timestamp(&mut self, timestamp: u64) { self.block.header.set_timestamp(timestamp); }

    /// Alter the difficulty for the block.
    pub fn set_difficulty(&mut self, a: U256) { self.block.header.set_difficulty(a); }

    /// Alter the gas limit for the block.
    pub fn set_gas_limit(&mut self, a: U256) { self.block.header.set_gas_limit(a); }

    /// Alter the gas limit for the block.
    pub fn set_gas_used(&mut self, a: U256) { self.block.header.set_gas_used(a); }

    /// Alter transactions root for the block.
    pub fn set_transactions_root(&mut self, h: H256) { self.block.header.set_transactions_root(h); }

    /// Alter the receipts root for the block.
    pub fn set_receipts_root(&mut self, h: H256) { self.block.header.set_receipts_root(h); }

    /// Alter the extra_data for the block.
    pub fn set_extra_data(&mut self, extra_data: Bytes) {
        let len = self.engine.maximum_extra_data_size();
        let mut data = extra_data;
        data.resize(len, 0u8);
        self.block.header.set_extra_data(data);
    }

    /// Get the environment info concerning this block.
    pub fn env_info(&self) -> EnvInfo { self.block.env_info() }

    // apply avm transactions
    pub fn apply_batch_txs(
        &mut self,
        txs: &[SignedTransaction],
        h: Option<H256>,
    ) -> Vec<Result<Receipt, Error>>
    {
        //TODO: deal with AVM parallelism
        if !txs
            .iter()
            .filter(|t| self.block.transactions_set.contains(&t.hash()))
            .collect::<Vec<_>>()
            .is_empty()
        {
            return vec![Err(From::from(TransactionError::AlreadyImported))];
        }
        let env_info = self.env_info();
        let mut idx = 0;
        let mut receipts_results = Vec::new();
        for apply_result in self
            .block
            .state
            .apply_batch(&env_info, self.engine.machine(), txs)
        {
            let result = match apply_result {
                Ok(outcome) => {
                    self.block
                        .transactions_set
                        .insert(h.unwrap_or_else(|| txs[idx].hash()));
                    self.block.transactions.push(txs[idx].clone().into());
                    self.block
                        .header
                        .add_transaction_fee(&outcome.receipt.transaction_fee);
                    self.block.receipts.push(outcome.receipt.clone());
                    idx += 1;
                    Ok(outcome.receipt)
                }
                Err(x) => Err(From::from(x)),
            };

            receipts_results.push(result);
        }

        receipts_results
    }

    /// Push a transaction into the block.
    ///
    /// If valid, it will be executed, and archived together with the receipt.
    /// This method is triggered both by sync and miner:
    /// sync module really call push_transactions, and we do bulk pushes during sync;
    /// however miner do not use push_transactions due to some special logic: 
    /// transaction penalisation .etc
    pub fn push_transaction(
        &mut self,
        t: SignedTransaction,
        h: Option<H256>,
    ) -> Result<&Receipt, Error>
    {
        if self.block.transactions_set.contains(&t.hash()) {
            return Err(From::from(TransactionError::AlreadyImported));
        }

        let mut result = Vec::new();
        let env_info = self.env_info();
        debug!(target: "vm", "tx type = {:?}", t.tx_type());
        if t.tx_type() == AVM_TRANSACTION_TYPE {
            result.append(&mut self.block.state.apply_batch(&env_info, self.engine.machine(), &[t.clone()]));
        } else {
            result.push(self.block.state.apply(&env_info, self.engine.machine(), &t));
        }

        
        match result.pop().unwrap() {
            Ok(outcome) => {
                self.block
                    .transactions_set
                    .insert(h.unwrap_or_else(|| t.hash()));
                self.block.transactions.push(t.into());
                self.block
                    .header
                    .add_transaction_fee(&outcome.receipt.transaction_fee);
                self.block.receipts.push(outcome.receipt);
                Ok(self
                    .block
                    .receipts
                    .last()
                    .expect("receipt just pushed; qed"))
            }
            Err(x) => Err(From::from(x)),
        }
    }

    /// Push transactions onto the block.
    pub fn push_transactions(&mut self, transactions: &[SignedTransaction]) -> Result<(), Error> {
        push_transactions(self, transactions)
    }

    /// Populate self from a header.
    pub fn populate_from(&mut self, header: &Header) {
        self.set_difficulty(*header.difficulty());
        self.set_gas_limit(*header.gas_limit());
        self.set_timestamp(header.timestamp());
        self.set_author(header.author().clone());
        self.set_extra_data(header.extra_data().clone());
        self.set_transactions_root(header.transactions_root().clone());
    }

    /// Turn this into a `ClosedBlock`.
    pub fn close(self) -> ClosedBlock {
        let mut s = self;

        let unclosed_state = s.block.state.clone();

        if let Err(e) = s.engine.on_close_block(&mut s.block) {
            warn!(target:"block","Encountered error on closing the block: {}", e);
        }

        if let Err(e) = s.block.state.commit() {
            warn!(target:"block","Encountered error on state commit: {}", e);
        }
        s.block.header.set_transactions_root(ordered_trie_root(
            s.block.transactions.iter().map(|e| e.rlp_bytes()),
        ));
        s.block.header.set_state_root(s.block.state.root().clone());
        s.block.header.set_receipts_root(ordered_trie_root(
            s.block
                .receipts
                .iter()
                .map(|r| r.simple_receipt().rlp_bytes()),
        ));
        s.block
            .header
            .set_log_bloom(s.block.receipts.iter().fold(Bloom::zero(), |mut b, r| {
                b = &b | &r.log_bloom();
                b
            })); //TODO: use |= operator
        s.block.header.set_gas_used(
            s.block
                .receipts
                .iter()
                .fold(U256::zero(), |b, r| b + r.gas_used),
        );

        ClosedBlock {
            block: s.block,
            unclosed_state: unclosed_state,
        }
    }

    /// Turn this into a `LockedBlock`.
    pub fn close_and_lock(self) -> LockedBlock {
        let mut s = self;

        if let Err(e) = s.engine.on_close_block(&mut s.block) {
            warn!(target:"block","Encountered error on closing the block: {}", e);
        }

        if let Err(e) = s.block.state.commit() {
            warn!(target:"block","Encountered error on state commit: {}", e);
        }

        if s.block.header.transactions_root().is_zero()
            || s.block.header.transactions_root() == &BLAKE2B_NULL_RLP
        {
            s.block.header.set_transactions_root(ordered_trie_root(
                s.block.transactions.iter().map(|e| e.rlp_bytes()),
            ));
        }
        if s.block.header.receipts_root().is_zero()
            || s.block.header.receipts_root() == &BLAKE2B_NULL_RLP
        {
            s.block.header.set_receipts_root(ordered_trie_root(
                s.block
                    .receipts
                    .iter()
                    .map(|r| r.simple_receipt().rlp_bytes()),
            ));
        }

        // s.block.header.set_state_root(s.block.state.root().clone());
        s.block.header.set_state_root(s.block.state.root().clone());

        s.block
            .header
            .set_log_bloom(s.block.receipts.iter().fold(Bloom::zero(), |mut b, r| {
                b = &b | &r.log_bloom();
                b
            })); //TODO: use |= operator
        s.block.header.set_gas_used(
            s.block
                .receipts
                .iter()
                .fold(U256::zero(), |b, r| b + r.gas_used),
        );

        LockedBlock {
            block: s.block,
        }
    }

    #[cfg(test)]
    /// Return mutable block reference. To be used in tests only.
    pub fn block_mut(&mut self) -> &mut ExecutedBlock { &mut self.block }
}

impl<'x> IsBlock for OpenBlock<'x> {
    fn block(&self) -> &ExecutedBlock { &self.block }
}

impl<'x> IsBlock for ClosedBlock {
    fn block(&self) -> &ExecutedBlock { &self.block }
}

impl<'x> IsBlock for LockedBlock {
    fn block(&self) -> &ExecutedBlock { &self.block }
}

impl ClosedBlock {
    /// Get the hash of the header without seal arguments.
    pub fn hash(&self) -> H256 { self.header().rlp_blake2b(Seal::Without) }

    /// Turn this into a `LockedBlock`, unable to be reopened again.
    pub fn lock(self) -> LockedBlock {
        LockedBlock {
            block: self.block,
        }
    }

    /// Given an engine reference, reopen the `ClosedBlock` into an `OpenBlock`.
    pub fn reopen(self, engine: &EthEngine) -> OpenBlock {
        // revert rewards (i.e. set state back at last transaction's state).
        let mut block = self.block;
        block.state = self.unclosed_state;
        OpenBlock {
            block: block,
            engine: engine,
        }
    }
}

impl LockedBlock {
    /// Get the hash of the header without seal arguments.
    pub fn hash(&self) -> H256 { self.header().rlp_blake2b(Seal::Without) }

    /// Provide a valid seal in order to turn this into a `SealedBlock`.
    ///
    /// NOTE: This does not check the validity of `seal` with the engine.
    pub fn seal(self, engine: &EthEngine, seal: Vec<Bytes>) -> Result<SealedBlock, BlockError> {
        let expected_seal_fields = engine.seal_fields(self.header());
        let mut s = self;
        if seal.len() != expected_seal_fields {
            return Err(BlockError::InvalidSealArity(Mismatch {
                expected: expected_seal_fields,
                found: seal.len(),
            }));
        }
        s.block.header.set_seal(seal);
        Ok(SealedBlock {
            block: s.block,
        })
    }

    /// Provide a valid seal in order to turn this into a `SealedBlock`.
    /// This does check the validity of `seal` with the engine.
    /// Returns the `ClosedBlock` back again if the seal is no good.
    pub fn try_seal(
        self,
        engine: &EthEngine,
        seal: Vec<Bytes>,
    ) -> Result<SealedBlock, (Error, LockedBlock)>
    {
        let mut s = self;
        s.block.header.set_seal(seal);

        // TODO: passing state context to avoid engines owning it?
        match engine.verify_local_seal(&s.block.header) {
            Err(e) => Err((e, s)),
            _ => {
                Ok(SealedBlock {
                    block: s.block,
                })
            }
        }
    }
}

impl Drain for LockedBlock {
    /// Drop this object and return the underlieing database.
    fn drain(self) -> StateDB { self.block.state.drop().1 }
}

impl SealedBlock {
    /// Get the RLP-encoding of the block.
    pub fn rlp_bytes(&self) -> Bytes {
        let mut block_rlp = RlpStream::new_list(2);
        self.block.header.stream_rlp(&mut block_rlp, Seal::With);
        block_rlp.append_list(&self.block.transactions);
        block_rlp.out()
    }
}

impl Drain for SealedBlock {
    /// Drop this object and return the underlieing database.
    fn drain(self) -> StateDB { self.block.state.drop().1 }
}

impl IsBlock for SealedBlock {
    fn block(&self) -> &ExecutedBlock { &self.block }
}

/// Enact the block given by block header, transactions
pub fn enact(
    header: &Header,
    transactions: &[SignedTransaction],
    engine: &EthEngine,
    db: StateDB,
    parent: &Header,
    grant_parent: Option<&Header>,
    last_hashes: Arc<LastHashes>,
    factories: Factories,
    is_epoch_begin: bool,
    kvdb: Arc<KeyValueDB>,
) -> Result<LockedBlock, Error>
{
    {
        if ::log::max_log_level() >= ::log::LogLevel::Trace {
            let s = State::from_existing(
                db.boxed_clone(),
                parent.state_root().clone(),
                engine.account_start_nonce(parent.number() + 1),
                factories.clone(),
                kvdb.clone(),
            )?;
            trace!(target: "enact", "num={}, root={}, author={}, author_balance={}\n",
                header.number(), s.root(), header.author(), s.balance(&header.author())?);
        }
    }
    // let s = State::from_existing(db.boxed_clone(), parent.state_root().clone(), engine.account_start_nonce(parent.number() + 1), factories.clone())?;
    // error!(target: "enact", "num={}, root={}, author={}, author_balance={}\n",
    //     header.number(), s.root(), header.author(), s.balance(&header.author())?);

    let mut b = OpenBlock::new(
        engine,
        factories,
        db,
        parent,
        grant_parent,
        last_hashes,
        Address::new(),
        (3141562.into(), 31415620.into()),
        vec![],
        is_epoch_begin,
        kvdb,
    )?;

    b.populate_from(header);
    b.push_transactions(transactions)?;
    Ok(b.close_and_lock())
}

#[inline]
fn is_normal(
    block: &mut OpenBlock,
    tx: &SignedTransaction,
) -> bool
{
    if let Action::Call(a) = tx.action {
        if block.block.state.code(&a).unwrap().is_some() {
            return false;
        }
    }

    return true;
}

#[inline]
fn is_for_avm(
    block: &mut OpenBlock,
    tx: &SignedTransaction,
) -> bool
{
    return tx.tx_type() == AVM_TRANSACTION_TYPE || is_normal(block, tx);
}

#[inline]
#[cfg(not(feature = "slow-blocks"))]
fn push_transactions(
    block: &mut OpenBlock,
    transactions: &[SignedTransaction],
) -> Result<(), Error>
{
    let mut tx_batch = Vec::new();

    debug!(target: "vm", "transactions = {:?}, len = {:?}", transactions, transactions.len());
    for tx in transactions {
        if !is_for_avm(block, tx) {
            if tx_batch.len() >= 1 {
                block.apply_batch_txs(tx_batch.as_slice(), None);
                tx_batch.clear();
            }
            block.push_transaction(tx.clone(), None)?;
        } else {
            tx_batch.push(tx.clone())
        }
    }

    if !tx_batch.is_empty() {
        block.apply_batch_txs(tx_batch.as_slice(), None);
    }

    debug!(target: "vm", "push transactions done");

    Ok(())
}

#[cfg(feature = "slow-blocks")]
fn push_transactions(
    block: &mut OpenBlock,
    transactions: &[SignedTransaction],
) -> Result<(), Error>
{
    use std::time;

    let slow_tx = option_env!("SLOW_TX_DURATION")
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    for t in transactions {
        let hash = t.hash();
        let start = time::Instant::now();
        block.push_transaction(t.clone(), None)?;
        let took = start.elapsed();
        let took_ms = took.as_secs() * 1000 + took.subsec_nanos() as u64 / 1000000;
        if took > time::Duration::from_millis(slow_tx) {
            warn!(
                target: "tx",
                "Heavy ({} ms) transaction in block {:?}: {:?}",
                took_ms,
                block.header().number(),
                hash
            );
        }
        debug!(target: "tx", "Transaction {:?} took: {} ms", hash, took_ms);
    }
    Ok(())
}

// TODO [ToDr] Pass `PreverifiedBlock` by move, this will avoid unecessary allocation
/// Enact the block given by `block_bytes` using `engine` on the database `db` with given `parent` block header
pub fn enact_verified(
    block: &PreverifiedBlock,
    engine: &EthEngine,
    db: StateDB,
    parent: &Header,
    grant_parent: Option<&Header>,
    last_hashes: Arc<LastHashes>,
    factories: Factories,
    is_epoch_begin: bool,
    kvdb: Arc<KeyValueDB>,
) -> Result<LockedBlock, Error>
{
    enact(
        &block.header,
        &block.transactions,
        engine,
        db,
        parent,
        grant_parent,
        last_hashes,
        factories,
        is_epoch_begin,
        kvdb,
    )
}

#[cfg(test)]
mod tests {
    use tests::helpers::*;
    use super::*;
    use engines::EthEngine;
    use vms::LastHashes;
    use error::Error;
    use header::Header;
    use factory::Factories;
    use state_db::StateDB;
    use views::BlockView;
    use aion_types::Address;
    use std::sync::Arc;
    use transaction::SignedTransaction;
    use kvdb::MemoryDBRepository;

    /// Enact the block given by `block_bytes` using `engine` on the database `db` with given `parent` block header
    fn enact_bytes(
        block_bytes: &[u8],
        engine: &EthEngine,
        db: StateDB,
        parent: &Header,
        _grant_parent: Option<&Header>,
        last_hashes: Arc<LastHashes>,
        factories: Factories,
    ) -> Result<LockedBlock, Error>
    {
        let block = BlockView::new(block_bytes);
        let header = block.header();
        let transactions: Result<Vec<_>, Error> = block
            .transactions()
            .into_iter()
            .map(SignedTransaction::new)
            .map(|r| r.map_err(Into::into))
            .collect();
        let transactions = transactions?;

        {
            if ::log::max_log_level() >= ::log::LogLevel::Trace {
                let s = State::from_existing(
                    db.boxed_clone(),
                    parent.state_root().clone(),
                    engine.account_start_nonce(parent.number() + 1),
                    factories.clone(),
                    Arc::new(MemoryDBRepository::new()),
                )?;
                trace!(target: "enact", "num={}, root={}, author={}, author_balance={}\n",
                    header.number(), s.root(), header.author(), s.balance(&header.author())?);
            }
        }

        let mut b = OpenBlock::new(
            engine,
            factories,
            db,
            parent,
            None,
            last_hashes,
            Address::new(),
            (3141562.into(), 31415620.into()),
            vec![],
            false,
            Arc::new(MemoryDBRepository::new()),
        )?;

        b.populate_from(&header);
        b.push_transactions(&transactions)?;

        Ok(b.close_and_lock())
    }

    /// Enact the block given by `block_bytes` using `engine` on the database `db` with given `parent` block header. Seal the block afterwards
    fn enact_and_seal(
        block_bytes: &[u8],
        engine: &EthEngine,
        db: StateDB,
        parent: &Header,
        last_hashes: Arc<LastHashes>,
        factories: Factories,
    ) -> Result<SealedBlock, Error>
    {
        let header = BlockView::new(block_bytes).header_view();
        Ok(enact_bytes(
            block_bytes,
            engine,
            db,
            parent,
            None,
            last_hashes,
            factories,
        )?
        .seal(engine, header.seal())?)
    }

    #[test]
    fn open_block() {
        use spec::*;
        let spec = Spec::new_test();
        let genesis_header = spec.genesis_header();
        let db = spec
            .ensure_db_good(get_temp_state_db(), &Default::default())
            .unwrap();
        let last_hashes = Arc::new(vec![genesis_header.hash()]);
        let b = OpenBlock::new(
            &*spec.engine,
            Default::default(),
            db,
            &genesis_header,
            None,
            last_hashes,
            Address::zero(),
            (3141562.into(), 31415620.into()),
            vec![],
            false,
            Arc::new(MemoryDBRepository::new()),
        )
        .unwrap();
        let b = b.close_and_lock();
        let _ = b.seal(&*spec.engine, vec![]);
    }

    #[test]
    fn enact_block() {
        use spec::*;
        let spec = Spec::new_test();
        let engine = &*spec.engine;
        let genesis_header = spec.genesis_header();

        let db = spec
            .ensure_db_good(get_temp_state_db(), &Default::default())
            .unwrap();
        let last_hashes = Arc::new(vec![genesis_header.hash()]);
        let b = OpenBlock::new(
            engine,
            Default::default(),
            db,
            &genesis_header,
            None,
            last_hashes.clone(),
            Address::zero(),
            (3141562.into(), 31415620.into()),
            vec![],
            false,
            Arc::new(MemoryDBRepository::new()),
        )
        .unwrap()
        .close_and_lock()
        .seal(engine, vec![])
        .unwrap();
        let orig_bytes = b.rlp_bytes();
        let orig_db = b.drain();

        let db = spec
            .ensure_db_good(get_temp_state_db(), &Default::default())
            .unwrap();
        let e = enact_and_seal(
            &orig_bytes,
            engine,
            db,
            &genesis_header,
            last_hashes,
            Default::default(),
        )
        .unwrap();

        assert_eq!(e.rlp_bytes(), orig_bytes);

        let db = e.drain();
        assert_eq!(orig_db.journal_db().keys(), db.journal_db().keys());
        assert!(
            orig_db
                .journal_db()
                .keys()
                .iter()
                .filter(|k| orig_db.journal_db().get(k.0) != db.journal_db().get(k.0))
                .next()
                == None
        );
    }
}
