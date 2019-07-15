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
use super::*;
use std::{thread, time};
use tokio::runtime::Runtime;

fn handle(node: &mut Node, req: ChannelBuffer) {
    println!("handle msg node: {}, msg: {:?}", node, req);
}

pub fn get_network_config() -> NetworkConfig {
    let mut net_config = NetworkConfig::new();
    net_config.boot_nodes.push(String::from(
        "p2p://c33d1066-8c7e-496c-9c4e-c89318280274@13.92.155.115:30303",
    ));
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
        "p2p://c33d5a94-20d8-49d9-97d6-284f88da5c21@13.89.244.125:30303",
    ));
    net_config.boot_nodes.push(String::from(
        "p2p://741b979e-6a06-493a-a1f2-693cafd37083@66.207.217.190:30303",
    ));

    net_config.local_node =
        String::from("p2p://00000000-6666-0000-0000-000000000000@0.0.0.0:30303");
    net_config.net_id = 256;
    net_config.sync_from_boot_nodes_only = false;
    net_config
}

#[test]
fn test_create_server() {
    let rt = Runtime::new().unwrap();
    let executor_handle = rt.executor();
    let net_config = get_network_config();

    P2pMgr::enable(net_config);
    let server_addr = String::from("127.0.0.1:30000");
    P2pMgr::create_server(&executor_handle, &server_addr, handle);
    let peer_node = Node::new_with_addr(server_addr.parse().unwrap());
    P2pMgr::create_client(peer_node, handle);
    let mut value = server_addr;
    let local_ip = P2pMgr::get_local_node().ip_addr.get_ip();
    value.push_str(&local_ip);
    let node_hash = P2pMgr::calculate_hash(&value);

    if let Some(peer_node) = P2pMgr::get_node(node_hash) {
        let msg = ChannelBuffer::new();
        P2pMgr::send(peer_node.node_hash, msg);
    }
    thread::sleep(time::Duration::from_millis(2000));
    rt.shutdown_now();
}

#[test]
fn test_connection() {
    let net_config = get_network_config();

    P2pMgr::enable(net_config);
}

#[test]
fn test_load_boot_nodes() {
    let net_config = get_network_config();

    P2pMgr::enable(net_config);

    let node_hash = 666;
    let address = "66.66.66.66:8888";
    let mut node = P2pMgr::get_local_node().clone();
    for _ in 0..10 {
        node.inc_repeated();
    }
    assert_eq!(node.ip_addr.port, 30303);
    assert_eq!(node.is_over_repeated_threshold(), true);

    node.node_hash = node_hash;
    P2pMgr::add_node(node.clone());
    println!("node: {}", node);

    let mut peer_node = P2pMgr::get_node(node_hash).unwrap();
    peer_node.set_ip_addr(address.parse().unwrap());
    assert_eq!(peer_node.get_ip_addr(), "66.66.66.66:8888".to_string());
    assert_eq!(peer_node.ip_addr.port, 8888);
    println!("peer node: {}", peer_node);
    assert_eq!(peer_node.is_over_repeated_threshold(), true);

    node.reset_repeated();
    P2pMgr::update_node(node_hash, &mut node);
    peer_node = P2pMgr::get_node(node_hash).unwrap();
    assert_eq!(peer_node.ip_addr.port, 30303);
    assert_eq!(peer_node.is_over_repeated_threshold(), false);
}

#[test]
fn test_nodes_tablet() {
    let net_config = get_network_config();

    P2pMgr::enable(net_config);
    for i in 0..66 {
        let mut peer_node = Node::new_with_addr(format!("10.1.1.{}:30303", i).parse().unwrap());
        peer_node.node_hash = P2pMgr::calculate_hash(&peer_node.get_ip_addr());
        P2pMgr::add_node(peer_node);
    }

    let peer_node_count = P2pMgr::get_all_nodes_count();
    assert_eq!(peer_node_count, 64);

    let ip_addr = "10.1.1.22:30303".to_string();
    let node_hash = P2pMgr::calculate_hash(&ip_addr);
    let peer_node = P2pMgr::remove_peer(node_hash).unwrap();

    let peer_node_count = P2pMgr::get_all_nodes_count();
    assert_eq!(peer_node_count, 63);
    assert_eq!(peer_node.get_ip_addr(), ip_addr);
    P2pMgr::reset();
}
