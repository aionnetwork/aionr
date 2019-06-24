use aion_types::U256;
use keychain;
use rustc_hex::FromHex;
use transaction::transaction_queue::PrioritizationStrategy;
use transaction::Transaction;
use transaction::Action;
use client::{BlockChainClient, EachBlockWith, TestBlockChainClient};
use miner::MinerService;
use tests::helpers::generate_dummy_client;

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
            reseal_on_external_tx: false,
            reseal_on_own_tx: true,
            reseal_min_period: Duration::from_secs(5),
            reseal_max_period: Duration::from_secs(120),
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
    assert!(miner.prepare_work_sealing(&client));
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

#[test]
fn internal_seals_without_work() {
    let spec = Spec::new_instant();
    let mut miner = Miner::with_spec(&spec);
    miner.set_minimal_gas_price(0.into());

    let client = generate_dummy_client(2);

    assert!(
        miner
            .import_external_transactions(&*client, vec![transaction().into()])
            .pop()
            .unwrap()
            .is_ok()
    );
    miner.update_transaction_pool(&*client, true);
    miner.update_sealing(&*client);
    client.flush_queue();
    assert!(miner.pending_block(0).is_none());
    assert_eq!(client.chain_info().best_block_number, 3 as BlockNumber);

    assert!(
        miner
            .import_own_transaction(
                &*client,
                PendingTransaction::new(transaction().into(), None)
            )
            .is_ok()
    );
    miner.update_transaction_pool(&*client, true);
    miner.update_sealing(&*client);
    client.flush_queue();
    assert!(miner.pending_block(0).is_none());
    assert_eq!(client.chain_info().best_block_number, 4 as BlockNumber);
}