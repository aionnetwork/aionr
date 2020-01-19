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

use aion_types::{H256, U256};
use crate::account_provider::AccountProvider;
use crate::block::{OpenBlock, Drain};
use crate::blockchain::{BlockChain};
use crate::types::blockchain::config::Config as BlockChainConfig;
use acore_bytes::Bytes;
use crate::client::{BlockChainClient, ChainNotify, Client, ClientConfig};
use key::{Ed25519Secret, Ed25519KeyPair};
use crate::header::{Header, SealType};
use crate::transaction::{Action, Transaction, SignedTransaction, DEFAULT_TRANSACTION_TYPE};
use crate::views::BlockView;
use io::*;
use crate::miner::{Miner, MinerService};
use parking_lot::RwLock;
use rlp::{self, RlpStream};
use crate::spec::*;
use crate::db::{self,StateDB};
use crate::state::*;
use std::sync::Arc;
use kvdb::{MockDbRepository, DBTransaction, KeyValueDB};
use crate::machine::EthereumMachine;
use std::collections::BTreeMap;
use crate::tests::common::TestBlockChainClient;

#[allow(unused)]
pub fn tests_logger_init() {
    use log::{SetLoggerError,LogLevelFilter,LogMetadata,LogLevel,LogRecord};
    struct SimpleLogger;

    impl log::Log for SimpleLogger {
        fn enabled(&self, metadata: &LogMetadata) -> bool { metadata.level() <= LogLevel::Trace }

        fn log(&self, record: &LogRecord) {
            if self.enabled(record.metadata()) {
                println!("{} - {}", record.level(), record.args());
            }
        }
    }
    pub fn init() -> Result<(), SetLoggerError> {
        ::log::set_logger(|max_log_level| {
            max_log_level.set(LogLevelFilter::Trace);
            Box::new(SimpleLogger)
        })
    }
    init().unwrap()
}

pub fn make_aion_machine() -> EthereumMachine {
    EthereumMachine::regular(Default::default(), BTreeMap::new(), U256::zero())
}

// TODO: move everything over to get_null_spec.
pub fn get_test_spec() -> Spec { Spec::new_test() }

pub fn create_test_block(header: &Header) -> Bytes {
    let mut rlp = RlpStream::new_list(3);
    rlp.append(header);
    rlp.append_raw(&rlp::EMPTY_LIST_RLP, 1);
    rlp.append_raw(&rlp::EMPTY_LIST_RLP, 1);
    rlp.out()
}

fn create_unverifiable_block_header(order: u32, parent_hash: H256) -> Header {
    let mut header = Header::new();
    header.set_gas_limit(0.into());
    header.set_difficulty((order * 100).into());
    header.set_timestamp((order * 10) as u64);
    header.set_number(order as u64);
    header.set_parent_hash(parent_hash);
    header.set_state_root(H256::zero());

    header
}

fn create_unverifiable_block_with_extra(
    order: u32,
    parent_hash: H256,
    extra: Option<Bytes>,
) -> Bytes
{
    let mut header = create_unverifiable_block_header(order, parent_hash);
    header.set_extra_data(match extra {
        Some(extra_data) => extra_data,
        None => {
            let base = (order & 0x000000ff) as u8;
            let generated: Vec<u8> = vec![base + 1, base + 2, base + 3];
            generated
        }
    });
    create_test_block(&header)
}

fn create_unverifiable_pos_block(order: u32, parent_hash: H256) -> Bytes {
    let mut header = create_unverifiable_block_header(order, parent_hash);
    header.set_seal_type(SealType::PoS);
    create_test_block(&header)
}

fn create_unverifiable_block(order: u32, parent_hash: H256) -> Bytes {
    create_test_block(&create_unverifiable_block_header(order, parent_hash))
}

pub fn create_test_block_with_data(header: &Header, transactions: &[SignedTransaction]) -> Bytes {
    let mut rlp = RlpStream::new_list(2);
    rlp.append(header);
    rlp.begin_list(transactions.len());
    for t in transactions {
        rlp.append_raw(&rlp::encode(t).into_vec(), 1);
    }
    rlp.out()
}

pub fn generate_dummy_client(block_number: u32) -> Arc<Client> {
    generate_dummy_client_with_spec_and_data(Spec::new_test, block_number, 0, &[])
}

pub fn generate_dummy_client_with_data(
    block_number: u32,
    txs_per_block: usize,
    tx_gas_prices: &[U256],
) -> Arc<Client>
{
    generate_dummy_client_with_spec_and_data(
        Spec::new_null,
        block_number,
        txs_per_block,
        tx_gas_prices,
    )
}

