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
//use node::IpAddr;
use node::TempNode;
//use event::Event;
use route::VERSION;
use route::MODULE;
use route::ACTION;
use super::super::Mgr as P2p;
//use super::super::calculate_hash;

pub fn send(p2p: P2p) {
    debug!(target: "p2p", "active_nodes.rs/send");
    let nodes: Vec<Node> = p2p.get_active_nodes();;
    let len: usize = nodes.len();
    if nodes.len() > 0 {
        let random = random::<usize>() % len;
        let hash = nodes[random].hash.clone();
        p2p.send(
            hash, 
            ChannelBuffer::new1(
                VERSION::V0.value(), 
                MODULE::P2P.value(), 
                ACTION::ACTIVENODESREQ.value(), 
                0
            )
        );
    }
}

pub fn receive_req(p2p: P2p, node: &mut Node) {
    debug!(target: "p2p", "active_noreceive_req");

    let mut res = ChannelBuffer::new();
    let hash = node.hash;

    res.head.ver = VERSION::V0.value();
    res.head.ctrl = MODULE::P2P.value();
    res.head.action = ACTION::ACTIVENODESRES.value();

    let active_nodes = p2p.get_active_nodes();
    let mut res_body = Vec::new();

    if active_nodes.len() > 0 {
        let mut active_nodes_to_send = Vec::new();
        for active_node in active_nodes.iter() {
            if active_node.hash != node.hash && node.addr.ip != node.addr.ip {
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
    res.body.put_slice(res_body.as_slice());
    res.head.len = res.body.len() as u32;
    p2p.send(node.hash, res);
}

pub fn receive_res(p2p: P2p, node: &mut Node, req: ChannelBuffer) {
    debug!(target: "p2p", "active_nodes/receive_res");

    let peer_node_hash = node.hash;
    let (node_count, rest) = req.body.split_at(1);
    let mut temp_list = Vec::new();
    let mut rest = rest;
    if node_count[0] > 0 {

        // TODO: max for check        
        for _i in 0..node_count[0] as u32 {
            
            let (id, rest) = rest.split_at(NODE_ID_LENGTH);
            let (ip, rest) = rest.split_at(IP_LENGTH);
            let (mut port, rest) = rest.split_at(mem::size_of::<u32>());

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
}