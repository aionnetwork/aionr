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

//! Block and transaction verification functions
//!
//! Block verification is done in 3 steps
//! 1. Quick verification upon adding to the block queue
//! 2. Signatures verification done in the queue.
//! 3. Final verification against the blockchain done before enactment.

pub mod queue;

use acore_bytes::Bytes;
use aion_types::{H256, U256};
use heapsize::HeapSizeOf;
use rlp::UntrustedRlp;
use time::get_time;
use triehash::ordered_trie_root;
use unexpected::{Mismatch, OutOfBounds};

use blockchain::*;
use client::BlockChainClient;
use engine::Engine;
use types::error::{BlockError, Error};
use header::{BlockNumber, Header};
use transaction::{SignedTransaction, UnverifiedTransaction};
use views::BlockView;

/// Preprocessed block data gathered in `verify_block_unordered` call
pub struct PreverifiedBlock {
    /// Populated block header
    pub header: Header,
    /// Populated block transactions
    pub transactions: Vec<SignedTransaction>,
    /// Block bytes
    pub bytes: Bytes,
}

impl HeapSizeOf for PreverifiedBlock {
    fn heap_size_of_children(&self) -> usize {
        self.header.heap_size_of_children()
            + self.transactions.heap_size_of_children()
            + self.bytes.heap_size_of_children()
    }
}

/// Phase 1 quick block verification. Only does checks that are cheap. Operates on a single block
pub fn verify_block_basic(header: &Header, bytes: &[u8], engine: &Engine) -> Result<(), Error> {
    verify_header_params(&header, engine, true)?;
    verify_block_integrity(bytes, &header.transactions_root())?;
    engine.verify_block_basic(&header)?;

    for t in UntrustedRlp::new(bytes)
        .at(1)?
        .iter()
        .map(|rlp| rlp.as_val::<UnverifiedTransaction>())
    {
        engine.verify_transaction_basic(&t?)?;
    }
    Ok(())
}

/// Phase 2 verification. Perform costly checks such as transaction signatures and block nonce for ethash.
/// Still operates on a individual block
/// Returns a `PreverifiedBlock` structure populated with transactions
pub fn verify_block_unordered(
    header: Header,
    bytes: Bytes,
    engine: &Engine,
) -> Result<PreverifiedBlock, Error>
{
    engine.verify_block_unordered(&header)?;
    let mut transactions = Vec::new();
    {
        let v = BlockView::new(&bytes);
        for t in v.transactions() {
            let t = engine.verify_transaction_signature(t, &header)?;
            transactions.push(t);
        }
    }
    Ok(PreverifiedBlock {
        header,
        transactions,
        bytes,
    })
}

/// Parameters for full verification of block family: block bytes, transactions, blockchain, and state access.
pub type FullFamilyParams<'a> = (
    &'a [u8],
    &'a [SignedTransaction],
    &'a BlockProvider,
    &'a BlockChainClient,
);

/// Phase 3 verification. Check block information against parent and uncles.
pub fn verify_block_family(
    header: &Header,
    parent: &Header,
    seal_parent: Option<&Header>,
    seal_grand_parent: Option<&Header>,
    engine: &Engine,
    do_full: Option<FullFamilyParams>,
) -> Result<(), Error>
{
    verify_parent(
        &header,
        &parent,
        engine.machine().params().gas_limit_bound_divisor,
    )?;
    engine.verify_block_family(&header, &parent, seal_parent, seal_grand_parent)?;

    let (_bytes, _txs, _bc, _client) = match do_full {
        Some(x) => x,
        None => return Ok(()),
    };

    Ok(())
}

