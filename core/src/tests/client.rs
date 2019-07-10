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

use std::str::FromStr;
use std::sync::Arc;
use io::IoChannel;
use client::{BlockChainClient, MiningBlockChainClient, Client, ClientConfig, BlockId};
use state::{self, State, CleanupMode};
use executive::Executive;
use block::IsBlock;
use super::*;
use types::filter::Filter;
use aion_types::{Address, U256};
use kvdb::{DatabaseConfig, DbRepository, RepositoryConfig};
use miner::Miner;
use spec::Spec;
use views::BlockView;
use key::Ed25519Secret;
use transaction::{PendingTransaction, Transaction, Action, Condition};
use miner::MinerService;
use tempdir::TempDir;
use kvdb::MemoryDBRepository;

#[test]
fn imports_from_empty() {
    let tempdir = TempDir::new("").unwrap();
    let spec = get_test_spec();
    let db_config = DatabaseConfig::default();
    let mut db_configs = Vec::new();
    for db_name in ::db::DB_NAMES.to_vec() {
        db_configs.push(RepositoryConfig {
            db_name: db_name.into(),
            db_config: db_config.clone(),
            db_path: tempdir.path().join(db_name).to_str().unwrap().to_string(),
        });
    }
    let client_db = Arc::new(DbRepository::init(db_configs).unwrap());

    let client = Client::new(
        ClientConfig::default(),
        &spec,
        client_db,
        Arc::new(Miner::with_spec(&spec)),
        IoChannel::disconnected(),
    )
    .unwrap();
    client.import_verified_blocks();
    client.flush_queue();
}

#[test]
fn returns_state_root_basic() {
    let client = generate_dummy_client(6);
    let test_spec = get_test_spec();
    let genesis_header = test_spec.genesis_header();

    assert!(client.state_data(genesis_header.state_root()).is_some());
}

#[test]
fn imports_good_block() {
    let tempdir = TempDir::new("").unwrap();
    let spec = get_test_spec();
    let db_config = DatabaseConfig::default();
    let mut db_configs = Vec::new();
    for db_name in ::db::DB_NAMES.to_vec() {
        db_configs.push(RepositoryConfig {
            db_name: db_name.into(),
            db_config: db_config.clone(),
            db_path: tempdir.path().join(db_name).to_str().unwrap().to_string(),
        });
    }
    let client_db = Arc::new(DbRepository::init(db_configs).unwrap());

    let client = Client::new(
        ClientConfig::default(),
        &spec,
        client_db,
        Arc::new(Miner::with_spec(&spec)),
        IoChannel::disconnected(),
    )
    .unwrap();
    let good_block = get_good_dummy_block();
    if client.import_block(good_block).is_err() {
        panic!("error importing block being good by definition");
    }
    client.flush_queue();
    client.import_verified_blocks();

    let block = client.block_header(BlockId::Number(1)).unwrap();
    assert!(!block.into_inner().is_empty());
}

#[test]
fn query_none_block() {
    let tempdir = TempDir::new("").unwrap();
    let spec = get_test_spec();
    let db_config = DatabaseConfig::default();
    let mut db_configs = Vec::new();
    for db_name in ::db::DB_NAMES.to_vec() {
        db_configs.push(RepositoryConfig {
            db_name: db_name.into(),
            db_config: db_config.clone(),
            db_path: tempdir.path().join(db_name).to_str().unwrap().to_string(),
        });
    }
    let client_db = Arc::new(DbRepository::init(db_configs).unwrap());

    let client = Client::new(
        ClientConfig::default(),
        &spec,
        client_db,
        Arc::new(Miner::with_spec(&spec)),
        IoChannel::disconnected(),
    )
    .unwrap();
    let non_existant = client.block_header(BlockId::Number(188));
    assert!(non_existant.is_none());
}

#[test]
fn query_bad_block() {
    let client = get_test_client_with_blocks(vec![get_bad_state_dummy_block()]);
    let bad_block: Option<_> = client.block_header(BlockId::Number(1));

    assert!(bad_block.is_none());
}

#[test]
fn returns_chain_info() {
    let dummy_block = get_good_dummy_block();
    let client = get_test_client_with_blocks(vec![dummy_block.clone()]);
    let block = BlockView::new(&dummy_block);
    let info = client.chain_info();
    assert_eq!(info.best_block_hash, block.header().hash());
}

