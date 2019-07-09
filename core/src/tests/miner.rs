use miner::{Miner,MinerService};
use spec::Spec;
use block::IsBlock;
use super::common::test_client::{TestBlockChainClient,EachBlockWith};


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

/// Hashrate test
use miner::external::{ExternalMiner,ExternalMinerService};
use std::thread::sleep;
use std::time::Duration;
use aion_types::{U256,H256};

fn ext_miner() -> ExternalMiner { ExternalMiner::default() }

#[test]
fn it_should_forget_old_hashrates() {
    // given
    let m = ext_miner();
    assert_eq!(m.hashrate(), U256::from(0));
    m.submit_hashrate(U256::from(10), H256::from(1));
    assert_eq!(m.hashrate(), U256::from(10));

    // when
    sleep(Duration::from_secs(3));

    // then
    assert_eq!(m.hashrate(), U256::from(0));
}

#[test]
fn should_sum_up_hashrate() {
    // given
    let m = ext_miner();
    assert_eq!(m.hashrate(), U256::from(0));
    m.submit_hashrate(U256::from(10), H256::from(1));
    assert_eq!(m.hashrate(), U256::from(10));

    // when
    m.submit_hashrate(U256::from(15), H256::from(1));
    m.submit_hashrate(U256::from(20), H256::from(2));

    // then
    assert_eq!(m.hashrate(), U256::from(35));
}
