mod generator;

use std::iter;
use std::sync::Arc;
use rustc_hex::FromHex;
use kvdb::{KeyValueDB, MockDbRepository, DBTransaction};
use aion_types::*;
use ethbloom::Bloom;
use receipt::{Receipt, SimpleReceipt};
use blockchain::{BlockProvider, BlockChain};
use types::blockchain::import_route::ImportRoute;
use helpers::*;
use self::generator::{BlockGenerator, BlockBuilder, BlockOptions};
use types::blockchain::extra::TransactionAddress;
use transaction::{Transaction, Action, DEFAULT_TRANSACTION_TYPE};
use log_entry::{LogEntry, LocalizedLogEntry};
use keychain;
use db;

fn new_db() -> Arc<KeyValueDB> {
    let mut db_configs = Vec::new();
    for db_name in db::DB_NAMES.to_vec() {
        db_configs.push(db_name.into());
    }
    Arc::new(MockDbRepository::init(db_configs))
}

fn new_chain(genesis: &[u8], db: Arc<KeyValueDB>) -> BlockChain {
    BlockChain::new(Default::default(), genesis, db, None, None)
}

#[test]
fn should_cache_best_block() {
    // given
    let genesis = BlockBuilder::genesis();
    let first = genesis.add_block();

    let db = new_db();
    let bc = new_chain(&genesis.last().encoded(), db.clone());
    assert_eq!(bc.best_block_number(), 0);

    // when
    let mut batch = DBTransaction::new();
    bc.insert_block(&mut batch, &first.last().encoded(), vec![]);
    assert_eq!(bc.best_block_number(), 0);
    bc.commit();
    // NOTE no db.write here (we want to check if best block is cached)

    // then
    assert_eq!(bc.best_block_number(), 1);
    assert!(
        bc.block(&bc.best_block_hash()).is_some(),
        "Best block should be queryable even without DB write."
    );
}

#[test]
fn basic_blockchain_insert() {
    let genesis = BlockBuilder::genesis();
    let first = genesis.add_block();

    let genesis = genesis.last();
    let first = first.last();
    let genesis_hash = genesis.hash();
    let first_hash = first.hash();

    let db = new_db();
    let bc = new_chain(&genesis.encoded(), db.clone());

    assert_eq!(bc.genesis_hash(), genesis_hash);
    assert_eq!(bc.best_block_hash(), genesis_hash);
    assert_eq!(bc.block_hash(0), Some(genesis_hash));
    assert_eq!(bc.block_hash(1), None);
    assert_eq!(bc.block_details(&genesis_hash).unwrap().children, vec![]);

    let mut batch = DBTransaction::new();
    bc.insert_block(&mut batch, &first.encoded(), vec![]);
    db.write(batch).unwrap();
    bc.commit();

    assert_eq!(bc.block_hash(0), Some(genesis_hash));
    assert_eq!(bc.best_block_number(), 1);
    assert_eq!(bc.best_block_hash(), first_hash);
    assert_eq!(bc.block_hash(1), Some(first_hash));
    assert_eq!(bc.block_details(&first_hash).unwrap().parent, genesis_hash);
    assert_eq!(
        bc.block_details(&genesis_hash).unwrap().children,
        vec![first_hash]
    );
    assert_eq!(bc.block_hash(2), None);
}

#[test]
fn check_ancestry_iter() {
    let genesis = BlockBuilder::genesis();
    let first_10 = genesis.add_blocks(10);
    let generator = BlockGenerator::new(vec![first_10]);

    let db = new_db();
    let bc = new_chain(&genesis.last().encoded(), db.clone());

    let mut block_hashes = vec![genesis.last().hash()];
    let mut batch = DBTransaction::new();
    for block in generator {
        block_hashes.push(block.hash());
        bc.insert_block(&mut batch, &block.encoded(), vec![]);
        bc.commit();
    }
    db.write(batch).unwrap();

    block_hashes.reverse();

    assert_eq!(
        bc.ancestry_iter(block_hashes[0].clone())
            .unwrap()
            .collect::<Vec<_>>(),
        block_hashes
    );
    assert_eq!(block_hashes.len(), 11);
}

#[test]
fn test_fork_transaction_addresses() {
    let t1 = Transaction {
        nonce: 0.into(),
        gas_price: 0.into(),
        gas: 100_000.into(),
        action: Action::Create,
        value: 100.into(),
        data: "601080600c6000396000f3006000355415600957005b60203560003555"
            .from_hex()
            .unwrap(),
        transaction_type: DEFAULT_TRANSACTION_TYPE,
        nonce_bytes: Vec::new(),
        gas_bytes: Vec::new(),
        gas_price_bytes: Vec::new(),
        value_bytes: Vec::new(),
    }
    .sign(keychain::ethkey::generate_keypair().secret(), None);

    let t1_hash = t1.hash().clone();

    let genesis = BlockBuilder::genesis();
    let b1a = genesis.add_block_with_transactions(iter::once(t1));
    let b1b = genesis.add_block_with_difficulty(9);
    let b2 = b1b.add_block();

    let b1a_hash = b1a.last().hash();
    let b2_hash = b2.last().hash();

    let db = new_db();
    let bc = new_chain(&genesis.last().encoded(), db.clone());

    let mut batch = DBTransaction::new();
    let _ = bc.insert_block(&mut batch, &b1a.last().encoded(), vec![]);
    bc.commit();
    let _ = bc.insert_block(&mut batch, &b1b.last().encoded(), vec![]);
    bc.commit();
    db.write(batch).unwrap();

    assert_eq!(bc.best_block_hash(), b1a_hash);
    assert_eq!(
        bc.transaction_address(&t1_hash),
        Some(TransactionAddress {
            block_hash: b1a_hash,
            index: 0,
        })
    );

    // now let's make forked chain the canon chain
    let mut batch = DBTransaction::new();
    let _ = bc.insert_block(&mut batch, &b2.last().encoded(), vec![]);
    bc.commit();
    db.write(batch).unwrap();

    // Transaction should be retracted
    assert_eq!(bc.best_block_hash(), b2_hash);
    assert_eq!(bc.transaction_address(&t1_hash), None);
}