pub fn generate_dummy_client_with_spec_and_data<F>(
    get_test_spec: F,
    block_number: u32,
    txs_per_block: usize,
    tx_gas_prices: &[U256],
) -> Arc<Client>
where
    F: Fn() -> Spec,
{
    generate_dummy_client_with_spec_accounts_and_data(
        get_test_spec,
        None,
        block_number,
        txs_per_block,
        tx_gas_prices,
        None,
    )
}
/*

pub fn generate_dummy_unity_client_with_beacon(block_number: u32) -> Arc<Client> {
    generate_dummy_client_with_spec_accounts_and_data(
        Spec::new_unity,
        None,
        block_number,
        1,
        &[0u64.into()],
            Some(1235661u64.into())
    )
}
*/

pub fn generate_dummy_client_with_spec_accounts_and_data<F>(
    get_test_spec: F,
    accounts: Option<Arc<AccountProvider>>,
    block_number: u32,
    txs_per_block: usize,
    tx_gas_prices: &[U256],
    beacon: Option<H256>,
) -> Arc<Client>
where
    F: Fn() -> Spec,
{
    let test_spec = get_test_spec();
    let client_db = new_db();

    let mut miner: Miner = Miner::with_spec_and_accounts(&test_spec, accounts);
    miner.set_minimal_gas_price(0.into());

    let client = Client::new(
        ClientConfig::default(),
        &test_spec,
        client_db,
        Arc::new(miner),
        IoChannel::disconnected(),
    )
    .unwrap();
    let dummy_client = TestBlockChainClient::new_with_spec(test_spec.clone());
    let test_engine = &*test_spec.engine;

    let mut db = test_spec
        .ensure_db_good(get_temp_state_db(), &Default::default())
        .unwrap();
    let genesis_header = test_spec.genesis_header();

    let mut rolling_timestamp = 40;
    let mut last_hashes = vec![];
    let mut last_header = genesis_header.clone();
    let mut second_last_header = Header::new();
    let secret = Ed25519Secret::from("7ea8af7d0982509cd815096d35bc3a295f57b2a078e4e25731e3ea977b9544626702b86f33072a55f46003b1e3e242eb18556be54c5ab12044c3c20829e0abb5");
    let kp = Ed25519KeyPair::from_secret(secret).unwrap();
    let author = kp.address();

    let mut n = 0;
    for i in 0..block_number {
        last_hashes.push(last_header.hash());

        // forge block.
        let mut b = OpenBlock::new(
            test_engine,
            Default::default(),
            db,
            &last_header,
            Default::default(),
            Some(&last_header),
            if i == 0 {
                None
            } else {
                Some(&second_last_header)
            },
            Arc::new(last_hashes.clone()),
            author.clone(),
            (3141562.into(), 31415620.into()),
            vec![],
            Arc::new(MockDbRepository::init(vec![String::new()])),
            None,
            &dummy_client,
        )
        .unwrap();
        b.set_difficulty(U256::from(0x20000));
        rolling_timestamp += 10;
        b.set_timestamp(rolling_timestamp);

        // first block we don't have any balance, so can't send any transactions.
        for _ in 0..txs_per_block {
            b.push_transaction(
                Transaction {
                    nonce: n.into(),
                    gas_price: tx_gas_prices[n % tx_gas_prices.len()],
                    gas: 100000.into(),
                    action: Action::Create,
                    data: vec![],
                    value: U256::zero(),
                    transaction_type: DEFAULT_TRANSACTION_TYPE,
                    nonce_bytes: Vec::new(),
                    gas_price_bytes: Vec::new(),
                    gas_bytes: Vec::new(),
                    value_bytes: Vec::new(),
                    beacon,
                }
                .sign(kp.secret()),
                None,
                true,
            )
            .unwrap();
            n += 1;
        }

        let b = b.close_and_lock().seal(test_engine, vec![]).unwrap();

        if let Err(e) = client.import_block(b.rlp_bytes()) {
            panic!(
                "error importing block which is valid by definition: {:?}",
                e
            );
        }
        second_last_header = last_header.clone();
        last_header = BlockView::new(&b.rlp_bytes()).header();
        db = b.drain();
    }
    client.flush_queue();
    client.import_verified_blocks();
    client
}

