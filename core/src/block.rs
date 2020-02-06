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
use vms::{EnvInfo, LastHashes};
use aion_types::{H256, U256, Address};
use ethbloom::Bloom;
use acore_bytes::Bytes;
use unexpected::Mismatch;
use engine::{Engine};
use types::error::{Error, BlockError};
use factory::Factories;
use header::{Header, Seal, SealType};
use receipt::Receipt;
use state::State;
use db::StateDB;
use transaction::{
    UnverifiedTransaction, SignedTransaction, Error as TransactionError, AVM_TRANSACTION_TYPE,
    Action,
};
use verification::PreverifiedBlock;
use kvdb::KeyValueDB;
use client::BlockChainClient;

use num_bigint::BigUint;

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
            state,
            last_hashes,
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

/// Block that is ready for transactions to be added.
///
/// It's a bit like a Vec<Transaction>, except that whenever a transaction is pushed, we execute it and
/// maintain the system `state()`. We also archive execution receipts in preparation for later block creation.
pub struct OpenBlock<'x> {
    block: ExecutedBlock,
    engine: &'x Engine,
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
#[derive(Clone)]
pub struct SealedBlock {
    block: ExecutedBlock,
}

impl<'x> OpenBlock<'x> {
    /// Create a new `OpenBlock` ready for transaction pushing.
    pub fn new(
        engine: &'x Engine,
        factories: Factories,
        db: StateDB,
        parent: &Header,
        seal_type: SealType,
        grand_parent: Option<&Header>,
        great_grand_parent: Option<&Header>,
        last_hashes: Arc<LastHashes>,
        author: Address,
        gas_range_target: (U256, U256),
        extra_data: Bytes,
        kvdb: Arc<KeyValueDB>,
        timestamp: Option<u64>,
        client: &BlockChainClient,
    ) -> Result<Self, Error>
    {
        let number = parent.number() + 1;

        let state = State::from_existing(
            db,
            parent.state_root().clone(),
            engine.machine().account_start_nonce(number),
            factories,
            kvdb.clone(),
        )?;

        let mut r = OpenBlock {
            block: ExecutedBlock::new(state, last_hashes),
            engine,
        };

        r.block.header.set_parent_hash(parent.hash());
        r.block.header.set_number(number);
        r.block.header.set_author(author);
        match timestamp {
            Some(timestamp) => {
                r.block
                    .header
                    .set_timestamp_later_than(timestamp, parent.timestamp());
            }
            None => {
                r.block
                    .header
                    .set_timestamp_now_later_than(parent.timestamp());
            }
        };
        r.block.header.set_seal_type(seal_type);
        r.set_extra_data(extra_data);
        r.block.header.note_dirty();

        let gas_floor_target =
            cmp::max(gas_range_target.0, engine.machine().params().min_gas_limit);
        let gas_ceil_target = cmp::max(gas_range_target.1, gas_floor_target);

        // Set gas_limit
        engine.machine().set_gas_limit_from_parent(
            &mut r.block.header,
            parent,
            gas_floor_target,
            gas_ceil_target,
        );
        // Set difficulty
        engine.set_difficulty_from_parent(
            &mut r.block.header,
            parent,
            grand_parent,
            great_grand_parent,
            client,
        );

        engine.machine().on_new_block(&mut r.block)?;

        Ok(r)
    }

    /// Alter the author for the block.
    pub fn set_author(&mut self, author: Address) { self.block.header.set_author(author); }

    /// Alter the timestamp of the block.
    pub fn set_timestamp(&mut self, timestamp: u64) { self.block.header.set_timestamp(timestamp); }

