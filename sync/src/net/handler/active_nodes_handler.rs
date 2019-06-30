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

use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::BufMut;
use rand::prelude::*;
use std::mem;

use p2p::{ChannelBuffer, Control, Version, NODE_ID_LENGTH, Node, DISCONNECTED, IP_LENGTH, P2pMgr};
use super::super::action::NetAction;
use super::super::event::{NetEvent, HANDSHAKE_DONE};

pub struct ActiveNodesHandler;

impl ActiveNodesHandler {
    pub fn send_activenodes_req() {
        let mut req = ChannelBuffer::new();
        req.head.ver = Version::V0.value();
        req.head.ctrl = Control::NET.value();
        req.head.action = NetAction::ACTIVENODESREQ.value();
        req.head.len = 0;
        let handshaked_nodes = P2pMgr::get_nodes(HANDSHAKE_DONE);
        let handshaked_nodes_count = handshaked_nodes.len();
        if handshaked_nodes_count > 0 {
            let random_index = random::<usize>() % handshaked_nodes_count;
            let node = &handshaked_nodes[random_index];
            P2pMgr::send(node.node_hash, req.clone());
            trace!(target: "net", "send active nodes req");
        } else {
            trace!(target: "net", "Net no active node...");
        }
    }

    pub fn handle_active_nodes_req(peer_node: &mut Node) {
        trace!(target: "net", "ACTIVENODESREQ received.");

        let mut res = ChannelBuffer::new();
        let peer_node_hash = peer_node.node_hash;

        res.head.set_version(Version::V0);
        res.head.set_control(Control::NET);
        res.head.action = NetAction::ACTIVENODESRES.value();

        let active_nodes = P2pMgr::get_nodes(HANDSHAKE_DONE);
        let mut res_body = Vec::new();
        let active_nodes_count = active_nodes.len();

        if active_nodes_count > 1 {
            let mut active_nodes_to_send = Vec::new();
            for node in active_nodes.iter() {
                if node.node_hash != peer_node.node_hash && peer_node.ip_addr.ip != node.ip_addr.ip
                {
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
        res.head.set_length(res.body.len() as u32);

        NetEvent::update_node_state(peer_node, NetEvent::OnActiveNodesReq);
        P2pMgr::update_node(peer_node_hash, peer_node);
        P2pMgr::send(peer_node_hash, res);
    }

    pub fn handle_active_nodes_res(peer_node: &mut Node, req: ChannelBuffer) {
        trace!(target: "net", "ACTIVENODESRES received.");

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
                node.state_code = DISCONNECTED;
                node.node_hash = P2pMgr::calculate_hash(&node.get_node_id());

                let local_node_ip = P2pMgr::get_local_node().ip_addr.ip;
                let local_node_ip_hash = P2pMgr::calculate_hash(&local_node_ip);
                let peer_node_ip_hash = P2pMgr::calculate_hash(&peer_node.ip_addr.ip);
                let node_ip_hash = P2pMgr::calculate_hash(&node.ip_addr.ip);

                if local_node_ip_hash != node_ip_hash && peer_node_ip_hash != node_ip_hash {
                    node_list.push(node);
                }

                rest = next;
            }
        }

        for n in node_list.iter() {
            match P2pMgr::get_node(n.node_hash) {
                Some(_) => {}
                None => {
                    P2pMgr::add_node(n.clone());
                }
            }
        }
        NetEvent::update_node_state(peer_node, NetEvent::OnActiveNodesRes);
        P2pMgr::update_node(peer_node_hash, peer_node);
    }
}
