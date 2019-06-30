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
use version::short_version;

use p2p::{ChannelBuffer, Control, Version, NODE_ID_LENGTH, Node, DISCONNECTED, IP_LENGTH, P2pMgr};
use p2p::Action;
use p2p::node::{ MAX_REVISION_LENGTH };
use super::super::net::event::{NetEvent, HANDSHAKE_DONE};

const VERSION: &str = "02";
const REVISION_PREFIX: &str = "r-";

pub fn send_activenodes_req() {
    let mut req = ChannelBuffer::new();
    req.head.ver = Version::V0.value();
    req.head.ctrl = Control::NET.value();
    req.head.action = Action::ACTIVENODESREQ.value();
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
    res.head.action = Action::ACTIVENODESRES.value();

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

pub fn send_handshake_req(node: &mut Node) {
    let local_node = P2pMgr::get_local_node();
    let mut req = ChannelBuffer::new();
    req.head.ver = Version::V0.value();
    req.head.ctrl = Control::NET.value();
    req.head.action = Action::HANDSHAKEREQ.value();

    req.body.clear();
    req.body.put_slice(&local_node.node_id);

    let mut net_id = [0; 4];
    BigEndian::write_u32(&mut net_id, local_node.net_id);

    req.body.put_slice(&net_id);
    req.body.put_slice(&local_node.ip_addr.ip);
    let mut port = [0; 4];
    BigEndian::write_u32(&mut port, local_node.ip_addr.port);
    req.body.put_slice(&port);
    let mut revision = short_version();
    revision.insert_str(0, REVISION_PREFIX);
    req.body.push(revision.len() as u8);
    req.body.put_slice(revision.as_bytes());
    req.body.push((VERSION.len() / 2) as u8);
    req.body.put_slice(VERSION.as_bytes());

    req.head.len = req.body.len() as u32;

    // handshake req
    trace!(target: "net", "Net handshake req sent...");
    P2pMgr::send(node.node_hash, req.clone());
    node.inc_repeated();

    P2pMgr::update_node(node.node_hash, node);
}

pub fn handle_handshake_req(node: &mut Node, req: ChannelBuffer) {
    trace!(target: "net", "HANDSHAKEREQ received.");

    let (node_id, req_body_rest) = req.body.split_at(NODE_ID_LENGTH);
    let (mut net_id, req_body_rest) = req_body_rest.split_at(mem::size_of::<i32>());
    let peer_net_id = net_id.read_u32::<BigEndian>().unwrap_or(0);
    let local_net_id = P2pMgr::get_network_config().net_id;
    if peer_net_id != local_net_id {
        warn!(target: "net", "Invalid net id {}, should be {}.", peer_net_id, local_net_id);
        return;
    }

    let (_ip, req_body_rest) = req_body_rest.split_at(IP_LENGTH);
    let (mut port, revision_version) = req_body_rest.split_at(mem::size_of::<i32>());
    let (revision_len, rest) = revision_version.split_at(1);
    let revision_len = revision_len[0] as usize;
    let (revision, rest) = rest.split_at(revision_len);
    let (version_len, rest) = rest.split_at(1);
    let version_len = version_len[0] as usize;
    let (_version, _rest) = rest.split_at(version_len);

    node.node_id.copy_from_slice(node_id);
    node.ip_addr.port = port.read_u32::<BigEndian>().unwrap_or(30303);
    if revision_len > MAX_REVISION_LENGTH {
        node.revision[0..MAX_REVISION_LENGTH].copy_from_slice(&revision[..MAX_REVISION_LENGTH]);
    } else {
        node.revision[0..revision_len].copy_from_slice(revision);
    }

    let mut res = ChannelBuffer::new();
    let mut res_body = Vec::new();

    res.head.set_version(Version::V0);
    res.head.set_control(Control::NET);
    res.head.action = Action::HANDSHAKERES.value();
    res_body.push(1 as u8);
    let mut revision = short_version();
    revision.insert_str(0, REVISION_PREFIX);
    res_body.push(revision.len() as u8);
    res_body.put_slice(revision.as_bytes());
    res.body.put_slice(res_body.as_slice());
    res.head.set_length(res.body.len() as u32);

    let old_node_hash = node.node_hash;
    let node_id_hash = P2pMgr::calculate_hash(&node.get_node_id());
    node.node_hash = node_id_hash;
    if P2pMgr::is_connected(node_id_hash) {
        trace!(target: "net", "known node {}@{} ...", node.get_node_id(), node.get_ip_addr());
    } else {
        NetEvent::update_node_state(node, NetEvent::OnHandshakeReq);
        if let Some(socket) = P2pMgr::get_peer(old_node_hash) {
            P2pMgr::add_peer(node.clone(), &socket);
        }
    }

    P2pMgr::send(node.node_hash, res);
    P2pMgr::remove_peer(old_node_hash);
}

pub fn handle_handshake_res(node: &mut Node, req: ChannelBuffer) {
    trace!(target: "net", "HANDSHAKERES received.");

    let (_, revision) = req.body.split_at(1);
    let (revision_len, rest) = revision.split_at(1);
    let revision_len = revision_len[0] as usize;
    let (revision, _rest) = rest.split_at(revision_len);
    if revision_len > MAX_REVISION_LENGTH {
        node.revision[0..MAX_REVISION_LENGTH].copy_from_slice(&revision[..MAX_REVISION_LENGTH]);
    } else {
        node.revision[0..revision_len].copy_from_slice(revision);
    }

    NetEvent::update_node_state(node, NetEvent::OnHandshakeRes);
    P2pMgr::update_node(node.node_hash, node);
}

pub type Callback = fn(node: &mut Node, cb: ChannelBuffer);

#[derive(Clone, Copy)]
pub struct DefaultHandler {
    pub callback: Callback,
}

impl DefaultHandler {
    pub fn set_callback(&mut self, c: Callback) { self.callback = c; }
    pub fn handle(&self, node: &mut Node, cb: ChannelBuffer) { (self.callback)(node, cb); }
}