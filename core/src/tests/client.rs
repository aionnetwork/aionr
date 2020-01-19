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
use crate::client::{BlockChainClient, MiningBlockChainClient, Client, ClientConfig, BlockId};
use crate::state::{CleanupMode};
use crate::block::IsBlock;
use crate::types::filter::Filter;
use aion_types::{Address, U256};
use kvdb::{DatabaseConfig, DbRepository, RepositoryConfig};
use crate::miner::Miner;
use crate::spec::Spec;
use crate::views::BlockView;
use key::Ed25519Secret;
use crate::transaction::{PendingTransaction, Transaction, Action, Condition};
use crate::miner::MinerService;
use tempdir::TempDir;
use crate::helpers::*;
use aion_types::H256;
use crate::db::DB_NAMES;

#[test]
fn imports_from_empty() {
    let tempdir = TempDir::new("").unwrap();
    let spec = get_test_spec();
    let db_config = DatabaseConfig::default();
    let mut db_configs = Vec::new();
    for db_name in DB_NAMES.to_vec() {
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
fn client_check_vote() {
    let tempdir = TempDir::new("").unwrap();
    let spec = get_test_spec();
    let db_config = DatabaseConfig::default();
    let mut db_configs = Vec::new();
    for db_name in DB_NAMES.to_vec() {
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

    assert_eq!(
        client
            .get_stake(&H256::default(), Address::default(), BlockId::Latest)
            .unwrap(),
        0u64.into()
    );
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
    for db_name in DB_NAMES.to_vec() {
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
    for db_name in DB_NAMES.to_vec() {
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
        // TODO: with limit
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
/*

#[test]
fn should_not_import_blocks_with_beacon_before_fork_point(){
    generate_dummy_unity_client_with_beacon(3);
}
*/

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
    // TODO: how to judge the first two
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
    for db_name in DB_NAMES.to_vec() {
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
            config, //ClientConfig::default(),
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
            beacon: None,
        }
        .sign(&secret),
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
            beacon: None,
        }
        .sign(&secret),
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
fn test_total_difficulty() {
    let tempdir = TempDir::new("").unwrap();
    // use null_morden spec
    let spec = get_test_spec();
    let db_config = DatabaseConfig::default();
    let mut db_configs = Vec::new();
    for db_name in DB_NAMES.to_vec() {
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

    // td == 0x2000 , pow_td == 0x2000, pos_td == 0
    assert_eq!(
        client.block_total_difficulty(BlockId::Latest),
        Some(0x2000.into())
    );

    //import a pos block
    let good_block = get_good_dummy_pos_block();
    if client.import_block(good_block).is_err() {
        panic!("error importing block being good by definition");
    }

    // pos block is in queue but not in chain now;
    // chain: td == 0x2000 , pow_td == 0x2000, pos_td == 0
    assert_eq!(
        client.block_total_difficulty(BlockId::Latest),
        Some(0x2000.into())
    );
    let info = client.chain_info();
    assert_eq!(info.total_difficulty, 0x2000.into());

    // TODO: Commented out for wrong pending_total_difficulty calculation. Open and fix it after modification.
    //    // chain+queue : td == 0x2000 + 0x20000 = 0x22000
    //    assert_eq!(info.pending_total_difficulty, 0x22000.into());
    //
    //    //import a pow block
    //    let good_block = get_good_dummy_block();
    //    if client.import_block(good_block).is_err() {
    //        panic!("error importing block being good by definition");
    //    }
    //
    //    // pow block and pos block are in queue but not in chain now;
    //    // chain: td == 0x2000 , pow_td == 0x2000, pos_td == 0
    //    assert_eq!(
    //        client.block_total_difficulty(BlockId::Latest),
    //        Some(0x2000.into())
    //    );
    //    let info = client.chain_info();
    //    assert_eq!(info.total_difficulty, 0x2000.into());
    //    // chain+queue : td == (0x2000 + 0x20000) + 0x20000 = 0x42000
    //    assert_eq!(info.pending_total_difficulty, 0x42000u64.into());

    // flush_queue
    client.flush_queue();
    client.import_verified_blocks();

    // pos block is in chain now;
    // chain: td == 0x22000 , pow_td == 0x2000 , pos_td == 0x20000
    assert_eq!(
        client.block_total_difficulty(BlockId::Latest),
        Some(0x22000u64.into())
    );

    let info = client.chain_info();
    assert_eq!(info.total_difficulty, 0x22000u64.into());

    //    // chain+queue : td == 0x2000 + 0x20000 = 0x22000
    //    assert_eq!(info.pending_total_difficulty,0x22000u64.into());
}