#[test]
fn test_overwriting_transaction_addresses() {
    let keypair = keychain::ethkey::generate_keypair();
    let t1 = Transaction {
        nonce: 0.into(),
        gas_price: 0.into(),
        gas: 100_000.into(),
        action: Action::Create,
        value: 100.into(),
        data: "601080600c6000396000f3006000355415600957005b60203560003555"
            .from_hex()
            .unwrap(),
        transaction_type: DEFAULT_TRANSACTION_TYPE,
        gas_price_bytes: Vec::new(),
        gas_bytes: Vec::new(),
        value_bytes: Vec::new(),
        nonce_bytes: Vec::new(),
    }
    .sign(&keypair.secret(), None);

    let t2 = Transaction {
        nonce: 1.into(),
        gas_price: 0.into(),
        gas: 100_000.into(),
        action: Action::Create,
        value: 100.into(),
        data: "601080600c6000396000f3006000355415600957005b60203560003555"
            .from_hex()
            .unwrap(),
        gas_price_bytes: Vec::new(),
        gas_bytes: Vec::new(),
        value_bytes: Vec::new(),
        nonce_bytes: Vec::new(),
        transaction_type: DEFAULT_TRANSACTION_TYPE,
    }
    .sign(&keypair.secret(), None);
    let t3 = Transaction {
        nonce: 2.into(),
        gas_price: 0.into(),
        gas: 100_000.into(),
        action: Action::Create,
        value: 100.into(),
        data: "601080600c6000396000f3006000355415600957005b60203560003555"
            .from_hex()
            .unwrap(),
        gas_price_bytes: Vec::new(),
        gas_bytes: Vec::new(),
        value_bytes: Vec::new(),
        nonce_bytes: Vec::new(),
        transaction_type: DEFAULT_TRANSACTION_TYPE,
    }
    .sign(&keypair.secret(), None);

    let genesis = BlockBuilder::genesis();
    let b1a = genesis.add_block_with_transactions(vec![t1.clone(), t2.clone()]);
    // insert transactions in different order,
    // the block has lower difficulty, so the hash is also different
    let b1b = genesis.add_block_with(|| {
        BlockOptions {
            difficulty: 9.into(),
            transactions: vec![t2.clone(), t1.clone()],
            ..Default::default()
        }
    });
    let b2 = b1b.add_block_with_transactions(iter::once(t3.clone()));

    let b1a_hash = b1a.last().hash();
    let b1b_hash = b1b.last().hash();
    let b2_hash = b2.last().hash();

    let t1_hash = t1.hash();
    let t2_hash = t2.hash();
    let t3_hash = t3.hash();

    let db = new_db();
    let bc = new_chain(&genesis.last().encoded(), db.clone());

    let mut batch = DBTransaction::new();
    let _ = bc.insert_block(&mut batch, &b1a.last().encoded(), vec![]);
    bc.commit();
    let _ = bc.insert_block(&mut batch, &b1b.last().encoded(), vec![]);
    bc.commit();
    db.write(batch).unwrap();

    assert_eq!(bc.best_block_hash(), b1a_hash);
    assert_eq!(
        bc.transaction_address(&t1_hash),
        Some(TransactionAddress {
            block_hash: b1a_hash,
            index: 0,
        })
    );
    assert_eq!(
        bc.transaction_address(&t2_hash),
        Some(TransactionAddress {
            block_hash: b1a_hash,
            index: 1,
        })
    );

    // now let's make forked chain the canon chain
    let mut batch = DBTransaction::new();
    let _ = bc.insert_block(&mut batch, &b2.last().encoded(), vec![]);
    bc.commit();
    db.write(batch).unwrap();

    assert_eq!(bc.best_block_hash(), b2_hash);
    assert_eq!(
        bc.transaction_address(&t1_hash),
        Some(TransactionAddress {
            block_hash: b1b_hash,
            index: 1,
        })
    );
    assert_eq!(
        bc.transaction_address(&t2_hash),
        Some(TransactionAddress {
            block_hash: b1b_hash,
            index: 0,
        })
    );
    assert_eq!(
        bc.transaction_address(&t3_hash),
        Some(TransactionAddress {
            block_hash: b2_hash,
            index: 0,
        })
    );
}

