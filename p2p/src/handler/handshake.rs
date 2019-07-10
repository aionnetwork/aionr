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
use byteorder::ReadBytesExt;
use version::short_version;
use P2pMgr;
use ChannelBuffer;
use Node;
use Event;
use MAX_REVISION_LENGTH;
use IP_LENGTH;
use NODE_ID_LENGTH;
use REVISION_PREFIX;
use route::VERSION;
use route::MODULE;
use route::ACTION;

//TODO: remove it
const VERSION: &str = "02";

pub fn send(node: &mut Node) {
    let local_node = P2pMgr::get_local_node();
    let mut req = ChannelBuffer::new();
    req.head.ver = VERSION::V0.value();
    req.head.ctrl = MODULE::P2P.value();
    req.head.action = ACTION::HANDSHAKEREQ.value();

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

pub fn receive_req(node: &mut Node, req: ChannelBuffer) {
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

    res.head.ver = VERSION::V0.value();
    res.head.ctrl = MODULE::P2P.value();
    res.head.action = ACTION::HANDSHAKERES.value();
    res_body.push(1 as u8);
    let mut revision = short_version();
    revision.insert_str(0, REVISION_PREFIX);
    res_body.push(revision.len() as u8);
    res_body.put_slice(revision.as_bytes());
    res.body.put_slice(res_body.as_slice());
    res.head.len = res.body.len() as u32;

    let old_node_hash = node.node_hash;
    let node_id_hash = P2pMgr::calculate_hash(&node.get_node_id());
    node.node_hash = node_id_hash;
    if P2pMgr::is_connected(node_id_hash) {
        trace!(target: "net", "known node {}@{} ...", node.get_node_id(), node.get_ip_addr());
    } else {
        Event::update_node_state(node, Event::OnHandshakeReq);
        if let Some(socket) = P2pMgr::get_peer(old_node_hash) {
            P2pMgr::add_peer(node.clone(), &socket);
        }
    }

    P2pMgr::send(node.node_hash, res);
    P2pMgr::remove_peer(old_node_hash);
}

pub fn receive_res(node: &mut Node, req: ChannelBuffer) {
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

    Event::update_node_state(node, Event::OnHandshakeRes);
    P2pMgr::update_node(node.node_hash, node);
}