pub fn push_blocks_to_client(
    client: &Arc<Client>,
    timestamp_salt: u64,
    starting_number: usize,
    block_number: usize,
)
{
    let test_spec = get_test_spec();
    let state_root = test_spec.genesis_header().state_root().clone();
    let genesis_gas = test_spec.genesis_header().gas_limit().clone();

    let mut rolling_hash = client.chain_info().best_block_hash;
    let mut rolling_block_number = starting_number as u64;
    let mut rolling_timestamp = timestamp_salt + starting_number as u64 * 10;

    for _ in 0..block_number {
        let mut header = Header::new();

        header.set_gas_limit(genesis_gas);
        header.set_difficulty(U256::from(0x20000));
        header.set_timestamp(rolling_timestamp);
        header.set_number(rolling_block_number);
        header.set_parent_hash(rolling_hash);
        header.set_state_root(state_root);

        rolling_hash = header.hash();
        rolling_block_number = rolling_block_number + 1;
        rolling_timestamp = rolling_timestamp + 10;

        if let Err(e) = client.import_block(create_test_block(&header)) {
            panic!(
                "error importing block which is valid by definition: {:?}",
                e
            );
        }
    }
}

pub fn get_test_client_with_blocks(blocks: Vec<Bytes>) -> Arc<Client> {
    let test_spec = get_test_spec();
    let client_db = new_db();

    let client = Client::new(
        ClientConfig::default(),
        &test_spec,
        client_db,
        Arc::new(Miner::with_spec(&test_spec)),
        IoChannel::disconnected(),
    )
    .unwrap();

    for block in blocks {
        if let Err(e) = client.import_block(block) {
            panic!("error importing block which is well-formed: {:?}", e);
        }
    }
    client.flush_queue();
    client.import_verified_blocks();
    client
}

pub fn new_db() -> Arc<dyn KeyValueDB> {
    let mut db_configs = Vec::new();
    for db_name in db::DB_NAMES.to_vec() {
        db_configs.push(db_name.into());
    }
    Arc::new(MockDbRepository::init(db_configs))
}

pub fn generate_dummy_blockchain(block_number: u32) -> BlockChain {
    let db = new_db();
    let bc = BlockChain::new(
        BlockChainConfig::default(),
        &create_unverifiable_block(0, H256::zero()),
        db.clone(),
    );

    let mut batch = DBTransaction::new();
    for block_order in 1..block_number {
        bc.insert_block(
            &mut batch,
            &create_unverifiable_block(block_order, bc.best_block_hash()),
            vec![],
        );
        bc.commit();
    }
    db.write(batch).unwrap();
    bc
}

pub fn generate_dummy_blockchain_with_db(block_number: u32, db: Arc<dyn KeyValueDB>) -> BlockChain {
    let bc = BlockChain::new(
        BlockChainConfig::default(),
        &create_unverifiable_block(0, H256::zero()),
        db.clone(),
    );

    let mut batch = DBTransaction::new();
    for block_order in 1..block_number {
        // which means pow and pos blocks alternately insert every two
        // it will be like pow pow pos pos pow pow pos pos ...
        //                 0   1   2   3   4   5   6   7
        let next_block = match block_order / 2 % 2 == 0 {
            true => create_unverifiable_block(block_order, bc.best_block_hash()),
            false => create_unverifiable_pos_block(block_order, bc.best_block_hash()),
        };
        bc.insert_block(&mut batch, &next_block, vec![]);
        bc.commit();
    }
    db.write(batch).unwrap();
    bc
}

pub fn generate_dummy_blockchain_with_extra(block_number: u32) -> BlockChain {
    let db = new_db();
    let bc = BlockChain::new(
        BlockChainConfig::default(),
        &create_unverifiable_block(0, H256::zero()),
        db.clone(),
    );

    let mut batch = DBTransaction::new();
    for block_order in 1..block_number {
        bc.insert_block(
            &mut batch,
            &create_unverifiable_block_with_extra(block_order, bc.best_block_hash(), None),
            vec![],
        );
        bc.commit();
    }
    db.write(batch).unwrap();
    bc
}

pub fn generate_dummy_blockchain_with_pos_block(block_number: u32) -> BlockChain {
    let db = new_db();
    let bc = BlockChain::new(
        BlockChainConfig::default(),
        &create_unverifiable_block(0, H256::zero()),
        db.clone(),
    );

    let mut batch = DBTransaction::new();
    for block_order in 1..block_number {
        // which means pow and pos blocks are inserted alternately
        // it will be like pow pow pos pow pos pow pos pow ...
        //                 0   1   2   3   4   5   6   7
        let next_block = match block_order % 2 == 1 {
            true => create_unverifiable_block(block_order, bc.best_block_hash()),
            false => create_unverifiable_pos_block(block_order, bc.best_block_hash()),
        };
        bc.insert_block(&mut batch, &next_block, vec![]);
        bc.commit();
    }
    db.write(batch).unwrap();
    bc
}