#[test]
fn test_small_fork() {
    let genesis = BlockBuilder::genesis();
    let b1 = genesis.add_block();
    let b2 = b1.add_block();
    let b3a = b2.add_block();
    let b3b = b2.add_block_with_difficulty(9);

    let genesis_hash = genesis.last().hash();
    let b1_hash = b1.last().hash();
    let b2_hash = b2.last().hash();
    let b3a_hash = b3a.last().hash();
    let b3b_hash = b3b.last().hash();

    // b3a is a part of canon chain, whereas b3b is part of sidechain
    let best_block_hash = b3a_hash;

    let db = new_db();
    let bc = new_chain(&genesis.last().encoded(), db.clone());

    let mut batch = DBTransaction::new();
    let ir1 = bc.insert_block(&mut batch, &b1.last().encoded(), vec![]);
    bc.commit();
    let ir2 = bc.insert_block(&mut batch, &b2.last().encoded(), vec![]);
    bc.commit();
    let ir3b = bc.insert_block(&mut batch, &b3b.last().encoded(), vec![]);
    bc.commit();
    db.write(batch).unwrap();
    assert_eq!(bc.block_hash(3).unwrap(), b3b_hash);
    let mut batch = DBTransaction::new();
    let ir3a = bc.insert_block(&mut batch, &b3a.last().encoded(), vec![]);
    bc.commit();
    db.write(batch).unwrap();

    assert_eq!(
        ir1,
        ImportRoute {
            enacted: vec![b1_hash],
            retracted: vec![],
            omitted: vec![],
        }
    );

    assert_eq!(
        ir2,
        ImportRoute {
            enacted: vec![b2_hash],
            retracted: vec![],
            omitted: vec![],
        }
    );

    assert_eq!(
        ir3b,
        ImportRoute {
            enacted: vec![b3b_hash],
            retracted: vec![],
            omitted: vec![],
        }
    );

    assert_eq!(
        ir3a,
        ImportRoute {
            enacted: vec![b3a_hash],
            retracted: vec![b3b_hash],
            omitted: vec![],
        }
    );

    assert_eq!(bc.best_block_hash(), best_block_hash);
    assert_eq!(bc.block_number(&genesis_hash).unwrap(), 0);
    assert_eq!(bc.block_number(&b1_hash).unwrap(), 1);
    assert_eq!(bc.block_number(&b2_hash).unwrap(), 2);
    assert_eq!(bc.block_number(&b3a_hash).unwrap(), 3);
    assert_eq!(bc.block_number(&b3b_hash).unwrap(), 3);

    assert_eq!(bc.block_hash(0).unwrap(), genesis_hash);
    assert_eq!(bc.block_hash(1).unwrap(), b1_hash);
    assert_eq!(bc.block_hash(2).unwrap(), b2_hash);
    assert_eq!(bc.block_hash(3).unwrap(), b3a_hash);

    // test trie route
    let r0_1 = bc.tree_route(genesis_hash, b1_hash).unwrap();
    assert_eq!(r0_1.ancestor, genesis_hash);
    assert_eq!(r0_1.blocks, [b1_hash]);
    assert_eq!(r0_1.index, 0);

    let r0_2 = bc.tree_route(genesis_hash, b2_hash).unwrap();
    assert_eq!(r0_2.ancestor, genesis_hash);
    assert_eq!(r0_2.blocks, [b1_hash, b2_hash]);
    assert_eq!(r0_2.index, 0);

    let r1_3a = bc.tree_route(b1_hash, b3a_hash).unwrap();
    assert_eq!(r1_3a.ancestor, b1_hash);
    assert_eq!(r1_3a.blocks, [b2_hash, b3a_hash]);
    assert_eq!(r1_3a.index, 0);

    let r1_3b = bc.tree_route(b1_hash, b3b_hash).unwrap();
    assert_eq!(r1_3b.ancestor, b1_hash);
    assert_eq!(r1_3b.blocks, [b2_hash, b3b_hash]);
    assert_eq!(r1_3b.index, 0);

    let r3a_3b = bc.tree_route(b3a_hash, b3b_hash).unwrap();
    assert_eq!(r3a_3b.ancestor, b2_hash);
    assert_eq!(r3a_3b.blocks, [b3a_hash, b3b_hash]);
    assert_eq!(r3a_3b.index, 1);

    let r1_0 = bc.tree_route(b1_hash, genesis_hash).unwrap();
    assert_eq!(r1_0.ancestor, genesis_hash);
    assert_eq!(r1_0.blocks, [b1_hash]);
    assert_eq!(r1_0.index, 1);

    let r2_0 = bc.tree_route(b2_hash, genesis_hash).unwrap();
    assert_eq!(r2_0.ancestor, genesis_hash);
    assert_eq!(r2_0.blocks, [b2_hash, b1_hash]);
    assert_eq!(r2_0.index, 2);

    let r3a_1 = bc.tree_route(b3a_hash, b1_hash).unwrap();
    assert_eq!(r3a_1.ancestor, b1_hash);
    assert_eq!(r3a_1.blocks, [b3a_hash, b2_hash]);
    assert_eq!(r3a_1.index, 2);

    let r3b_1 = bc.tree_route(b3b_hash, b1_hash).unwrap();
    assert_eq!(r3b_1.ancestor, b1_hash);
    assert_eq!(r3b_1.blocks, [b3b_hash, b2_hash]);
    assert_eq!(r3b_1.index, 2);

    let r3b_3a = bc.tree_route(b3b_hash, b3a_hash).unwrap();
    assert_eq!(r3b_3a.ancestor, b2_hash);
    assert_eq!(r3b_3a.blocks, [b3b_hash, b3a_hash]);
    assert_eq!(r3b_3a.index, 1);
}