/// Phase 4 verification. Check block information against transaction enactment results,
pub fn verify_block_final(expected: &Header, got: &Header) -> Result<(), Error> {
    if expected.gas_used() != got.gas_used() {
        return Err(From::from(BlockError::InvalidGasUsed(Mismatch {
            expected: expected.gas_used().clone(),
            found: got.gas_used().clone(),
        })));
    }
    if expected.log_bloom() != got.log_bloom() {
        return Err(From::from(BlockError::InvalidLogBloom(Mismatch {
            expected: expected.log_bloom().clone(),
            found: got.log_bloom().clone(),
        })));
    }
    if expected.state_root() != got.state_root() {
        return Err(From::from(BlockError::InvalidStateRoot(Mismatch {
            expected: expected.state_root().clone(),
            found: got.state_root().clone(),
        })));
    }
    if expected.receipts_root() != got.receipts_root() {
        return Err(From::from(BlockError::InvalidReceiptsRoot(Mismatch {
            expected: expected.receipts_root().clone(),
            found: got.receipts_root().clone(),
        })));
    }
    Ok(())
}

/// Check basic header parameters.
pub fn verify_header_params(header: &Header, engine: &Engine, is_full: bool) -> Result<(), Error> {
    let expected_seal_fields = engine.seal_fields(header);
    if header.seal().len() != expected_seal_fields {
        return Err(From::from(BlockError::InvalidSealArity(Mismatch {
            expected: expected_seal_fields,
            found: header.seal().len(),
        })));
    }

    if header.number() >= From::from(BlockNumber::max_value()) {
        return Err(From::from(BlockError::RidiculousNumber(OutOfBounds {
            max: Some(From::from(BlockNumber::max_value())),
            min: None,
            found: header.number(),
        })));
    }
    if header.gas_used() > header.gas_limit() {
        return Err(From::from(BlockError::TooMuchGasUsed(OutOfBounds {
            max: Some(header.gas_limit().clone()),
            min: None,
            found: header.gas_used().clone(),
        })));
    }
    let min_gas_limit = engine.params().min_gas_limit;
    if header.gas_limit() < &min_gas_limit {
        return Err(From::from(BlockError::InvalidGasLimit(OutOfBounds {
            min: Some(min_gas_limit),
            max: None,
            found: header.gas_limit().clone(),
        })));
    }
    let maximum_extra_data_size = engine.maximum_extra_data_size();
    if header.number() != 0 && header.extra_data().len() > maximum_extra_data_size {
        return Err(From::from(BlockError::ExtraDataOutOfBounds(OutOfBounds {
            min: None,
            max: Some(maximum_extra_data_size),
            found: header.extra_data().len(),
        })));
    }

    if is_full {
        const ACCEPTABLE_DRIFT_SECS: u64 = 15;
        let max_time = get_time().sec as u64 + ACCEPTABLE_DRIFT_SECS;
        let invalid_threshold = max_time + ACCEPTABLE_DRIFT_SECS * 9;
        let timestamp = header.timestamp();

        if timestamp > invalid_threshold {
            return Err(From::from(BlockError::InvalidTimestamp(OutOfBounds {
                max: Some(max_time),
                min: None,
                found: timestamp,
            })));
        }

        if timestamp > max_time {
            return Err(From::from(BlockError::TemporarilyInvalid(OutOfBounds {
                max: Some(max_time),
                min: None,
                found: timestamp,
            })));
        }
    }

    Ok(())
}

/// Check header parameters agains parent header.
fn verify_parent(header: &Header, parent: &Header, gas_limit_divisor: U256) -> Result<(), Error> {
    if !header.parent_hash().is_zero() && &parent.hash() != header.parent_hash() {
        return Err(From::from(BlockError::InvalidParentHash(Mismatch {
            expected: parent.hash(),
            found: header.parent_hash().clone(),
        })));
    }
    if header.timestamp() <= parent.timestamp() {
        return Err(From::from(BlockError::InvalidTimestamp(OutOfBounds {
            max: None,
            min: Some(parent.timestamp() + 1),
            found: header.timestamp(),
        })));
    }
    if header.number() != parent.number() + 1 {
        return Err(From::from(BlockError::InvalidNumber(Mismatch {
            expected: parent.number() + 1,
            found: header.number(),
        })));
    }

    if header.number() == 0 {
        return Err(BlockError::RidiculousNumber(OutOfBounds {
            min: Some(1),
            max: None,
            found: header.number(),
        })
        .into());
    }

    let parent_gas_limit = *parent.gas_limit();
    let min_gas = parent_gas_limit - parent_gas_limit / gas_limit_divisor;
    let max_gas = parent_gas_limit + parent_gas_limit / gas_limit_divisor;
    if header.gas_limit() < &min_gas || header.gas_limit() > &max_gas {
        return Err(From::from(BlockError::InvalidGasLimit(OutOfBounds {
            min: Some(min_gas),
            max: Some(max_gas),
            found: header.gas_limit().clone(),
        })));
    }

    Ok(())
}

