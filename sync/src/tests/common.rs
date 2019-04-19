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

use acore::client::{Client, ClientConfig};
use acore::db;
use acore::miner::Miner;
use acore::spec::Spec;
use acore_io::IoChannel;
use kvdb::{KeyValueDB, MockDbRepository};
use std::sync::Arc;
use acore::client::header_chain::HeaderChain;
use std::path::Path;

use p2p::NetworkConfig;
use sync::storage::SyncStorage;

fn load<'a>(params: &'a String, b: &[u8]) -> Spec {
    match params.into() {
        Some(params) => Spec::load(params, b),
        None => Spec::load(&::std::env::temp_dir(), b),
    }
    .expect("chain spec is invalid")
}

pub fn new_spec() -> Spec {
    load(
        &"$HOME/.aion/cache_".into(),
        include_bytes!("../../../core/res/aion/mainnet.json"),
    )
}

fn new_db() -> Arc<KeyValueDB> {
    let mut db_configs = Vec::new();
    for db_name in db::DB_NAMES.to_vec() {
        db_configs.push(db_name.into());
    }
    Arc::new(MockDbRepository::init(db_configs))
}

pub fn get_network_config() -> NetworkConfig {
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
        "p2p://741b979e-6a06-493a-a1f2-693cafd37083@66.207.217.190:30303",
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

    net_config.local_node =
        String::from("p2p://00000000-6666-0000-0000-000000000000@0.0.0.0:30309");
    net_config.net_id = 256;
    net_config.sync_from_boot_nodes_only = false;
    net_config
}

pub fn init_sync_storage(path: &str) {
    let spec = new_spec();
    let client = get_client(&spec);
    let header_chain = get_header_chain(path, &spec);
    SyncStorage::init(client.clone(), header_chain);
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

pub fn get_header_chain(path: &str, spec: &Spec) -> Arc<HeaderChain> {
    let header_chain = HeaderChain::new(&Path::new(path), spec).unwrap();
    return Arc::new(header_chain);
}

pub fn remove_test_db(path: &str) {
    let path = Path::new(path);
    let _ = ::std::fs::remove_dir_all(path);
}