#[test]
fn test_reopen_blockchain_db() {
    let genesis = BlockBuilder::genesis();
    let first = genesis.add_block();
    let genesis_hash = genesis.last().hash();
    let first_hash = first.last().hash();

    let db = new_db();

    {
        let bc = new_chain(&genesis.last().encoded(), db.clone());
        assert_eq!(bc.best_block_hash(), genesis_hash);
        let mut batch = DBTransaction::new();
        bc.insert_block(&mut batch, &first.last().encoded(), vec![]);
        db.write(batch).unwrap();
        bc.commit();
        assert_eq!(bc.best_block_hash(), first_hash);
    }

    {
        let bc = new_chain(&genesis.last().encoded(), db.clone());

        assert_eq!(bc.best_block_hash(), first_hash);
    }
}

#[test]
fn can_contain_arbitrary_block_sequence() {
    let bc = generate_dummy_blockchain(50);
    assert_eq!(bc.best_block_number(), 49);
}

#[test]
fn can_collect_garbage() {
    let bc = generate_dummy_blockchain(3000);

    assert_eq!(bc.best_block_number(), 2999);
    let best_hash = bc.best_block_hash();
    let mut block_header = bc.block_header(&best_hash);

    while !block_header.is_none() {
        block_header = bc.block_header(block_header.unwrap().parent_hash());
    }
    assert!(bc.cache_size().blocks > 1024 * 1024);

    for _ in 0..2 {
        bc.collect_garbage();
    }
    assert!(bc.cache_size().blocks < 1024 * 1024);
}

#[test]
fn can_contain_arbitrary_block_sequence_with_extra() {
    let bc = generate_dummy_blockchain_with_extra(25);
    assert_eq!(bc.best_block_number(), 24);
}

#[test]
fn can_contain_only_genesis_block() {
    let bc = generate_dummy_empty_blockchain();
    assert_eq!(bc.best_block_number(), 0);
}

