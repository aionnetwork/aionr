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

extern crate sync;
extern crate acore;
extern crate aion_types;
extern crate acore_io;
extern crate db;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};
use acore::client::{BlockChainClient, BlockId, ChainNotify, Client, ClientConfig};
use acore::spec::Spec;
use acore::db::DB_NAMES;
use acore::miner::Miner;
use acore_io::IoChannel;
use aion_types::H256;
use sync::p2p::P2pMgr;
use sync::p2p::NetworkConfig;
use sync::sync::SyncConfig;
use sync::sync::storage::SyncStorage;
use sync::sync::SyncProvider;
use sync::sync::NetworkManager;
use sync::sync::Params;
use sync::sync::Sync;
use db::KeyValueDB;
use db::MockDbRepository;

fn new_spec() -> Spec {
    load(
        &"$HOME/.aion/cache_".into(),
        include_bytes!("../../resources/mainnet.json"),
    )
}

fn new_db() -> Arc<KeyValueDB> {
    let mut db_configs = Vec::new();
    for db_name in DB_NAMES.to_vec() {
        db_configs.push(db_name.into());
    }
    Arc::new(MockDbRepository::init(db_configs))
}

fn get_network_config() -> NetworkConfig {
    let mut net_config = NetworkConfig::default();
    net_config.boot_nodes.push(String::from(
        "p2p://c33d2207-729a-4584-86f1-e19ab97cf9ce@51.144.42.220:30303",
    ));
    net_config.boot_nodes.push(String::from(
        "p2p://c33d302f-216b-47d4-ac44-5d8181b56e7e@52.231.187.227:30303",
    ));
    net_config.boot_nodes.push(String::from(
        "p2p://c33d4c07-6a29-4ca6-8b06-b2781ba7f9bf@191.232.164.119:30303",
    ));
    net_config.boot_nodes.push(String::from(
        "p2p://c39d0a10-20d8-49d9-97d6-284f88da5c25@13.92.157.19:30303",
    ));
    net_config.boot_nodes.push(String::from(
        "p2p://c38d2a32-20d8-49d9-97d6-284f88da5c83@40.78.84.78:30303",
    ));
    net_config.boot_nodes.push(String::from(
        "p2p://c37d6b45-20d8-49d9-97d6-284f88da5c51@104.40.182.54:30303",
    ));
    net_config.boot_nodes.push(String::from(
        "p2p://c36d4208-fe4b-41fa-989b-c7eeafdffe72@35.208.215.219:30303",
    ));

    net_config.local_node =
        String::from("p2p://00000000-6666-0000-0000-000000000000@0.0.0.0:30309");
    net_config.net_id = 256;
    net_config.sync_from_boot_nodes_only = false;
    net_config
}

pub fn init_sync_storage() {
    let spec = new_spec();
    let client = get_client(&spec);
    SyncStorage::init(client.clone() as Arc<BlockChainClient>);
}

fn load<'a>(params: &'a String, b: &[u8]) -> Spec {
    match params.into() {
        Some(params) => Spec::load(params, b),
        None => Spec::load(&::std::env::temp_dir(), b),
    }
    .expect("chain spec is invalid")
}

pub fn get_client(spec: &Spec) -> Arc<Client> {
    let channel = IoChannel::disconnected();
    Client::new(
        ClientConfig::default(),
        &spec,
        new_db(),
        Arc::new(Miner::with_spec(&spec)),
        channel.clone(),
    )
    .unwrap()
}

#[test]
fn benchtest_sync_mainnet() {
    let target = 99;

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

    while SyncStorage::get_synced_block_number() < target {
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
    // assert!(duration < Duration::from_secs(110));

    let chain_info = client.chain_info();
    assert!(chain_info.best_block_number >= target);
    //    let block_99 = client.block(BlockId::Number(target)).unwrap();
    //    assert!(
    //        block_99.hash()
    //            == H256::from("0x765baf520b24fb81f95d2f7f9fa28069a203b372f66401f947c5e5a62735bb22")
    //    );
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