    /// Set the timestamp of the block to the current local time, but later than the given time.
    pub fn set_timestamp_now_later_than(&mut self, later_than: u64) {
        self.block.header.set_timestamp_now_later_than(later_than);
    }

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
        let len = self.engine.machine().maximum_extra_data_size();
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
        is_building_block: bool,
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
        // avm should deal with exceptions correctly
        for apply_result in
            self.block
                .state
                .apply_batch(&env_info, self.engine.machine(), txs, is_building_block)
        {
            let result = match apply_result {
                Ok(outcome) => {
                    self.block
                        .transactions_set
                        .insert(h.unwrap_or_else(|| txs[idx].hash().clone()));
                    self.block.transactions.push(txs[idx].clone().into());
                    self.block
                        .header
                        .add_transaction_fee(&outcome.receipt.transaction_fee);
                    self.block.receipts.push(outcome.receipt.clone());
                    Ok(outcome.receipt)
                }
                Err(x) => Err(From::from(x)),
            };

            receipts_results.push(result);
            idx += 1;
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
        is_building_block: bool,
    ) -> Result<&Receipt, Error>
    {
        if self.block.transactions_set.contains(&t.hash()) {
            return Err(From::from(TransactionError::AlreadyImported));
        }

        let mut result = Vec::new();
        let env_info = self.env_info();
        debug!(target: "vm", "tx type = {:?}", t.tx_type());

        let aion040fork = self
            .engine
            .machine()
            .params()
            .monetary_policy_update
            .map_or(false, |v| self.block.header().number() >= v);
        if aion040fork {
            if t.tx_type() == AVM_TRANSACTION_TYPE || is_normal_or_avm_call(self, &t) {
                result.append(&mut self.block.state.apply_batch(
                    &env_info,
                    self.engine.machine(),
                    &[t.clone()],
                    is_building_block,
                ));
            } else {
                result.push(self.block.state.apply(
                    &env_info,
                    self.engine.machine(),
                    &t,
                    is_building_block,
                ));
            }
        } else {
            result.push(self.block.state.apply(
                &env_info,
                self.engine.machine(),
                &t,
                is_building_block,
            ));
        }

        match result.pop().unwrap() {
            Ok(outcome) => {
                self.block
                    .transactions_set
                    .insert(h.unwrap_or_else(|| t.hash().clone()));
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
            unclosed_state,
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

    pub fn pre_seal(self, seal: Vec<Bytes>) -> Self {
        let mut s = self;
        s.block.header.set_seal(seal);
        s
    }

    /// Given an engine reference, reopen the `ClosedBlock` into an `OpenBlock`.
    pub fn reopen(self, engine: &Engine) -> OpenBlock {
        // revert rewards (i.e. set state back at last transaction's state).
        let mut block = self.block;
        block.state = self.unclosed_state;
        OpenBlock {
            block,
            engine,
        }
    }
}

impl LockedBlock {
    /// Get the hash of the header without seal arguments.
    pub fn hash(&self) -> H256 { self.header().rlp_blake2b(Seal::Without) }

    /// Provide a valid seal in order to turn this into a `SealedBlock`.
    ///
    /// NOTE: This does not check the validity of `seal` with the engine.
    pub fn seal(self, engine: &Engine, seal: Vec<Bytes>) -> Result<SealedBlock, BlockError> {
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

    /// Provide a valid PoW seal in order to turn this into a `SealedBlock`.
    /// This does check the validity of `seal` with the engine.
    /// Returns the `LockedBlock` back again if the seal is no good.
    pub fn try_seal_pow(
        self,
        engine: &Engine,
        seal: Vec<Bytes>,
    ) -> Result<SealedBlock, (Error, LockedBlock)>
    {
        let mut s = self;
        s.block.header.set_seal(seal);

        match engine.verify_local_seal_pow(&s.block.header) {
            Err(e) => Err((e, s)),
            _ => {
                Ok(SealedBlock {
                    block: s.block,
                })
            }
        }
    }

    /// Provide a valid PoS seal in order to turn this into a `SealedBlock`.
    /// This does check the validity of `seal` with the engine.
    /// Returns the `LockedBlock` back again if the seal is no good.
    pub fn try_seal_pos(
        self,
        engine: &Engine,
        seal: Vec<Bytes>,
        parent: &Header,
        grand_parent: Option<&Header>,
        stake: Option<BigUint>,
    ) -> Result<SealedBlock, (Error, LockedBlock)>
    {
        let mut s = self;
        s.block.header.set_seal(seal);

        match engine.verify_seal_pos(&s.block.header, parent, grand_parent, stake) {
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
fn enact(
    header: &Header,
    transactions: &[SignedTransaction],
    engine: &Engine,
    db: StateDB,
    parent: &Header,
    grand_parent: Option<&Header>,
    great_grand_parent: Option<&Header>,
    last_hashes: Arc<LastHashes>,
    factories: Factories,
    kvdb: Arc<KeyValueDB>,
    client: &BlockChainClient,
) -> Result<LockedBlock, Error>
{
    {
        if ::log::max_log_level() >= ::log::LogLevel::Trace {
            let s = State::from_existing(
                db.boxed_clone(),
                parent.state_root().clone(),
                engine.machine().account_start_nonce(parent.number() + 1),
                factories.clone(),
                kvdb.clone(),
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
        header.seal_type().to_owned().unwrap_or_default(),
        grand_parent,
        great_grand_parent,
        last_hashes,
        Address::new(),
        (3141562.into(), 31415620.into()),
        vec![],
        kvdb,
        None,
        client,
    )?;

    b.populate_from(header);
    b.push_transactions(transactions)?;
    Ok(b.close_and_lock())
}

#[inline]
fn is_normal_or_avm_call(block: &mut OpenBlock, tx: &SignedTransaction) -> bool {
    if let Action::Call(a) = tx.action {
        // since fastvm is executed one transaction after another,
        // code() gets the real code of contract
        // for avm: when creation and call are in one block
        // code() will return None. However avm solves the dependency,
        // call will be executed after creation, and code is retrieved by avm callback
        let code = block.block.state.code(&a).unwrap_or(None);
        if a == H256::from("0000000000000000000000000000000000000000000000000000000000000100")
            || a == H256::from("0000000000000000000000000000000000000000000000000000000000000200")
        {
            return false;
        } else {
            // not builtin call
            if let Some(c) = code {
                debug!(target: "vm", "pre bytes = {:?}", &c[0..2]);
                // fastvm contract in database must have header 0x60,0x50
                return c[0..2] != [0x60u8, 0x50];
            }
        }
    }

    // fastvm creation and call a contract with empty code

    return false;
}

#[inline]
fn is_for_avm(block: &mut OpenBlock, tx: &SignedTransaction) -> bool {
    // AVM creation = 0x02; normal call = 0x01
    return tx.tx_type() == AVM_TRANSACTION_TYPE || is_normal_or_avm_call(block, tx);
}

#[inline]
fn push_transactions(
    block: &mut OpenBlock,
    transactions: &[SignedTransaction],
) -> Result<(), Error>
{
    let aion040fork = block
        .engine
        .machine()
        .params()
        .monetary_policy_update
        .map_or(false, |v| block.block.header().number() >= v);

    if aion040fork {
        let mut tx_batch = Vec::new();
        debug!(target: "vm", "transactions = {:?}, len = {:?}", transactions, transactions.len());
        for tx in transactions {
            if !is_for_avm(block, tx) {
                if tx_batch.len() >= 1 {
                    block.apply_batch_txs(tx_batch.as_slice(), None, false);
                    tx_batch.clear();
                }
                block.push_transaction(tx.clone(), None, false)?;
            } else {
                trace!(target: "vm", "found avm transaction");
                tx_batch.push(tx.clone())
            }
        }

        if !tx_batch.is_empty() {
            block.apply_batch_txs(tx_batch.as_slice(), None, false);
            tx_batch.clear();
        }
    } else {
        for t in transactions {
            block.push_transaction(t.clone(), None, false)?;
        }
    }

    trace!(target: "vm", "push transactions done");

    Ok(())
}

// TODO [ToDr] Pass `PreverifiedBlock` by move, this will avoid unecessary allocation
/// Enact the block given by `block_bytes` using `engine` on the database `db` with given `parent` block header
pub fn enact_verified(
    block: &PreverifiedBlock,
    engine: &Engine,
    db: StateDB,
    parent: &Header,
    grand_parent: Option<&Header>,
    great_grand_parent: Option<&Header>,
    last_hashes: Arc<LastHashes>,
    factories: Factories,
    kvdb: Arc<KeyValueDB>,
    client: &BlockChainClient,
) -> Result<LockedBlock, Error>
{
    enact(
        &block.header,
        &block.transactions,
        engine,
        db,
        parent,
        grand_parent,
        great_grand_parent,
        last_hashes,
        factories,
        kvdb,
        client,
    )
}