#[test]
fn find_transaction_by_hash() {
    let genesis = "f9077ef9077a0180a06a6d99a2ef14ab3b835dfc92fb918d76c37f6578a69825fbe19cd366485604b1a00000000000000000000000000000000000000000000000000000000000000000a03663a3a8bc1204f4c3ac972278493e26a339b7fb720c94a777a86a39debdf810a045b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0a045b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0b901000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010a000000000000000000000000000000000000000000000000000000000000001008083e4e1c0845ade7380a00000000000000000000000000000000000000000000000000000000000000000b9058000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c0".from_hex().unwrap();
    let b1 = "f908ccf9078c0101a0ef32028308d0dc0376be3ddde8ec56fd23d1142300441afc459c3c6bdc39a7d1a00000000000000000000000000000000000000000000000000000000000000000a08f3b78418265c4112d517180089ca78ebdfa005610b4890d0ec3a05b4894e6aea013f0924f46521a109a46d1c30a79b754e7f1cc5e234366f2454ebf0f135622bda00e6a1d518ad68354e3efdabe300ff14dee3a47d77309cf275f9d1e49359d41f8b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000009000000000000000000000000000000010a041494f4e0000000000000000000000000000000000000000000000000000000082a41083e4a889845b864af2a00400000000000000000000000000000000000001000000000000000000000000b9058000a7e2ebbd73cdac14f8c01cfaca912e63c7cd63c192169b76790863f1757a34e06f68aa204a9860b43997940d741a94ee98991ff6ad1820527f81890823473785eb25b5ef1e9dad79c47d6e195fc04b30a10b80affeac5511d74b3ba501ab2256dd616d2a02be1287e7ce7557ec4f21aede56bc129330c2e370db2bbcffb97c7efddccc313ee94c4ff55081a7bfe65e75f68f7eb80ff62aaea341bff9b01ba71fae7db7106d972b9e4c99848bf0e2d502a3144e2c77ec91b41b9c2728b5d24682d180a861b6058565e2e68076a6e7d8463c33ed28dc171276fe1dcb07a3ae9fa6ba8066f47b82092f39b5b525ff75f637194e37a67d92972ef2fe121e5e0ef60371ac6550388e75163c75886dd38eacf56fa8246cf14aed2e3918fb904f16592af2eb0eeda87cd20920b4ce8acafdda94b7e6741bb9fb67c336e05faa69db5c6f75a94c4b0e667330c1440cfd54e03447045ad442a972c780c04d8ddafc2c1e0128b3055e340760a0812a3fa7f9086fb7e2bc72acef0bf5d1e431eb640ab2c4852bfe5e58ada6df066fb90e06928f161f6392ceedda894b4abbd9b266cf9ea3a87b1bea90b1cc3c6781bdb47e54242ac70928ca5de81470012e152dc10b0080be3d0a1a9f387d87bbb2b9bb5e650eef97644939328bb19a4d528162f92f1b91e3fdd5ba05dc45bda431ab1d738b7677eb435ff1ba9738b6ba9362c447699a180d00f7c1ce6453da239aea645fbe448602ed881fc476569c1a4421445c560f1b57cfdca8904e088a674e13f8a79a752c4973ff638a331b4b3a7ea5ca09367e262664c538a312b90a3499b97ea3b04d631cc94df593ed13c9018eb1d7305ec4163b73076940a058a71e1cabed5b84edc9735f87463e9180f33a4b367855b979b96b584aad24db78285088ba976e3c8a4bdba9d3d83cec02c1b734f5601886b674e8b6b38eb7c14aa4b13d7e51f2aba6b8a6e06b55648c9843617f1b5df62e6ec801f065bb8c81640b71561508ccd12290f28e666028e507b147aa5bf75846fdb724d021bb65143fd6ffbf926f1c64b674efca2b3171546954f175a0bed6bd862c552831091bffd52660e56373a842319e40117690e29d2ac1071a3a48d389804e79aa920e6ff179e3f0ff455900a52cfd2fcd4f44232475840d6d88de75c8a8d1783d59560d5d420fd57223b9271c033f072f611d4c9465b86fd027ff4cfb48560f8bb9c6b63ab76ef49454ca0d1ca6ce06a913b123131f2a1a105b5e6fe3705295e7e4ffbb593d62f30cda47c402f41afa74c3b25a6e6b4408ef5ba60a0f7ce21a61b45561c2790f430ded3ab4c743738ee7281151df1552bab96facd5ad4b330bca7d3a7477ad3e0792ed488925fc31eed2828f35029fbc0a3f90f3747a20eaebb1f9669bc2a6955025a346a175e374449c026422f473483f094c872b23d34a7c22a2255712ba7af9635ffa7185358aeb91320e0869223df12fa82d416a6026039785792351219be47249566a26288df6929db2e3134a77b60a42d6aaa39bf4d65b53c9cc8576f9896f43b70983505eb0741d639b02151927255b871a347b36f1943d76f5618ea9912febe3fc7903dabdc3b99607371b4b0e7887599851e53750d35c6456eefccb7d5ee43b9f02377dc631e7b4fbc9d6e8b149827a54457bef1a79b4001283e7183c0173418c3e1b27e557d3ee727e9e3b3ed5366eaa21e66aeb4776c6a974d432bedd276f8461f7eb09b8aecd95a0b535502cc6136a87985a6354cc99ecbd440c038b0f197ff32efbbc4c80bb679d18c3102edcc41b1c73c445a30853b3f2d34bc743964547d26e6e17cc38fb22f46147b7f7e39cf5429f05f7bb28f361ebda3610d6e54b24ccb5bcf6c13864ed06546018863fa25bf311399db17353f253a065bf25b211ff0d8bade1b2cef627f0ab8d33f472fde7ef0955b5b3bde869e74e765b6e3861b968bdb7d2a274e1e05b2417643f18354de1ce23f9013af89b80a0a054340a3152d10006b66c4248cfa73e5725056294081c476c0e67ef5ad25334820fff80880005748de2c04d69830e57e0841f38b2e601b8608bc5c4e5599afac7cb0efcb0010540017dda3e80870bb543b356867b2a8cacbfcdffb6e1b3784f4497b6121502a0991077c657e4f8e5b68f24b3644964fcf6935a3d6735521ae94c1a361d692c04769e8e8fb19392a9badd73002ce13dbf5c08f89b01a0a054340a3152d10006b66c4248cfa73e5725056294081c476c0e67ef5ad25334820fff80880005748de73f18bb830e57e0841f38b2e601b8608bc5c4e5599afac7cb0efcb0010540017dda3e80870bb543b356867b2a8cacbf516f28ee029ef5bf3231862b4065ddd9195ae560e42c216918b4d045889a37e8b7c5b0648c3b5d4190382ec34a22179c1cca4572b2ad5d5c431370c9d4a91c05".from_hex().unwrap();
    let b1_hash: H256 = "e6a15bb33f19c1292aec97acc24b35b8d2b3312619102f4887a9e4eee5171f0e".into();

    let db = new_db();
    let bc = new_chain(&genesis, db.clone());
    let mut batch = DBTransaction::new();
    bc.insert_block(&mut batch, &b1, vec![]);
    db.write(batch).unwrap();
    bc.commit();

    let transactions = bc.transactions(&b1_hash).unwrap();
    assert_eq!(transactions.len(), 2);
    for t in transactions {
        assert_eq!(
            bc.transaction(&bc.transaction_address(&t.hash()).unwrap())
                .unwrap(),
            t
        );
    }
}

fn insert_block(
    db: &Arc<KeyValueDB>,
    bc: &BlockChain,
    bytes: &[u8],
    receipts: Vec<Receipt>,
) -> ImportRoute
{
    let mut batch = DBTransaction::new();
    let res = bc.insert_block(&mut batch, bytes, receipts);
    db.write(batch).unwrap();
    bc.commit();
    res
}

