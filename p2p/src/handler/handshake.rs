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
use std::sync::Arc;
use std::sync::RwLock;
use std::collections::HashMap;
use bytes::BufMut;
use byteorder::BigEndian;
use byteorder::ByteOrder;
use byteorder::ReadBytesExt;
use version::short_version;
use ChannelBuffer;
use node::MAX_REVISION_LENGTH;
use node::IP_LENGTH;
use node::NODE_ID_LENGTH;
use node::REVISION_PREFIX;
use node::Node;
use node::convert_ip_string;
use route::VERSION;
use route::MODULE;
use route::ACTION;
use config::Config;
use state::STATE;
use super::super::send as p2p_send;

//TODO: remove it
const VERSION: &str = "02";

// TODO: validate len
pub fn send(hash: u64, id: String, net_id: u32, ip: String, port: u32, nodes: Arc<RwLock<HashMap<u64, Node>>>) {
    debug!(target: "p2p", "handshake/send");

    // header
    let mut req = ChannelBuffer::new();
    req.head.ver = VERSION::V0.value();
    req.head.ctrl = MODULE::P2P.value();
    req.head.action = ACTION::HANDSHAKEREQ.value();
    
    // write id
    req.body.put_slice(id.as_bytes());
    
    // write net_id
    let mut net_id_bytes = [0; 4];
    BigEndian::write_u32(&mut net_id_bytes, net_id);
    req.body.put_slice(&net_id_bytes);
    
    // write ip
    req.body.put_slice(&convert_ip_string(ip));
    
    // write port
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
    p2p_send(&hash, req, nodes);
}

/// 1. decode handshake msg
/// 2. validate and prove incoming connection to active
/// 3. acknowledge sender if it is proved
pub fn receive_req(hash: u64, cb_in: ChannelBuffer, config: Arc<Config>, nodes: Arc<RwLock<HashMap<u64, Node>>>) {
    debug!(target: "p2p", "handshake/receive_req");

    let (node_id, req_body_rest) = cb_in.body.split_at(NODE_ID_LENGTH);
    let (mut net_id, req_body_rest) = req_body_rest.split_at(mem::size_of::<i32>());
    let peer_net_id = net_id.read_u32::<BigEndian>().unwrap_or(0);
    let local_net_id = config.net_id;
    if peer_net_id != local_net_id {
        warn!(target: "p2p", "invalid net id {}, should be {}.", peer_net_id, local_net_id);
        return;
    }

    let (_ip, req_body_rest) = req_body_rest.split_at(IP_LENGTH);
    let (port, revision_version) = req_body_rest.split_at(mem::size_of::<i32>());
    let (revision_len, rest) = revision_version.split_at(1);
    let revision_len = revision_len[0] as usize;
    let (revision, rest) = rest.split_at(revision_len);
    let (version_len, rest) = rest.split_at(1);
    let version_len = version_len[0] as usize;
    let (_version, _rest) = rest.split_at(version_len);

    if let Ok(mut write) = nodes.try_write(){
        if let Some(mut node) = write.remove(&hash) {
            
            node.id.copy_from_slice(node_id);
            // node.addr.port = port.read_u32::<BigEndian>().unwrap_or(30303);
            node.state = STATE::ACTIVE;
            if revision_len > MAX_REVISION_LENGTH {
                node.revision[0..MAX_REVISION_LENGTH].copy_from_slice(&revision[..MAX_REVISION_LENGTH]);
            } else {
                node.revision[0..revision_len].copy_from_slice(revision);
            }

            // due to target id updated, hash of node needs to be updated
            let new_hash = node.get_hash();
            let mut tx = node.tx.clone();
            if let None = write.insert(new_hash.clone(), node) {
                debug!(target: "p2p", "inbound node state: connected -> active");
            }

            let mut cb_out = ChannelBuffer::new();
            let mut res_body = Vec::new();
            cb_out.head.ver = VERSION::V0.value();
            cb_out.head.ctrl = MODULE::P2P.value();
            cb_out.head.action = ACTION::HANDSHAKERES.value();
            res_body.push(1 as u8);
            let mut revision = short_version();
            revision.insert_str(0, REVISION_PREFIX);
            res_body.push(revision.len() as u8);
            res_body.put_slice(revision.as_bytes());
            cb_out.body.put_slice(res_body.as_slice());
            cb_out.head.len = cb_out.body.len() as u32;
            
            println!("old hash {}", &hash);
            println!("new hash {}", &new_hash);    


            // special handle prevent read lock
            // TODO: 
            match tx.try_send(cb_out) {
                Ok(_) => trace!(target: "p2p", "succeed sending handshake res"),
                Err(err) => { 
                    error!(target: "p2p", "failed sending handshake res: {:?}", err); 
                }
            }
            // p2p_send(&new_hash, cb_out, nodes.clone());           
        }
    } 
}

/// 1. decode handshake res msg
/// 2. update outbound node to active
pub fn receive_res<'a>(hash: u64, cb_in: ChannelBuffer, nodes: Arc<RwLock<HashMap<u64, Node>>>) {
    debug!(target: "p2p", "handshake/receive_res");

    let (_, revision) = cb_in.body.split_at(1);
    let (revision_len, rest) = revision.split_at(1);
    let revision_len = revision_len[0] as usize;
    let (revision_bytes, _rest) = rest.split_at(revision_len);

    if let Ok(mut write) = nodes.try_write(){
        if let Some(mut node) = write.get_mut(&hash) {

            // TODO: math::low
            if revision_len > MAX_REVISION_LENGTH {
                node.revision[0..MAX_REVISION_LENGTH].copy_from_slice(&revision_bytes[..MAX_REVISION_LENGTH]);
            } else {
                node.revision[0..revision_len].copy_from_slice(revision_bytes);
            }
            node.state = STATE::ACTIVE;
        }
    }
}
