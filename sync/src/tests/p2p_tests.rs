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

use p2p::*;
use std::{thread, time};
use std::time::Duration;

use super::common::*;

fn handle(node: &mut Node, req: ChannelBuffer) {
    println!("handle msg node: {}, msg: {}", node, req);
}

#[test]
fn test_create_server() {
    let net_config = get_network_config();

    P2pMgr::enable(net_config);
    let server_addr = String::from("127.0.0.1:30000");
    P2pMgr::create_server(server_addr.clone(), handle);
    let peer_node = Node::new_with_addr(server_addr.clone().parse().unwrap());
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
    P2pMgr::disable();
}

#[test]
fn test_connection() {
    let net_config = get_network_config();

    P2pMgr::enable(net_config);
    thread::sleep(Duration::from_secs(8));
    P2pMgr::disable();
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
    assert_eq!(node.ip_addr.port, 30309);
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
    assert_eq!(peer_node.ip_addr.port, 30309);
    assert_eq!(peer_node.is_over_repeated_threshold(), false);
    P2pMgr::reset();
}

#[test]
fn test_nodes_tablet() {
    let net_config = get_network_config();

    P2pMgr::enable(net_config);
    for i in 0..64 {
        let mut peer_node = Node::new_with_addr(format!("10.1.1.{}:30303", i).parse().unwrap());
        peer_node.node_hash = P2pMgr::calculate_hash(&peer_node.get_ip_addr());
        P2pMgr::add_node(peer_node);
    }

    let peer_node_count = P2pMgr::get_all_nodes_count();
    assert_eq!(peer_node_count, 64);

    let ip_addr = "10.1.1.22:30303".to_string();
    let node_hash = P2pMgr::calculate_hash(&ip_addr);
    P2pMgr::remove_peer(node_hash);

    let peer_node_count = P2pMgr::get_all_nodes_count();
    assert_eq!(peer_node_count, 63);
    P2pMgr::reset();
}