#[test]
fn test_logs() {
    let keypair = keychain::ethkey::generate_keypair();
    let t1 = Transaction {
        nonce: 0.into(),
        gas_price: 0.into(),
        gas: 100_000.into(),
        action: Action::Create,
        value: 101.into(),
        data: "601080600c6000396000f3006000355415600957005b60203560003555"
            .from_hex()
            .unwrap(),
        nonce_bytes: Vec::new(),
        gas_price_bytes: Vec::new(),
        gas_bytes: Vec::new(),
        value_bytes: Vec::new(),
        transaction_type: DEFAULT_TRANSACTION_TYPE,
    }
    .sign(keypair.secret(), None);
    let t2 = Transaction {
        nonce: 0.into(),
        gas_price: 0.into(),
        gas: 100_000.into(),
        action: Action::Create,
        value: 102.into(),
        data: "601080600c6000396000f3006000355415600957005b60203560003555"
            .from_hex()
            .unwrap(),
        nonce_bytes: Vec::new(),
        gas_price_bytes: Vec::new(),
        gas_bytes: Vec::new(),
        value_bytes: Vec::new(),
        transaction_type: DEFAULT_TRANSACTION_TYPE,
    }
    .sign(keypair.secret(), None);
    let t3 = Transaction {
        nonce: 0.into(),
        gas_price: 0.into(),
        gas: 100_000.into(),
        action: Action::Create,
        value: 103.into(),
        data: "601080600c6000396000f3006000355415600957005b60203560003555"
            .from_hex()
            .unwrap(),
        nonce_bytes: Vec::new(),
        gas_price_bytes: Vec::new(),
        gas_bytes: Vec::new(),
        value_bytes: Vec::new(),
        transaction_type: DEFAULT_TRANSACTION_TYPE,
    }
    .sign(keypair.secret(), None);
    let tx_hash1 = t1.hash().clone();
    let tx_hash2 = t2.hash().clone();
    let tx_hash3 = t3.hash().clone();

    let genesis = BlockBuilder::genesis();
    let b1 = genesis.add_block_with_transactions(vec![t1, t2]);
    let b2 = b1.add_block_with_transactions(iter::once(t3));
    let b1_hash = b1.last().hash();
    let b1_number = b1.last().number();
    let b2_hash = b2.last().hash();
    let b2_number = b2.last().number();

    let db = new_db();
    let bc = new_chain(&genesis.last().encoded(), db.clone());
    insert_block(
        &db,
        &bc,
        &b1.last().encoded(),
        vec![
            Receipt {
                simple_receipt: SimpleReceipt {
                    state_root: Default::default(),
                    log_bloom: Default::default(),
                    logs: vec![
                        LogEntry {
                            address: Default::default(),
                            topics: vec![],
                            data: vec![1],
                        },
                        LogEntry {
                            address: Default::default(),
                            topics: vec![],
                            data: vec![2],
                        },
                    ],
                },
                gas_used: 10_000.into(),
                transaction_fee: U256::zero(),
                output: Default::default(),
                error_message: Default::default(),
            },
            Receipt {
                simple_receipt: SimpleReceipt {
                    state_root: Default::default(),
                    log_bloom: Default::default(),
                    logs: vec![LogEntry {
                        address: Default::default(),
                        topics: vec![],
                        data: vec![3],
                    }],
                },
                gas_used: 10_000.into(),
                transaction_fee: U256::zero(),
                output: Default::default(),
                error_message: Default::default(),
            },
        ],
    );
    insert_block(
        &db,
        &bc,
        &b2.last().encoded(),
        vec![Receipt {
            simple_receipt: SimpleReceipt {
                state_root: Default::default(),
                log_bloom: Default::default(),
                logs: vec![LogEntry {
                    address: Default::default(),
                    topics: vec![],
                    data: vec![4],
                }],
            },
            gas_used: 10_000.into(),
            transaction_fee: U256::zero(),
            output: Default::default(),
            error_message: Default::default(),
        }],
    );

    // when
    let logs1 = bc.logs(vec![1, 2], |_| true, None);
    let logs2 = bc.logs(vec![1, 2], |_| true, Some(1));

    // then
    assert_eq!(
        logs1,
        vec![
            LocalizedLogEntry {
                entry: LogEntry {
                    address: Default::default(),
                    topics: vec![],
                    data: vec![1],
                },
                block_hash: b1_hash,
                block_number: b1_number,
                transaction_hash: tx_hash1,
                transaction_index: 0,
                transaction_log_index: 0,
                log_index: 0,
            },
            LocalizedLogEntry {
                entry: LogEntry {
                    address: Default::default(),
                    topics: vec![],
                    data: vec![2],
                },
                block_hash: b1_hash,
                block_number: b1_number,
                transaction_hash: tx_hash1,
                transaction_index: 0,
                transaction_log_index: 1,
                log_index: 1,
            },
            LocalizedLogEntry {
                entry: LogEntry {
                    address: Default::default(),
                    topics: vec![],
                    data: vec![3],
                },
                block_hash: b1_hash,
                block_number: b1_number,
                transaction_hash: tx_hash2,
                transaction_index: 1,
                transaction_log_index: 0,
                log_index: 2,
            },
            LocalizedLogEntry {
                entry: LogEntry {
                    address: Default::default(),
                    topics: vec![],
                    data: vec![4],
                },
                block_hash: b2_hash,
                block_number: b2_number,
                transaction_hash: tx_hash3,
                transaction_index: 0,
                transaction_log_index: 0,
                log_index: 0,
            },
        ]
    );
    assert_eq!(
        logs2,
        vec![LocalizedLogEntry {
            entry: LogEntry {
                address: Default::default(),
                topics: vec![],
                data: vec![4],
            },
            block_hash: b2_hash,
            block_number: b2_number,
            transaction_hash: tx_hash3,
            transaction_index: 0,
            transaction_log_index: 0,
            log_index: 0,
        }]
    );
}