#[test]
fn returns_logs() {
    let dummy_block = get_good_dummy_block();
    let client = get_test_client_with_blocks(vec![dummy_block.clone()]);
    let logs = client.logs(Filter {
        from_block: BlockId::Earliest,
        to_block: BlockId::Latest,
        address: None,
        topics: vec![],
        limit: None,
    });
    assert_eq!(logs.len(), 0);
}

#[test]
fn returns_logs_with_limit() {
    let dummy_block = get_good_dummy_block();
    let client = get_test_client_with_blocks(vec![dummy_block.clone()]);
    let logs = client.logs(Filter {
        from_block: BlockId::Earliest,
        to_block: BlockId::Latest,
        address: None,
        topics: vec![],
        limit: None,
    });
    assert_eq!(logs.len(), 0);
}

#[test]
fn returns_block_body() {
    let dummy_block = get_good_dummy_block();
    let client = get_test_client_with_blocks(vec![dummy_block.clone()]);
    let block = BlockView::new(&dummy_block);
    let body = client
        .block_body(BlockId::Hash(block.header().hash()))
        .unwrap();
    let body = body.rlp();
    assert_eq!(body.item_count(), 1);
    assert_eq!(body.at(0).as_raw()[..], block.rlp().at(1).as_raw()[..]);
}

#[test]
fn imports_block_sequence() {
    let client = generate_dummy_client(6);
    let block = client.block_header(BlockId::Number(5)).unwrap();

    assert!(!block.into_inner().is_empty());
}

#[test]
fn can_client_collect_garbage() {
    let client = generate_dummy_client(100);
    client.tick();
    assert!(client.blockchain_cache_info().blocks < 100 * 1024);
}

#[test]
fn empty_gas_price_histogram() {
    let client = generate_dummy_client_with_data(20, 0, slice_into![]);

    assert!(client.gas_price_corpus(20, 64).histogram(5).is_none());
}

#[test]
fn can_handle_long_fork() {
    let client = generate_dummy_client(1200);
    for _ in 0..20 {
        client.import_verified_blocks();
    }
    assert_eq!(1200, client.chain_info().best_block_number);

    push_blocks_to_client(&client, 45, 1201, 800);
    push_blocks_to_client(&client, 49, 1201, 800);
    push_blocks_to_client(&client, 53, 1201, 600);

    for _ in 0..400 {
        client.import_verified_blocks();
    }
    assert_eq!(2000, client.chain_info().best_block_number);
}

#[test]
fn can_mine() {
    let dummy_blocks = get_good_dummy_block_seq(2);
    let client = get_test_client_with_blocks(vec![dummy_blocks[0].clone()]);

    let b = client
        .prepare_open_block(
            Address::default(),
            (3141562.into(), 31415620.into()),
            vec![],
            None,
        )
        .close();

    assert_eq!(
        *b.block().header().parent_hash(),
        BlockView::new(&dummy_blocks[0]).header_view().hash()
    );
}

#[test]
fn change_history_size() {
    let tempdir = TempDir::new("").unwrap();
    let test_spec = Spec::new_null();
    let mut config = ClientConfig::default();
    let mut db_config = DatabaseConfig::default();
    db_config.max_open_files = 512;
    db_config.memory_budget = 1024;
    db_config.block_size = 64;
    let mut db_configs = Vec::new();
    for db_name in ::db::DB_NAMES.to_vec() {
        db_configs.push(RepositoryConfig {
            db_name: db_name.into(),
            db_config: db_config.clone(),
            db_path: tempdir.path().join(db_name).to_str().unwrap().to_string(),
        });
    }
    let client_db = Arc::new(DbRepository::init(db_configs).unwrap());

    config.history = 2;
    let address = Address::random();

    {
        let client = Client::new(
            ClientConfig::default(),
            &test_spec,
            client_db.clone(),
            Arc::new(Miner::with_spec(&test_spec)),
            IoChannel::disconnected(),
        )
        .unwrap();
        for _ in 0..20 {
            let mut b = client.prepare_open_block(
                Address::default(),
                (3141562.into(), 31415620.into()),
                vec![],
                None,
            );
            b.set_difficulty(U256::from(1));
            b.block_mut()
                .state_mut()
                .add_balance(&address, &5.into(), CleanupMode::NoEmpty)
                .unwrap();
            b.block_mut().state_mut().commit().unwrap();
            let b = b.close_and_lock().seal(&*test_spec.engine, vec![]).unwrap();
            client.import_sealed_block(b).unwrap(); // account change is in the journal overlay
        }
    }
    let mut config = ClientConfig::default();
    config.history = 10;
    let client = Client::new(
        config,
        &test_spec,
        client_db,
        Arc::new(Miner::with_spec(&test_spec)),
        IoChannel::disconnected(),
    )
    .unwrap();
    assert_eq!(client.state().balance(&address).unwrap(), 100.into());
}

