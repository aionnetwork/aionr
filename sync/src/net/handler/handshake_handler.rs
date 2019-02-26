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
use std::mem;

use super::super::action::NetAction;
use super::super::event::NetEvent;
use p2p::*;
use version::short_version;

const VERSION: &str = "02";
const REVISION_PREFIX: &str = "r-";

pub struct HandshakeHandler;

impl HandshakeHandler {
    pub fn send_handshake_req(node: &mut Node) {
        let local_node = P2pMgr::get_local_node();
        let mut req = ChannelBuffer::new();
        req.head.ver = Version::V0.value();
        req.head.ctrl = Control::NET.value();
        req.head.action = NetAction::HANDSHAKEREQ.value();

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

        if req.body.len() < NODE_ID_LENGTH + IP_LENGTH + 2 * mem::size_of::<i32>() + 2 {
            warn!(target: "net", "Node {}@{} removed: Invalid handshake request length!!", node.get_node_id(), node.get_ip_addr());
            P2pMgr::remove_peer(node.node_hash);
            return;
        }
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

        if rest.len() < revision_len + 1 {
            warn!(target: "net", "Node {}@{} removed: Invalid node revision length!!", node.get_node_id(), node.get_ip_addr());
            P2pMgr::remove_peer(node.node_hash);
            return;
        }

        let (revision, rest) = rest.split_at(revision_len);
        let (version_len, rest) = rest.split_at(1);
        let version_len = version_len[0] as usize;

        if rest.len() < version_len {
            warn!(target: "net", "Node {}@{} removed: Invalid node version length!!", node.get_node_id(), node.get_ip_addr());
            P2pMgr::remove_peer(node.node_hash);
            return;
        }

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
        res.head.action = NetAction::HANDSHAKERES.value();
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
            info!(target: "net", "known node {}@{} ...", node.get_node_id(), node.get_ip_addr());
            P2pMgr::remove_peer(old_node_hash);
        } else {
            NetEvent::update_node_state(node, NetEvent::OnHandshakeReq);
            if let Some(socket) = P2pMgr::get_peer(old_node_hash) {
                P2pMgr::remove_peer(old_node_hash);
                P2pMgr::add_peer(node.clone(), socket);
            }
        }
        node.inc_reputation(1);

        P2pMgr::send(node.node_hash, res);
    }

    pub fn handle_handshake_res(node: &mut Node, req: ChannelBuffer) {
        trace!(target: "net", "HANDSHAKERES received.");

        if req.body.len() < 2 {
            warn!(target: "net", "Node {}@{} removed: Invalid handshake response length!!", node.get_node_id(), node.get_ip_addr());
            P2pMgr::remove_peer(node.node_hash);
            return;
        }

        let (_, revision) = req.body.split_at(1);
        let (revision_len, rest) = revision.split_at(1);
        let revision_len = revision_len[0] as usize;

        if rest.len() < revision_len {
            warn!(target: "net", "Node {}@{} removed: Invalid node revision length!!", node.get_node_id(), node.get_ip_addr());
            P2pMgr::remove_peer(node.node_hash);
            return;
        }

        let (revision, _rest) = rest.split_at(revision_len);
        if revision_len > MAX_REVISION_LENGTH {
            node.revision[0..MAX_REVISION_LENGTH].copy_from_slice(&revision[..MAX_REVISION_LENGTH]);
        } else {
            node.revision[0..revision_len].copy_from_slice(revision);
        }
        node.inc_reputation(10);
        NetEvent::update_node_state(node, NetEvent::OnHandshakeRes);
        P2pMgr::update_node(node.node_hash, node);
    }
}
