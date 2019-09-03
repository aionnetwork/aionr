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
use node::NODE_ID_LENGTH;
use node::IP_LENGTH;
use node::Node;
use node::TempNode;
use route::VERSION;
use route::MODULE;
use route::ACTION;
use super::super::Mgr;

pub fn send(p2p: Mgr) {
    let active: Vec<Node> = p2p.get_active_nodes();
    let len: usize = active.len();
    if len > 0 {
        let random = random::<usize>() % len;
        let hash = active[random].get_hash();
        debug!(target: "p2p", "active_nodes/send:  hash {}", &hash);
        p2p.send(
            hash,
            ChannelBuffer::new1(
                VERSION::V0.value(),
                MODULE::P2P.value(),
                ACTION::ACTIVENODESREQ.value(),
                0,
            ),
        );
    }
}

pub fn receive_req(p2p: Mgr, hash: u64) {
    debug!(target: "p2p", "active_nodes/receive_req");

    let mut cb_out = ChannelBuffer::new();
    cb_out.head.ver = VERSION::V0.value();
    cb_out.head.ctrl = MODULE::P2P.value();
    cb_out.head.action = ACTION::ACTIVENODESRES.value();

    let active_nodes = p2p.get_active_nodes();
    let mut res_body = Vec::new();

    if active_nodes.len() > 0 {
        let mut active_nodes_to_send = Vec::new();
        for active_node in active_nodes.iter() {
            if active_node.hash != hash {
                active_nodes_to_send.push(active_node);
            }
        }
        if active_nodes_to_send.len() > 0 {
            res_body.push(active_nodes_to_send.len() as u8);
            for n in active_nodes_to_send.iter() {
                res_body.put_slice(&n.id);
                res_body.put_slice(&n.addr.ip);
                let mut port = [0; 4];
                BigEndian::write_u32(&mut port, n.addr.port);
                res_body.put_slice(&port);
            }
        } else {
            res_body.push(0 as u8);
        }
    } else {
        res_body.push(0 as u8);
    }

    cb_out.body.put_slice(res_body.as_slice());
    cb_out.head.len = cb_out.body.len() as u32;
    p2p.send(hash, cb_out);
}

pub fn receive_res(p2p: Mgr, hash: u64, cb_in: ChannelBuffer) {
    debug!(target: "p2p", "active_nodes/receive_res");

    let (node_count, mut rest) = cb_in.body.split_at(1);

    let nodes_count: u32 = node_count[0] as u32;

    if nodes_count > 0 {
        let mut temp_list = Vec::new();
        let (local_ip, _) = p2p.config.get_ip_and_port();

        // TODO: update node status with healthy active nodes msg
        // TODO: max active nodes filter
        for _i in 0..nodes_count {
            let (id, rest1) = rest.split_at(NODE_ID_LENGTH);
            let (ip, rest1) = rest1.split_at(IP_LENGTH);

            if local_ip.as_bytes() == ip {
                continue;
            }

            let (mut port, rest1) = rest1.split_at(mem::size_of::<u32>());
            rest = rest1;

            let mut temp = TempNode::default();
            temp.addr.ip.copy_from_slice(ip);
            temp.addr.port = port.read_u32::<BigEndian>().unwrap_or(30303);
            temp.id.copy_from_slice(id);

            // TODO: complete if should add
            temp_list.push(temp);
        }
        if let Ok(mut lock) = p2p.temp.try_lock() {
            for t in temp_list.iter() {
                lock.push_back(t.to_owned());
            }
        }
    }

    if let Ok(mut lock) = p2p.nodes.try_write() {
        if let Some(node) = lock.get_mut(&hash) {
            node.update();
        }
    }
}
