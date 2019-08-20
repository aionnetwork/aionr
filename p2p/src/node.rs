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

use std::fmt;
use std::time::SystemTime;
use std::net::SocketAddr;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use uuid::Uuid;
use futures::sync::mpsc;
use aion_types::H256;
use super::msg::*;
use super::state::STATE;

const EMPTY_ID: &str = "00000000-0000-0000-0000-000000000000";

pub const HEADER_LENGTH: usize = 8;
pub const NODE_ID_LENGTH: usize = 36;
pub const PROTOCOL_LENGTH: usize = 6;
pub const MAX_REVISION_LENGTH: usize = 24;
pub const REVISION_PREFIX: &str = "r-";
pub const IP_LENGTH: usize = 8;
pub const DIFFICULTY_LENGTH: usize = 16;

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct IpAddr {
    pub ip: [u8; 8],
    pub port: u32,
}

impl IpAddr {
    pub fn new() -> IpAddr {
        IpAddr {
            ip: [0; 8],
            port: 0,
        }
    }
    pub fn new1(ip: [u8; 8], port: u32) -> IpAddr {
        IpAddr {
            ip,
            port,
        }
    }

    // TODO: merge with new1
    pub fn parse(sa: SocketAddr) -> IpAddr {
        let mut addr = IpAddr::new();
        addr.ip
            .copy_from_slice(&convert_ip_string(sa.ip().to_string()));
        addr.port = sa.port() as u32;
        addr
    }

    pub fn get_ip(&self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.ip[1], self.ip[3], self.ip[5], self.ip[7]
        )
        .to_string()
    }

    pub fn to_string(&self) -> String {
        format!(
            "{}.{}.{}.{}:{}",
            self.ip[1], self.ip[3], self.ip[5], self.ip[7], self.port
        )
        .to_string()
    }

    pub fn to_formatted_string(&self) -> String {
        format!(
            "{:>3}.{:>3}.{:>3}.{:>3}:{}",
            self.ip[1], self.ip[3], self.ip[5], self.ip[7], self.port
        )
        .to_string()
    }
}

// struct for initial seeds && active nodes from p2p communication
#[derive(Clone)]
pub struct TempNode {
    pub id: [u8; NODE_ID_LENGTH],
    pub addr: IpAddr,
    pub if_seed: bool,
}

impl TempNode {
    pub fn new(id: [u8; NODE_ID_LENGTH], addr: IpAddr, if_seed: bool) -> TempNode {
        TempNode {
            id,
            addr,
            if_seed,
        }
    }

    // TODO: remove in future
    pub fn default() -> TempNode {
        TempNode {
            id: [b'0'; NODE_ID_LENGTH],
            addr: IpAddr::new(),
            if_seed: false,
        }
    }

    pub fn get_hash(&self) -> u64 {
        let ip = self.addr.get_ip();
        // let list = vec![String::from(from_utf8(&self.id).unwrap()), ip];
        // let text = list.join("").to_string();
        // calculate_hash(&text)
        let hash: u64 = calculate_hash(&ip);
        trace!(target: "p2p", "temp_node/get_hash: {}", &hash);
        hash
    }

    // filter out incoming connection
    // pub fn get_blank_id_hash(&self) -> u64 {
    //     let ip = self.addr.get_ip();
    //     let list = vec![String::from(EMPTY_ID), ip];
    //     let text = list.join("").to_string();
    //     calculate_hash(&text)
    // }

    pub fn get_id_string(&self) -> String { String::from_utf8_lossy(&self.id).into() }