#[test]
fn test_bloom_filter_simple() {
    let bloom_b1: Bloom = "00000020000000000000000000000000000000000000000002000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008000400000000000000000000002000".into();

    let bloom_b2: Bloom = "00000000000000000000000000000000000000000000020000001000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000008000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into();

    let bloom_ba: Bloom = "00000000000000000000000000000000000000000000020000000800000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000008000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into();

    let genesis = BlockBuilder::genesis();
    let b1 = genesis.add_block_with(|| {
        BlockOptions {
            bloom: bloom_b1.clone(),
            difficulty: 9.into(),
            ..Default::default()
        }
    });
    let b2 = b1.add_block_with_bloom(bloom_b2);
    let b3 = b2.add_block_with_bloom(bloom_ba);

    let b1a = genesis.add_block_with_bloom(bloom_ba);
    let b2a = b1a.add_block_with_bloom(bloom_ba);

    let db = new_db();
    let bc = new_chain(&genesis.last().encoded(), db.clone());

    let blocks_b1 = bc.blocks_with_bloom(&bloom_b1, 0, 5);
    let blocks_b2 = bc.blocks_with_bloom(&bloom_b2, 0, 5);
    assert!(blocks_b1.is_empty());
    assert!(blocks_b2.is_empty());

    insert_block(&db, &bc, &b1.last().encoded(), vec![]);
    let blocks_b1 = bc.blocks_with_bloom(&bloom_b1, 0, 5);
    let blocks_b2 = bc.blocks_with_bloom(&bloom_b2, 0, 5);
    assert_eq!(blocks_b1, vec![1]);
    assert!(blocks_b2.is_empty());

    insert_block(&db, &bc, &b2.last().encoded(), vec![]);
    let blocks_b1 = bc.blocks_with_bloom(&bloom_b1, 0, 5);
    let blocks_b2 = bc.blocks_with_bloom(&bloom_b2, 0, 5);
    assert_eq!(blocks_b1, vec![1]);
    assert_eq!(blocks_b2, vec![2]);

    // hasn't been forked yet
    insert_block(&db, &bc, &b1a.last().encoded(), vec![]);
    let blocks_b1 = bc.blocks_with_bloom(&bloom_b1, 0, 5);
    let blocks_b2 = bc.blocks_with_bloom(&bloom_b2, 0, 5);
    let blocks_ba = bc.blocks_with_bloom(&bloom_ba, 0, 5);
    assert_eq!(blocks_b1, vec![1]);
    assert_eq!(blocks_b2, vec![2]);
    assert!(blocks_ba.is_empty());

    // fork has happend
    insert_block(&db, &bc, &b2a.last().encoded(), vec![]);
    let blocks_b1 = bc.blocks_with_bloom(&bloom_b1, 0, 5);
    let blocks_b2 = bc.blocks_with_bloom(&bloom_b2, 0, 5);
    let blocks_ba = bc.blocks_with_bloom(&bloom_ba, 0, 5);
    assert!(blocks_b1.is_empty());
    assert!(blocks_b2.is_empty());
    assert_eq!(blocks_ba, vec![1, 2]);

    // fork back
    insert_block(&db, &bc, &b3.last().encoded(), vec![]);
    let blocks_b1 = bc.blocks_with_bloom(&bloom_b1, 0, 5);
    let blocks_b2 = bc.blocks_with_bloom(&bloom_b2, 0, 5);
    let blocks_ba = bc.blocks_with_bloom(&bloom_ba, 0, 5);
    assert_eq!(blocks_b1, vec![1]);
    assert_eq!(blocks_b2, vec![2]);
    assert_eq!(blocks_ba, vec![3]);
}