/// Verify block data against header: transactions root and uncles hash.
fn verify_block_integrity(block: &[u8], transactions_root: &H256) -> Result<(), Error> {
    let block = UntrustedRlp::new(block);
    let tx = block.at(1)?;
    let expected_root = &ordered_trie_root(tx.iter().map(|r| r.as_raw()));
    if expected_root != transactions_root {
        return Err(From::from(BlockError::InvalidTransactionsRoot(Mismatch {
            expected: expected_root.clone(),
            found: transactions_root.clone(),
        })));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;
    use aion_types::H256;
    use ethbloom::Bloom;
    use types::blockchain::extra::{BlockDetails, TransactionAddress, BlockReceipts};
    use encoded;
    use types::error::BlockError::*;
    use spec::Spec;
    use helpers::{create_test_block_with_data, create_test_block};
    use transaction::{SignedTransaction, Transaction, UnverifiedTransaction, Action};
    use types::state::log_entry::{LogEntry, LocalizedLogEntry};
    use rlp;
    use header::SealType;
    use keychain;

    fn check_ok(result: Result<(), Error>) {
        result.unwrap_or_else(|e| panic!("Block verification failed: {:?}", e));
    }

    fn check_fail(result: Result<(), Error>, e: BlockError) {
        match result {
            Err(Error::Block(ref error)) if *error == e => (),
            Err(other) => {
                panic!(
                    "Block verification failed.\nExpected: {:?}\nGot: {:?}",
                    e, other
                )
            }
            Ok(_) => panic!("Block verification failed.\nExpected: {:?}\nGot: Ok", e),
        }
    }

    fn check_fail_timestamp(result: Result<(), Error>, temp: bool) {
        let name = if temp {
            "TemporarilyInvalid"
        } else {
            "InvalidTimestamp"
        };
        match result {
            Err(Error::Block(BlockError::InvalidTimestamp(_))) if !temp => (),
            Err(Error::Block(BlockError::TemporarilyInvalid(_))) if temp => (),
            Err(other) => {
                panic!(
                    "Block verification failed.\nExpected: {}\nGot: {:?}",
                    name, other
                )
            }
            Ok(_) => panic!("Block verification failed.\nExpected: {}\nGot: Ok", name),
        }
    }

    struct TestBlockChain {
        blocks: HashMap<H256, Bytes>,
        numbers: HashMap<BlockNumber, H256>,
    }

    impl Default for TestBlockChain {
        fn default() -> Self { TestBlockChain::new() }
    }

    impl TestBlockChain {
        pub fn new() -> Self {
            TestBlockChain {
                blocks: HashMap::new(),
                numbers: HashMap::new(),
            }
        }

        pub fn insert(&mut self, bytes: Bytes) {
            let number = BlockView::new(&bytes).header_view().number();
            let hash = BlockView::new(&bytes).header_view().hash();
            self.blocks.insert(hash.clone(), bytes);
            self.numbers.insert(number, hash.clone());
        }
    }

    impl BlockProvider for TestBlockChain {
        fn is_known(&self, hash: &H256) -> bool { self.blocks.contains_key(hash) }

        fn first_block(&self) -> Option<H256> { unimplemented!() }

        /// Get raw block data
        fn block(&self, hash: &H256) -> Option<encoded::Block> {
            self.blocks.get(hash).cloned().map(encoded::Block::new)
        }

        fn block_header_data(&self, hash: &H256) -> Option<encoded::Header> {
            self.block(hash)
                .map(|b| b.header_view().rlp().as_raw().to_vec())
                .map(encoded::Header::new)
        }

        /// Get the header RLP of the seal parent of the given block.
        /// Parameters:
        ///   parent_hash: parent hash of the given block
        ///   seal_type: seal type of the given block
        fn seal_parent_header(
            &self,
            parent_hash: &H256,
            seal_type: &Option<SealType>,
        ) -> Option<::encoded::Header>
        {
            // Get parent header
            let parent_header: ::encoded::Header = match self.block_header_data(parent_hash) {
                Some(header) => header,
                None => return None,
            };
            let parent_seal_type: Option<SealType> = parent_header.seal_type();
            // If parent's seal type is the same as the current, return parent
            if seal_type == &parent_seal_type {
                Some(parent_header)
            }
            // Else return the anti seal parent of the parent
            else {
                let parent_details: BlockDetails = match self.block_details(parent_hash) {
                    Some(details) => details,
                    None => return None,
                };
                let anti_seal_parent: H256 = parent_details.anti_seal_parent;
                self.block_header_data(&anti_seal_parent)
            }
        }

        fn block_body(&self, hash: &H256) -> Option<encoded::Body> {
            self.block(hash)
                .map(|b| BlockChain::block_to_body(&b.into_inner()))
                .map(encoded::Body::new)
        }

        fn best_ancient_block(&self) -> Option<H256> { None }

        /// Get the familial details concerning a block.
        fn block_details(&self, hash: &H256) -> Option<BlockDetails> {
            self.blocks.get(hash).map(|bytes| {
                let header = BlockView::new(bytes).header();
                BlockDetails {
                    number: header.number(),
                    total_difficulty: header.difficulty().clone(),
                    parent: header.parent_hash().clone(),
                    children: Vec::new(),
                    anti_seal_parent: H256::zero(),
                }
            })
        }

        fn transaction_address(&self, _hash: &H256) -> Option<TransactionAddress> {
            unimplemented!()
        }

        /// Get the hash of given block's number.
        fn block_hash(&self, index: BlockNumber) -> Option<H256> {
            self.numbers.get(&index).cloned()
        }

        fn block_receipts(&self, _hash: &H256) -> Option<BlockReceipts> { unimplemented!() }

        fn blocks_with_bloom(
            &self,
            _bloom: &Bloom,
            _from_block: BlockNumber,
            _to_block: BlockNumber,
        ) -> Vec<BlockNumber>
        {
            unimplemented!()
        }

        fn logs<F>(
            &self,
            _blocks: Vec<BlockNumber>,
            _matches: F,
            _limit: Option<usize>,
        ) -> Vec<LocalizedLogEntry>
        where
            F: Fn(&LogEntry) -> bool,
            Self: Sized,
        {
            unimplemented!()
        }
    }

    fn basic_test(bytes: &[u8], engine: &Engine) -> Result<(), Error> {
        let header = BlockView::new(bytes).header();
        verify_block_basic(&header, bytes, engine)
    }

    fn family_test<BC>(bytes: &[u8], engine: &Engine, bc: &BC) -> Result<(), Error>
    where BC: BlockProvider {
        let view = BlockView::new(bytes);
        let header = view.header();
        let transactions: Vec<_> = view
            .transactions()
            .into_iter()
            .map(SignedTransaction::new)
            .collect::<Result<_, _>>()?;

        // TODO: client is really meant to be used for state query here by machine
        // additions that need access to state (tx filter in specific)
        // no existing tests need access to test, so having this not function
        // is fine.
        let client = ::tests::common::TestBlockChainClient::default();

        let parent = bc
            .block_header(header.parent_hash())
            .ok_or(BlockError::UnknownParent(header.parent_hash().clone()))?;

        let full_params: FullFamilyParams = (
            bytes,
            &transactions[..],
            bc as &BlockProvider,
            &client as &::client::BlockChainClient,
        );
        verify_block_family(&header, &parent, None, None, engine, Some(full_params))?;
        Ok(())
    }

    #[test]
    fn test_verify_block_basic_with_invalid_transactions() {
        let spec = Spec::new_test();
        let engine = &*spec.engine;

        let block = {
            let mut rlp = rlp::RlpStream::new_list(3);
            let mut header = Header::default();
            // that's an invalid transaction list rlp
            let invalid_transactions = vec![vec![0u8]];
            header.set_transactions_root(ordered_trie_root(&invalid_transactions));
            header.set_gas_limit(engine.params().min_gas_limit);
            rlp.append(&header);
            rlp.append_list::<Vec<u8>, _>(&invalid_transactions);
            rlp.append_raw(&rlp::EMPTY_LIST_RLP, 1);
            rlp.out()
        };

        assert!(basic_test(&block, engine).is_err());
    }

    #[test]
    fn test_verify_block() {
        // Test against morden
        let mut good = Header::new();
        let spec = Spec::new_test();
        let engine = &*spec.engine;

        let min_gas_limit = engine.params().min_gas_limit;
        good.set_gas_limit(min_gas_limit);
        good.set_timestamp(40);
        good.set_number(10);

        let keypair = keychain::ethkey::generate_keypair();

        let tr1 = Transaction {
            action: Action::Create,
            value: U256::from(0),
            data: Bytes::new(),
            gas: U256::from(300_000),
            gas_price: U256::from(40_000),
            nonce: U256::one(),
            nonce_bytes: Vec::new(),
            gas_bytes: Vec::new(),
            gas_price_bytes: Vec::new(),
            value_bytes: Vec::new(),
            transaction_type: U256::from(1),
        }
        .sign(keypair.secret(), None);

        let tr2 = Transaction {
            action: Action::Create,
            value: U256::from(0),
            data: Bytes::new(),
            gas: U256::from(300_000),
            gas_price: U256::from(40_000),
            nonce: U256::from(2),
            nonce_bytes: Vec::new(),
            gas_bytes: Vec::new(),
            gas_price_bytes: Vec::new(),
            value_bytes: Vec::new(),
            transaction_type: U256::from(1),
        }
        .sign(keypair.secret(), None);

        let good_transactions = [tr1.clone(), tr2.clone()];

        let diff_inc = U256::from(0x40);

        let mut parent6 = good.clone();
        parent6.set_number(6);
        let mut parent7 = good.clone();
        parent7.set_number(7);
        parent7.set_parent_hash(parent6.hash());
        parent7.set_difficulty(parent6.difficulty().clone() + diff_inc);
        parent7.set_timestamp(parent6.timestamp() + 10);
        let mut parent8 = good.clone();
        parent8.set_number(8);
        parent8.set_parent_hash(parent7.hash());
        parent8.set_difficulty(parent7.difficulty().clone() + diff_inc);
        parent8.set_timestamp(parent7.timestamp() + 10);

        let good_transactions_root = ordered_trie_root(
            good_transactions
                .iter()
                .map(|t| ::rlp::encode::<UnverifiedTransaction>(t)),
        );

        let mut parent = good.clone();
        parent.set_number(9);
        parent.set_timestamp(parent8.timestamp() + 10);
        parent.set_parent_hash(parent8.hash());
        parent.set_difficulty(parent8.difficulty().clone() + diff_inc);

        good.set_parent_hash(parent.hash());
        good.set_difficulty(parent.difficulty().clone() + diff_inc);
        good.set_timestamp(parent.timestamp() + 10);

        let mut bc = TestBlockChain::new();
        bc.insert(create_test_block(&good));
        bc.insert(create_test_block(&parent));
        bc.insert(create_test_block(&parent6));
        bc.insert(create_test_block(&parent7));
        bc.insert(create_test_block(&parent8));

        check_ok(basic_test(&create_test_block(&good), engine));

        let mut header = good.clone();
        header.set_transactions_root(good_transactions_root.clone());
        check_ok(basic_test(
            &create_test_block_with_data(&header, &good_transactions),
            engine,
        ));

        header.set_gas_limit(min_gas_limit - From::from(1));
        check_fail(
            basic_test(&create_test_block(&header), engine),
            InvalidGasLimit(OutOfBounds {
                min: Some(min_gas_limit),
                max: None,
                found: header.gas_limit().clone(),
            }),
        );

        header = good.clone();
        header.set_number(BlockNumber::max_value());
        check_fail(
            basic_test(&create_test_block(&header), engine),
            RidiculousNumber(OutOfBounds {
                max: Some(BlockNumber::max_value()),
                min: None,
                found: header.number(),
            }),
        );

        header = good.clone();
        let gas_used = header.gas_limit().clone() + 1.into();
        header.set_gas_used(gas_used);
        check_fail(
            basic_test(&create_test_block(&header), engine),
            TooMuchGasUsed(OutOfBounds {
                max: Some(header.gas_limit().clone()),
                min: None,
                found: header.gas_used().clone(),
            }),
        );

        header = good.clone();
        header
            .extra_data_mut()
            .resize(engine.maximum_extra_data_size() + 1, 0u8);
        check_fail(
            basic_test(&create_test_block(&header), engine),
            ExtraDataOutOfBounds(OutOfBounds {
                max: Some(engine.maximum_extra_data_size()),
                min: None,
                found: header.extra_data().len(),
            }),
        );

        header = good.clone();
        header
            .extra_data_mut()
            .resize(engine.maximum_extra_data_size() + 1, 0u8);
        check_fail(
            basic_test(&create_test_block(&header), engine),
            ExtraDataOutOfBounds(OutOfBounds {
                max: Some(engine.maximum_extra_data_size()),
                min: None,
                found: header.extra_data().len(),
            }),
        );

        header = good.clone();
        check_fail(
            basic_test(
                &create_test_block_with_data(&header, &good_transactions),
                engine,
            ),
            InvalidTransactionsRoot(Mismatch {
                expected: good_transactions_root.clone(),
                found: header.transactions_root().clone(),
            }),
        );

        check_ok(family_test(&create_test_block(&good), engine, &bc));
        check_ok(family_test(
            &create_test_block_with_data(&good, &good_transactions),
            engine,
            &bc,
        ));

        header = good.clone();
        header.set_parent_hash(H256::random());
        check_fail(
            family_test(
                &create_test_block_with_data(&header, &good_transactions),
                engine,
                &bc,
            ),
            UnknownParent(header.parent_hash().clone()),
        );

        header = good.clone();
        header.set_timestamp(10);
        check_fail(
            family_test(
                &create_test_block_with_data(&header, &good_transactions),
                engine,
                &bc,
            ),
            InvalidTimestamp(OutOfBounds {
                max: None,
                min: Some(parent.timestamp() + 1),
                found: header.timestamp(),
            }),
        );

        header = good.clone();
        header.set_timestamp(2450000000);
        check_fail_timestamp(
            basic_test(
                &create_test_block_with_data(&header, &good_transactions),
                engine,
            ),
            false,
        );

        header = good.clone();
        header.set_timestamp(get_time().sec as u64 + 20);
        check_fail_timestamp(
            basic_test(
                &create_test_block_with_data(&header, &good_transactions),
                engine,
            ),
            true,
        );

        header = good.clone();
        header.set_timestamp(get_time().sec as u64 + 10);
        header.set_transactions_root(good_transactions_root.clone());
        check_ok(basic_test(
            &create_test_block_with_data(&header, &good_transactions),
            engine,
        ));

        header = good.clone();
        header.set_number(9);
        check_fail(
            family_test(
                &create_test_block_with_data(&header, &good_transactions),
                engine,
                &bc,
            ),
            InvalidNumber(Mismatch {
                expected: parent.number() + 1,
                found: header.number(),
            }),
        );

        header = good.clone();
        header.set_gas_limit(0.into());
        header.set_difficulty(
            "0000000000000000000000000000000000000000000000000000000000020000"
                .parse::<U256>()
                .unwrap(),
        );
        match family_test(&create_test_block(&header), engine, &bc) {
            Err(Error::Block(InvalidGasLimit(_))) => {}
            Err(_) => {
                panic!("should be invalid difficulty fail");
            }
            _ => {
                panic!("Should be error, got Ok");
            }
        }
    }
}
