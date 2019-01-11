/*******************************************************************************
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

use acore::client::{BlockChainClient, BlockId, ChainNotify};
use aion_types::H256;
use p2p::P2pMgr;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use super::common::*;
use sync::storage::SyncStorage;
use sync::*;

#[test]
fn benchtest_sync_mainnet() {
    let test_spec = new_spec();
    let client = get_client(&test_spec);

    let sync_config = SyncConfig::default();
    let net_config = get_network_config();

    let sync = Sync::get_instance(Params {
        config: sync_config,
        client: client.clone() as Arc<BlockChainClient>,
        network_config: net_config,
    });

    let (sync_provider, network_manager, _chain_notify) = (
        sync.clone() as Arc<SyncProvider>,
        sync.clone() as Arc<NetworkManager>,
        sync.clone() as Arc<ChainNotify>,
    );

    let start_time = SystemTime::now();

    network_manager.start_network();

    sync_provider.enode();
    sync_provider.status();
    sync_provider.peers();

    SyncStorage::set_synced_block_number(0);
    println!(
        "synced_block_number: {}",
        SyncStorage::get_synced_block_number()
    );

    while SyncStorage::get_synced_block_number() < 999 {
        thread::sleep(Duration::from_secs(1));
        // client.import_verified_blocks();
        client.flush_queue();
        let active_nodes = P2pMgr::get_nodes(1 << 3);
        let synced_block_number = client.chain_info().best_block_number;

        SyncStorage::set_synced_block_number(synced_block_number);
        println!("==================== Sync Statics ====================");
        println!(
            "Best block number: {}",
            SyncStorage::get_synced_block_number()
        );
        println!(
            "Total/Connected/Active peers: {}/{}/{}",
            P2pMgr::get_all_nodes_count(),
            P2pMgr::get_nodes_count(1),
            active_nodes.len()
        );
        println!("Address\t\t\tSeed\tBlock No.\tSynced No.\tMode\tLQN\tLQT");
        for node in active_nodes.iter() {
            let duration = node.last_request_timestamp.elapsed().unwrap();
            println!(
                "{}\t{}\t{}\t\t{}\t\t{}\t{}\t{:#?}",
                node.get_ip_addr(),
                node.is_from_boot_list,
                node.best_block_num,
                node.synced_block_num,
                node.mode,
                node.last_request_num,
                duration
            );
        }
    }

    // network_manager.stop_network();

    let duration = start_time.elapsed().unwrap();

    println!(
        "[benchtest_sync_mainnet] Duration of sync 1000 blocks(ms): {:#?}",
        duration.subsec_millis() as u64 + duration.as_secs() * 1000
    );
    assert!(duration < Duration::from_secs(110));

    let chain_info = client.chain_info();
    assert!(chain_info.best_block_number >= 1000);
    let block_1000 = client.block(BlockId::Number(1000)).unwrap();
    assert!(
        block_1000.hash()
            == H256::from("0x765baf520b24fb81f95d2f7f9fa28069a203b372f66401f947c5e5a62735bb22")
    );
}

#[test]
fn benchtest_sync_storage_get_client() {
    init_sync_storage();
    let start_time = SystemTime::now();

    let mut threads = Vec::new();
    for _ in 0..100 {
        let t = thread::spawn(|| {
            for _ in 0..1000 {
                SyncStorage::get_block_chain();
            }
        });
        threads.push(t);
    }
    for t in threads {
        t.join().expect("thread failed");
    }
    let duration = start_time.elapsed().unwrap();

    println!(
        "[benchtest_sync_storage_get_chain_info] Duration of 100000 queries(ms): {:#?}",
        duration.subsec_millis() as u64 + duration.as_secs() * 1000
    );

    assert!(duration < Duration::from_secs(1));
}

#[test]
fn benchtest_sync_storage_get_block_chain() {
    init_sync_storage();
    let start_time = SystemTime::now();

    let mut threads = Vec::new();
    for _ in 0..100 {
        let t = thread::spawn(|| {
            for _ in 0..1000 {
                SyncStorage::get_block_chain();
            }
        });
        threads.push(t);
    }
    for t in threads {
        t.join().expect("thread failed");
    }
    let duration = start_time.elapsed().unwrap();

    println!(
        "[benchtest_sync_storage_get_chain_info] Duration of 100000 queries(ms): {:#?}",
        duration.subsec_millis() as u64 + duration.as_secs() * 1000
    );
    assert!(duration < Duration::from_secs(1));
}

#[test]
fn benchtest_sync_storage_get_chain_info() {
    init_sync_storage();
    let test_spec = new_spec();
    let client = get_client(&test_spec);
    SyncStorage::init(client.clone() as Arc<BlockChainClient>);

    let start_time = SystemTime::now();

    let mut threads = Vec::new();
    for _ in 0..100 {
        let t = thread::spawn(|| {
            for _ in 0..1000 {
                SyncStorage::get_chain_info();
            }
        });
        threads.push(t);
    }
    for t in threads {
        t.join().expect("thread failed");
    }
    let duration = start_time.elapsed().unwrap();

    println!(
        "[benchtest_sync_storage_get_chain_info] Duration of 100000 queries(ms): {:#?}",
        duration.subsec_millis() as u64 + duration.as_secs() * 1000
    );
    assert!(duration < Duration::from_secs(1));
}

#[test]
fn benchtest_sync_storage_synced_block_number() {
    init_sync_storage();
    let start_time = SystemTime::now();

    let mut threads = Vec::new();
    for i in 0..500 {
        let t = thread::spawn(move || {
            for j in 0..1000 {
                SyncStorage::set_synced_block_number(i * j as u64);
                SyncStorage::get_synced_block_number();
            }
        });
        threads.push(t);
    }
    for t in threads {
        t.join().expect("thread failed");
    }
    let duration = start_time.elapsed().unwrap();

    println!(
        "[benchtest_sync_storage_get_synced_block_number] Duration of 500000 sets/gets: {:#?}",
        duration.subsec_millis() as u64 + duration.as_secs() * 1000
    );
    assert!(duration < Duration::from_secs(1));
}