#[test]
fn test_insert_unordered() {
    let bloom_b1: Bloom = "00000020000000000000000000000000000000000000000002000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008000400000000000000000000002000".into();

    let bloom_b2: Bloom = "00000000000000000000000000000000000000000000020000001000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000008000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into();

    let bloom_b3: Bloom = "00000000000000000000000000000000000000000000020000000800000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000008000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into();

    let genesis = BlockBuilder::genesis();
    let b1 = genesis.add_block_with_bloom(bloom_b1);
    let b2 = b1.add_block_with_bloom(bloom_b2);
    let b3 = b2.add_block_with_bloom(bloom_b3);
    let b1_pow_total_difficulty = genesis.last().difficulty() + b1.last().difficulty();

    let db = new_db();
    let bc = new_chain(&genesis.last().encoded(), db.clone());
    let mut batch = DBTransaction::new();
    bc.insert_unordered_block(
        &mut batch,
        &b2.last().encoded(),
        vec![],
        Some(b1_pow_total_difficulty),
        Some(U256::from(0)),
        false,
        false,
    );
    db.write_buffered(batch.clone());
    bc.commit();
    bc.insert_unordered_block(
        &mut batch,
        &b3.last().encoded(),
        vec![],
        None,
        None,
        true,
        false,
    );
    db.write_buffered(batch.clone());
    bc.commit();
    bc.insert_unordered_block(
        &mut batch,
        &b1.last().encoded(),
        vec![],
        None,
        None,
        false,
        false,
    );
    bc.commit();
    db.write(batch).unwrap();

    assert_eq!(bc.best_block_hash(), b3.last().hash());
    assert_eq!(bc.block_hash(1).unwrap(), b1.last().hash());
    assert_eq!(bc.block_hash(2).unwrap(), b2.last().hash());
    assert_eq!(bc.block_hash(3).unwrap(), b3.last().hash());

    let blocks_b1 = bc.blocks_with_bloom(&bloom_b1, 0, 3);
    let blocks_b2 = bc.blocks_with_bloom(&bloom_b2, 0, 3);
    let blocks_b3 = bc.blocks_with_bloom(&bloom_b3, 0, 3);

    assert_eq!(blocks_b1, vec![1]);
    assert_eq!(blocks_b2, vec![2]);
    assert_eq!(blocks_b3, vec![3]);
}

#[test]
fn test_best_block_update() {
    let genesis = BlockBuilder::genesis();
    let next_5 = genesis.add_blocks(5);
    let uncle = genesis.add_block_with_difficulty(9);
    let generator = BlockGenerator::new(iter::once(next_5));

    let db = new_db();
    {
        let bc = new_chain(&genesis.last().encoded(), db.clone());

        let mut batch = DBTransaction::new();
        // create a longer fork
        for block in generator {
            bc.insert_block(&mut batch, &block.encoded(), vec![]);
            bc.commit();
        }

        assert_eq!(bc.best_block_number(), 5);
        bc.insert_block(&mut batch, &uncle.last().encoded(), vec![]);
        db.write(batch).unwrap();
        bc.commit();
    }

    // re-loading the blockchain should load the correct best block.
    let bc = new_chain(&genesis.last().encoded(), db);
    assert_eq!(bc.best_block_number(), 5);
}

use rlp::*;
use super::BlockReceipts;

#[test]
fn encode_block_receipts() {
    let br = BlockReceipts::new(Vec::new());

    let mut s = RlpStream::new_list(2);
    s.append(&br);
    assert!(!s.is_finished(), "List shouldn't finished yet");
    s.append(&br);
    assert!(s.is_finished(), "List should be finished now");
    s.out();
}

#[test]
fn test_new_difficulty1() {
    let bc_pow_only = generate_dummy_blockchain(50);
    // td = pow_td if there is no pos block in chain
    // td = pow_td(0+100+200+300+...+4800+4900) * 1 = 122500
    assert_eq!(
        bc_pow_only.best_block_total_difficulty(),
        U256::from(122500)
    );

    let bc = generate_dummy_blockchain_with_pos_block(50);
    // td = pow_td * pos_td if there is pos block in chain
    // td = pow_td(0+100+400+500+...+4800+4900) * pos_td(200+300+600+700+...+4600+4700) = 3745560000
    assert_eq!(bc.best_block_total_difficulty(), U256::from(3745560000u64));
}

#[test]
fn test_new_difficulty2() {
    // genensis difficulty is 0
    let genesis = BlockBuilder::genesis();

    let db = new_db();
    let bc = new_chain(&genesis.last().encoded(), db.clone());
    assert_eq!(bc.best_block_total_difficulty(), U256::zero());

    // add 10 pow blocks with total pow difficulty 100 to simulate a blockchain
    // pow td > 0 when the first pos block come to chain.
    let a1 = genesis.add_blocks(10);
    let generator1 = BlockGenerator::new(iter::once(a1.clone()));

    let mut batch = DBTransaction::new();
    let mut difficulty = U256::zero();
    for block in generator1 {
        bc.insert_block(&mut batch, &block.encoded(), vec![]);
        bc.commit();
        difficulty = difficulty + U256::from(10);
        assert_eq!(bc.best_block_total_difficulty(), difficulty);
    }

    assert_eq!(bc.best_block_total_difficulty(), U256::from(100));

    // add a pos block and then td = 100 * 10 = 1000
    let a2 = a1.add_pos_block();
    bc.insert_block(&mut batch, &a2.last().encoded(), vec![]);
    db.write_buffered(batch.clone());
    bc.commit();
    assert_eq!(bc.best_block_total_difficulty(), U256::from(1000));

    // if a pow block come with the same block number as the pos block at moment,
    // b2 td = 100 + 10 = 110 < 1000, it will not be accepted
    let b2 = a1.add_block();
    bc.insert_block(&mut batch, &b2.last().encoded(), vec![]);
    bc.commit();
    assert_eq!(bc.best_block_total_difficulty(), U256::from(1000));

    // if a pow block come with the same block number as the pos block and 901 difficulty at moment,
    // b3 td = 100 + 901 = 1001 > 1000, it will be accepted
    let c2 = a1.add_block_with_difficulty(901);
    bc.insert_block(&mut batch, &c2.last().encoded(), vec![]);
    bc.commit();
    assert_eq!(bc.best_block_total_difficulty(), U256::from(1001));
}
