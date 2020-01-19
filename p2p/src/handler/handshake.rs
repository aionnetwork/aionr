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
use crate::ChannelBuffer;
use crate::node::MAX_REVISION_LENGTH;
use crate::node::IP_LENGTH;
use crate::node::NODE_ID_LENGTH;
use crate::node::REVISION_PREFIX;
use crate::node::convert_ip_string;
use crate::route::Action;
use crate::state::STATE;
use super::super::Mgr;

use super::{channel_buffer_template,channel_buffer_template_with_version};

//TODO: remove it
const VERSION: &str = "02";

// TODO: validate len
pub fn send(p2p: Mgr, hash: u64) {
    debug!(target: "p2p_send", "handshake/send");

    // header
    let mut req = channel_buffer_template(Action::HANDSHAKEREQ.value());

    // write id
    let (id, _) = p2p.config.get_id_and_binding();
    req.body.put_slice(id.as_bytes());

    // write net_id
    let mut net_id_bytes = [0; 4];
    BigEndian::write_u32(&mut net_id_bytes, p2p.config.net_id);
    req.body.put_slice(&net_id_bytes);

    // write ip & port
    let (ip, port) = p2p.config.get_ip_and_port();
    req.body.put_slice(&convert_ip_string(ip));
    let mut port_bytes = [0; 4];
    BigEndian::write_u32(&mut port_bytes, port);
    req.body.put_slice(&port_bytes);

    // write revision
    let mut revision = short_version();
    revision.insert_str(0, REVISION_PREFIX);
    req.body.push(revision.len() as u8);
    req.body.put_slice(revision.as_bytes());

    // write version
    req.body.push((VERSION.len() / 2) as u8);
    req.body.put_slice(VERSION.as_bytes());

    // get bodylen
    req.head.len = req.body.len() as u32;

    // send
    p2p.send(hash, req);
}

/// 1. decode handshake msg
/// 2. validate and prove incoming connection to active
/// 3. acknowledge sender if it is proved
/// 4. update new hash
pub fn receive_req(p2p: Mgr, hash: u64, cb_in: ChannelBuffer) {
    debug!(target: "p2p_req", "handshake/receive_req");

    // check channelbuffer len
    if (cb_in.head.len as usize) < NODE_ID_LENGTH + 2 * mem::size_of::<i32>() + IP_LENGTH + 2 {
        debug!(target: "p2p_req", "handshake req channelbuffer length is too short" );
        return;
    }

    let (node_id, req_body_rest) = cb_in.body.split_at(NODE_ID_LENGTH);
    {
        let id_set = p2p.nodes_id.lock();
        if id_set.contains(&String::from_utf8_lossy(&node_id).to_string()) {
            return;
        }
    }
    let (mut net_id, req_body_rest) = req_body_rest.split_at(mem::size_of::<i32>());
    let peer_net_id = net_id.read_u32::<BigEndian>().unwrap_or(0);
    let local_net_id = p2p.config.net_id;
    if peer_net_id != local_net_id {
        debug!(target: "p2p_req", "Node: {:?}, invalid net id {}, should be {}.", node_id, peer_net_id, local_net_id);
        return;
    }

    let (ip, req_body_rest) = req_body_rest.split_at(IP_LENGTH);
    let (mut port, revision_version) = req_body_rest.split_at(mem::size_of::<i32>());
    let (revision_len, rest) = revision_version.split_at(1);
    let revision_len = revision_len[0] as usize;

    // check revision length
    if rest.len() < revision_len + 1 {
        debug!(target: "p2p_req", "handshake req with wrong revision length: {} rest: {}", revision_len, rest.len() );
        return;
    }

    let (revision, rest) = rest.split_at(revision_len);
    let (version_len, version) = rest.split_at(1);
    let version_len = version_len[0] as usize;

    // check version length
    if version_len * 2 != version.len() {
        debug!(target: "p2p_req", "handshake req with wrong version length" );
        return;
    }

    let nodes_read = p2p.nodes.read();
    if let Some(node_lock) = nodes_read.get(&hash) {
        let mut node = node_lock.write();
        debug!(target: "p2p_req", "inbound node state: connected -> active");
        node.id.copy_from_slice(node_id);
        let addr_ip = node.addr.ip;
        trace!(target: "p2p_req", "ip:{:?} - {:?}", addr_ip, ip);
        if ip == &[0u8; 8] {
            node.real_addr.ip.copy_from_slice(&addr_ip);
        } else {
            node.real_addr.ip.copy_from_slice(ip);
        }
        let port = port.read_u32::<BigEndian>().unwrap_or(30303);
        trace!(target: "p2p_req", "port:{} - {}", node.addr.port, port);
        node.real_addr.port = port;
        node.state = STATE::ACTIVE;
        {
            let mut id_set = p2p.nodes_id.lock();
            id_set.insert(node.get_id_string());
        }
        if revision_len > MAX_REVISION_LENGTH {
            node.revision[0..MAX_REVISION_LENGTH].copy_from_slice(&revision[..MAX_REVISION_LENGTH]);
        } else {
            node.revision[0..revision_len].copy_from_slice(revision);
        }

        let mut cb_out =
            channel_buffer_template_with_version(cb_in.head.ver, Action::HANDSHAKERES.value());
        let mut res_body = Vec::new();

        res_body.push(1 as u8);
        let mut revision = short_version();
        revision.insert_str(0, REVISION_PREFIX);
        res_body.push(revision.len() as u8);
        res_body.put_slice(revision.as_bytes());
        cb_out.body.put_slice(res_body.as_slice());
        cb_out.head.len = cb_out.body.len() as u32;

        let mut tx = node.tx.clone();
        match tx.try_send(cb_out) {
            Ok(_) => trace!(target: "p2p_req", "succeed sending handshake res"),
            Err(err) => {
                error!(target: "p2p_req", "failed sending handshake res: {:?}", err);
            }
        }
    }
}

/// 1. decode handshake res msg
/// 2. update outbound node to active
pub fn receive_res(p2p: Mgr, hash: u64, cb_in: ChannelBuffer) {
    debug!(target: "p2p_res", "handshake/receive_res");

    // check channelbuffer len
    if cb_in.head.len < 2 {
        debug!(target: "p2p_res", "handshake res channelbuffer length is too short" );
        return;
    }

    let (_, revision) = cb_in.body.split_at(1);
    let (revision_len, revision_bytes) = revision.split_at(1);
    let revision_len = revision_len[0] as usize;

    // check revision length
    if revision_len != revision_bytes.len() {
        debug!(target: "p2p_res", "handshake req with wrong revision length" );
        return;
    }

    let nodes_read = p2p.nodes.read();
    if let Some(node_lock) = nodes_read.get(&hash) {
        let mut node = node_lock.write();
        if revision_len > MAX_REVISION_LENGTH {
            node.revision[0..MAX_REVISION_LENGTH]
                .copy_from_slice(&revision_bytes[..MAX_REVISION_LENGTH]);
        } else {
            node.revision[0..revision_len].copy_from_slice(revision_bytes);
        }

        node.state = STATE::ACTIVE;
        let mut id_set = p2p.nodes_id.lock();
        id_set.insert(node.get_id_string());
    }
}