pub fn generate_dummy_empty_blockchain() -> BlockChain {
    let db = new_db();
    let bc = BlockChain::new(
        BlockChainConfig::default(),
        &create_unverifiable_block(0, H256::zero()),
        db.clone(),
    );
    bc
}

pub fn get_temp_state() -> State<StateDB> {
    let journal_db = get_temp_state_db();
    State::new(
        journal_db,
        U256::from(0),
        Default::default(),
        Arc::new(MockDbRepository::init(vec![String::from(
            db::COL_AVM_GRAPH,
        )])),
    )
}

pub fn get_temp_state_with_nonce() -> State<StateDB> {
    let journal_db = get_temp_state_db();
    State::new(
        journal_db,
        U256::from(1),
        Default::default(),
        Arc::new(MockDbRepository::init(vec![String::new()])),
    )
}

pub fn get_temp_state_db() -> StateDB {
    let db = new_db();
    let journal_db = ::journaldb::new(db, ::journaldb::Algorithm::OverlayRecent, db::COL_STATE);
    StateDB::new(journal_db, 5 * 1024 * 1024)
}

pub fn get_good_dummy_block_seq(count: usize) -> Vec<Bytes> {
    let test_spec = get_test_spec();
    get_good_dummy_block_fork_seq(1, count, &test_spec.genesis_header().hash())
}

pub fn get_good_dummy_block_fork_seq(
    start_number: usize,
    count: usize,
    parent_hash: &H256,
) -> Vec<Bytes>
{
    let test_spec = get_test_spec();
    let genesis_gas = test_spec.genesis_header().gas_limit().clone();
    let mut rolling_timestamp = start_number as u64 * 10;
    let mut parent = *parent_hash;
    let mut r = Vec::new();
    for i in start_number..start_number + count + 1 {
        let mut block_header = Header::new();
        block_header.set_gas_limit(genesis_gas);
        block_header.set_difficulty(U256::from(i) * U256([0, 1, 0, 0]));
        block_header.set_timestamp(rolling_timestamp);
        block_header.set_number(i as u64);
        block_header.set_parent_hash(parent);
        block_header.set_state_root(test_spec.genesis_header().state_root().clone());

        parent = block_header.hash();
        rolling_timestamp = rolling_timestamp + 10;

        r.push(create_test_block(&block_header));
    }
    r
}

pub fn get_good_dummy_block_hash(seal_type: Option<SealType>) -> (H256, Bytes) {
    let mut block_header = Header::new();
    let test_spec = get_test_spec();
    let genesis_gas = test_spec.genesis_header().gas_limit().clone();
    block_header.set_gas_limit(genesis_gas);
    block_header.set_difficulty(U256::from(0x20000));
    block_header.set_timestamp(40);
    block_header.set_number(1);
    block_header.set_parent_hash(test_spec.genesis_header().hash());
    block_header.set_state_root(test_spec.genesis_header().state_root().clone());
    block_header.set_seal_type(seal_type.unwrap_or_default());
    // block_header.set_seal(vec![vec![0u8;64], vec![0u8;64], vec![0u8;32]]);

    (block_header.hash(), create_test_block(&block_header))
}

pub fn get_good_dummy_block() -> Bytes {
    let (_, bytes) = get_good_dummy_block_hash(None);
    bytes
}

pub fn get_good_dummy_pos_block() -> Bytes {
    let (_, bytes) = get_good_dummy_block_hash(Some(SealType::PoS));
    bytes
}

pub fn get_bad_state_dummy_block() -> Bytes {
    let mut block_header = Header::new();
    let test_spec = get_test_spec();
    let genesis_gas = test_spec.genesis_header().gas_limit().clone();

    block_header.set_gas_limit(genesis_gas);
    block_header.set_difficulty(U256::from(0x20000));
    block_header.set_timestamp(40);
    block_header.set_number(1);
    block_header.set_parent_hash(test_spec.genesis_header().hash());
    block_header.set_state_root(0xbad.into());

    create_test_block(&block_header)
}

#[derive(Default)]
pub struct TestNotify {
    pub messages: RwLock<Vec<Bytes>>,
}

impl ChainNotify for TestNotify {
    fn new_blocks(
        &self,
        _imported: Vec<H256>,
        _invalid: Vec<H256>,
        _enacted: Vec<H256>,
        _retracted: Vec<H256>,
        _sealed: Vec<H256>,
        // Block bytes.
        _proposed: Vec<Bytes>,
        _duration: u64,
    )
    {
        unimplemented!()
    }
    fn broadcast(&self, data: Vec<u8>) { self.messages.write().push(data); }
}