#[test]
fn does_not_propagate_delayed_transactions() {
    let secret = Ed25519Secret::from_str("7ea8af7d0982509cd815096d35bc3a295f57b2a078e4e25731e3ea977b9544626702b86f33072a55f46003b1e3e242eb18556be54c5ab12044c3c20829e0abb5").unwrap();
    let tx0 = PendingTransaction::new(
        Transaction {
            nonce: 0.into(),
            gas_price: 0.into(),
            gas: 21000.into(),
            action: Action::Call(Address::default()),
            value: 0.into(),
            data: Vec::new(),
            nonce_bytes: Vec::new(),
            gas_price_bytes: Vec::new(),
            gas_bytes: Vec::new(),
            value_bytes: Vec::new(),
            transaction_type: 0x01.into(),
        }
        .sign(&secret, None),
        Some(Condition::Number(2)),
    );
    let tx1 = PendingTransaction::new(
        Transaction {
            nonce: 1.into(),
            gas_price: 0.into(),
            gas: 21000.into(),
            action: Action::Call(Address::default()),
            value: 0.into(),
            data: Vec::new(),
            nonce_bytes: Vec::new(),
            gas_price_bytes: Vec::new(),
            gas_bytes: Vec::new(),
            value_bytes: Vec::new(),
            transaction_type: 0x01.into(),
        }
        .sign(&secret, None),
        None,
    );
    let client = generate_dummy_client(1);

    client
        .miner()
        .import_own_transaction(&*client, tx0)
        .unwrap();
    client
        .miner()
        .import_own_transaction(&*client, tx1)
        .unwrap();
    client.miner().update_transaction_pool(&*client, true);
    assert_eq!(0, client.ready_transactions().len());
    assert_eq!(2, client.miner().pending_transactions().len());
    push_blocks_to_client(&client, 53, 2, 2);
    client.flush_queue();
    assert_eq!(2, client.ready_transactions().len());
    assert_eq!(2, client.miner().pending_transactions().len());
}

#[test]
fn transaction_proof() {
    use client::ProvingBlockChainClient;

    let client = generate_dummy_client(0);
    let address = Address::random();
    let test_spec = Spec::new_test();
    for _ in 0..20 {
        let mut b = client.prepare_open_block(
            Address::default(),
            (3141562.into(), 31415620.into()),
            vec![],
            None,
        );
        b.block_mut()
            .state_mut()
            .add_balance(&address, &5.into(), CleanupMode::NoEmpty)
            .unwrap();
        b.set_difficulty(U256::from(1));
        b.block_mut().state_mut().commit().unwrap();
        let b = b.close_and_lock().seal(&*test_spec.engine, vec![]).unwrap();
        client.import_sealed_block(b).unwrap(); // account change is in the journal overlay
    }

    let transaction = Transaction {
        nonce: 0.into(),
        gas_price: 0.into(),
        gas: 21000.into(),
        action: Action::Call(Address::default()),
        value: 5.into(),
        data: Vec::new(),
        nonce_bytes: Vec::new(),
        gas_price_bytes: Vec::new(),
        gas_bytes: Vec::new(),
        value_bytes: Vec::new(),
        transaction_type: 0x01.into(),
    }
    .fake_sign(address);

    let proof = client
        .prove_transaction(transaction.clone(), BlockId::Latest)
        .unwrap()
        .1;
    let backend = state::backend::ProofCheck::new(&proof);

    let mut factories = ::factory::Factories::default();
    factories.accountdb = ::account_db::Factory::Plain; // raw state values, no mangled keys.
    let root = client.best_block_header().state_root();

    let mut state = State::from_existing(
        backend,
        root,
        0.into(),
        factories.clone(),
        Arc::new(MemoryDBRepository::new()),
    )
    .unwrap();
    Executive::new(
        &mut state,
        &client.latest_env_info(),
        test_spec.engine.machine(),
    )
    .transact(&transaction, false, false)
    .unwrap();

    assert_eq!(state.balance(&Address::default()).unwrap(), 5.into());
    assert_eq!(state.balance(&address).unwrap(), 95.into());
}