    // construct node from seed config
    // constrait check
    // TODO: return Option<TempNode>
    pub fn new_from_str(node_str: String) -> TempNode {
        let (_, node_str) = node_str.split_at(PROTOCOL_LENGTH);
        let (id_str, addr_str_0) = node_str.split_at(NODE_ID_LENGTH);
        let (_, addr_str_1) = addr_str_0.split_at(1);
        let addr_str_1_arr: Vec<&str> = addr_str_1.split(":").collect();
        let ip_str = addr_str_1_arr[0];
        let port_str = addr_str_1_arr[1];

        let mut id: [u8; NODE_ID_LENGTH] = [b'0'; NODE_ID_LENGTH];
        if EMPTY_ID == id_str.to_string() {
            let uuid = Uuid::new_v4();
            id.copy_from_slice(uuid.hyphenated().to_string().as_bytes());
        } else {
            id.copy_from_slice(id_str.as_bytes());
        }

        let mut addr = IpAddr::new();
        addr.ip
            .copy_from_slice(&convert_ip_string(ip_str.to_string()));
        addr.port = port_str.parse::<u32>().unwrap_or(30303);

        TempNode {
            id,
            addr,
            if_seed: true,
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum Connection {
    INBOUND,
    OUTBOUND,
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            Connection::INBOUND => "inbound",
            Connection::OUTBOUND => "outbound",
        };
        write!(f, "{}", printable)
    }
}

#[derive(Clone)]
pub struct Node {
    pub hash: u64,
    pub id: [u8; NODE_ID_LENGTH],
    pub net_id: u32,
    pub addr: IpAddr,
    pub genesis_hash: H256,

    pub if_boot: bool,
    pub revision: [u8; MAX_REVISION_LENGTH],
    pub tx: mpsc::Sender<ChannelBuffer>,
    pub state: STATE,
    pub connection: Connection,
    pub if_seed: bool,
    pub update: SystemTime,
}

impl Node {
    // construct inbound node
    pub fn new_outbound(
        sa: SocketAddr,
        tx: mpsc::Sender<ChannelBuffer>,
        id: [u8; NODE_ID_LENGTH],
        if_seed: bool,
    ) -> Node
    {
        Node {
            hash: 0,
            id,
            net_id: 0,
            addr: IpAddr::parse(sa),
            genesis_hash: H256::default(),
            if_boot: false,
            revision: [b' '; MAX_REVISION_LENGTH],
            tx,
            state: STATE::CONNECTED,
            connection: Connection::OUTBOUND,
            if_seed,
            update: SystemTime::now(),
        }
    }

    // construct outbound node
    pub fn new_inbound(sa: SocketAddr, tx: mpsc::Sender<ChannelBuffer>, if_seed: bool) -> Node {
        Node {
            hash: 0,
            id: [b'0'; NODE_ID_LENGTH],
            net_id: 0,
            addr: IpAddr::parse(sa),
            genesis_hash: H256::default(),
            if_boot: false,
            revision: [b' '; MAX_REVISION_LENGTH],
            tx,
            state: STATE::CONNECTED,
            connection: Connection::INBOUND,
            if_seed,
            update: SystemTime::now(),
        }
    }

    pub fn get_hash(&self) -> u64 {
        let ip = self.addr.get_ip();
        let hash: u64 = calculate_hash(&ip);
        trace!(target: "p2p", "node/get_hash: {}", &hash);
        hash
    }

    pub fn get_id_string(&self) -> String { String::from_utf8_lossy(&self.id).into() }

    pub fn update(&mut self) {
        debug!(target: "p2p", "node timestamp updated");
        self.update = SystemTime::now();
    }
}

pub fn convert_ip_string(ip_str: String) -> [u8; 8] {
    let mut ip: [u8; 8] = [0u8; 8];
    let ip_vec: Vec<&str> = ip_str.split(".").collect();
    if ip_vec.len() == 4 {
        ip[0] = 0;
        ip[1] = ip_vec[0].parse::<u8>().unwrap_or(0);
        ip[2] = 0;
        ip[3] = ip_vec[1].parse::<u8>().unwrap_or(0);
        ip[4] = 0;
        ip[5] = ip_vec[2].parse::<u8>().unwrap_or(0);
        ip[6] = 0;
        ip[7] = ip_vec[3].parse::<u8>().unwrap_or(0);
    }
    ip
}

// TODO
#[cfg(test)]
mod node_tests {

    use TempNode;

    #[test]
    fn parse_seed_test() {
        let node_str = "p2p://00000000-0000-0000-0000-000000000000@0.0.0.0:30303".to_string();
        let tn = TempNode::new_from_str(node_str);
        assert_eq!(tn.addr.to_string(), "0.0.0.0:30303".to_string());
    }
}
