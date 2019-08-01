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

use std::mem;
use bytes::BufMut;
use byteorder::BigEndian;
use byteorder::ByteOrder;
use rand::random;
use byteorder::ReadBytesExt;
use ChannelBuffer;
use Node;
use IP_LENGTH;
use NODE_ID_LENGTH;
use event::Event;
use route::VERSION;
use route::MODULE;
use route::ACTION;
use states::STATE::HANDSHAKEDONE;
use states::STATE::DISCONNECTED;
use super::super::send as p2p_send;
use super::super::get_nodes;
use super::super::get_node;
use super::super::get_local_node;
use super::super::add_node;
use super::super::update_node;
use super::super::calculate_hash;

pub fn send() {
    debug!(target: "p2p", "active_nodes.rs/send");
    let handshaked_nodes = get_nodes(HANDSHAKEDONE.value());
    let handshaked_nodes_count = handshaked_nodes.len();
    if handshaked_nodes_count > 0 {
        let random_index = random::<usize>() % handshaked_nodes_count;
        let node = &handshaked_nodes[random_index];
        p2p_send(
            node.node_hash, 
            ChannelBuffer::new1(
                VERSION::V0.value(), 
                MODULE::P2P.value(), 
                ACTION::ACTIVENODESREQ.value(), 
                0
            )
        );
    }
}

pub fn receive_req(peer_node: &mut Node) {
    debug!(target: "p2p", "active_nodes/receive_req");

    let mut res = ChannelBuffer::new();
    let peer_node_hash = peer_node.node_hash;

    res.head.ver = VERSION::V0.value();
    res.head.ctrl = MODULE::P2P.value();
    res.head.action = ACTION::ACTIVENODESRES.value();

    let active_nodes = get_nodes(HANDSHAKEDONE.value());
    let mut res_body = Vec::new();
    let active_nodes_count = active_nodes.len();

    if active_nodes_count > 1 {
        let mut active_nodes_to_send = Vec::new();
        for node in active_nodes.iter() {
            if node.node_hash != peer_node.node_hash && peer_node.ip_addr.ip != node.ip_addr.ip {
                active_nodes_to_send.push(node);
            }
        }
        if active_nodes_to_send.len() > 0 {
            res_body.push(active_nodes_to_send.len() as u8);
            for n in active_nodes_to_send.iter() {
                res_body.put_slice(&n.node_id);
                res_body.put_slice(&n.ip_addr.ip);
                let mut port = [0; 4];
                BigEndian::write_u32(&mut port, n.ip_addr.port);
                res_body.put_slice(&port);
            }
        } else {
            res_body.push(0 as u8);
        }
    } else {
        res_body.push(0 as u8);
    }
    res.body.put_slice(res_body.as_slice());
    res.head.len = res.body.len() as u32;

    Event::update_node_state(peer_node, Event::OnActiveNodesReq);
    update_node(peer_node_hash, peer_node);
    p2p_send(peer_node_hash, res);
}

pub fn receive_res(peer_node: &mut Node, req: ChannelBuffer) {
    debug!(target: "p2p", "active_nodes/receive_res");

    let peer_node_hash = peer_node.node_hash;
    let (node_count, rest) = req.body.split_at(1);
    let mut node_list = Vec::new();
    let mut rest = rest;
    if node_count[0] > 0 {
        for _i in 0..node_count[0] as u32 {
            let mut node = Node::new();

            let (node_id, rest_body) = rest.split_at(NODE_ID_LENGTH);
            let (ip, rest_body) = rest_body.split_at(IP_LENGTH);
            let (mut port, next) = rest_body.split_at(mem::size_of::<u32>());

            node.ip_addr.ip.copy_from_slice(ip);
            node.ip_addr.port = port.read_u32::<BigEndian>().unwrap_or(30303);
            node.node_id.copy_from_slice(node_id);
            node.state_code = DISCONNECTED as u32;
            node.node_hash = calculate_hash(&node.get_node_id());

            let local_node_ip = get_local_node().ip_addr.ip;
            let local_node_ip_hash = calculate_hash(&local_node_ip);
            let peer_node_ip_hash = calculate_hash(&peer_node.ip_addr.ip);
            let node_ip_hash = calculate_hash(&node.ip_addr.ip);

            if local_node_ip_hash != node_ip_hash && peer_node_ip_hash != node_ip_hash {
                node_list.push(node);
            }

            rest = next;
        }
    }

    for n in node_list.iter() {
        match get_node(n.node_hash) {
            Some(_) => {}
            None => {
                add_node(n.clone());
            }
        }
    }
    Event::update_node_state(peer_node, Event::OnActiveNodesRes);
    update_node(peer_node_hash, peer_node);
}
