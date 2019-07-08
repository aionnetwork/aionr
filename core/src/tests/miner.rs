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
