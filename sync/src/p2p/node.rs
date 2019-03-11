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

use aion_types::{H256, U256};
use byteorder::{BigEndian, ReadBytesExt};
use futures::sync::mpsc;
use std::collections::HashSet;
use std::fmt;
use std::net::SocketAddr;
use std::time::SystemTime;
use uuid::Uuid;

pub use super::event::*;
pub use super::msg::*;

pub type Tx = mpsc::Sender<ChannelBuffer>;

pub const HEADER_LENGTH: usize = 8;
pub const NODE_ID_LENGTH: usize = 36;
pub const PROTOCOL_LENGTH: usize = 6;
pub const IP_LENGTH: usize = 8;
pub const DIFFICULTY_LENGTH: usize = 16;
pub const MAX_REVISION_LENGTH: usize = 24;

pub const CONNECTED: u32 = 1;
pub const IS_SERVER: u32 = 1 << 1;
pub const ALIVE: u32 = 1 << 3;
pub const DISCONNECTED: u32 = 1 << 10;

#[derive(Clone, Copy, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct IpAddr {
    pub ip: [u8; 8],
    pub port: u32,
    pub is_server: bool,
}

impl IpAddr {
    pub fn new() -> IpAddr {
        IpAddr {
            ip: [0; 8],
            port: 0,
            is_server: false,
        }
    }

    pub fn get_addr(&self) -> String {
        format!(
            "{}.{}.{}.{}:{}",
            self.ip[1], self.ip[3], self.ip[5], self.ip[7], self.port
        )
        .to_string()
    }

    pub fn get_display_addr(&self) -> String {
        format!(
            "{:>3}.{:>3}.{:>3}.{:>3}:{}",
            self.ip[1], self.ip[3], self.ip[5], self.ip[7], self.port
        )
        .to_string()
    }

    pub fn get_ip(&self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.ip[1], self.ip[3], self.ip[5], self.ip[7]
        )
        .to_string()
    }
}

impl fmt::Display for IpAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Ip Address: {}, Server: {}",
            self.get_addr(),
            self.is_server
        )
    }
}

#[derive(Clone)]
pub struct Node {
    pub node_id: [u8; NODE_ID_LENGTH],
    pub net_id: u32,
    pub ip_addr: IpAddr,
    pub node_hash: u64,
    pub state_code: u32,
    pub best_block_num: u64,
    pub synced_block_num: u64,
    pub best_hash: H256,
    pub genesis_hash: H256,
    pub target_total_difficulty: U256,
    pub current_total_difficulty: U256,
    pub last_request_timestamp: SystemTime,
    pub requested_block_num: u64,
    pub last_broadcast_timestamp: SystemTime,
    pub last_sent_transactions: HashSet<H256>,
    pub tx: Option<Tx>,
    pub is_from_boot_list: bool,
    pub repeated: u8,
    pub revision: [u8; MAX_REVISION_LENGTH],
    pub reputation: u64,
}

impl Node {
    pub fn new() -> Node {
        Node {
            node_id: [b'0'; NODE_ID_LENGTH],
            net_id: 0,
            ip_addr: IpAddr::new(),
            node_hash: 0,
            state_code: DISCONNECTED,
            best_block_num: 0,
            synced_block_num: 0,
            best_hash: H256::default(),
            genesis_hash: H256::default(),
            target_total_difficulty: U256::default(),
            current_total_difficulty: U256::default(),
            last_request_timestamp: SystemTime::now(),
            requested_block_num: 0,
            last_broadcast_timestamp: SystemTime::now(),
            last_sent_transactions: HashSet::new(),
            tx: None,
            is_from_boot_list: false,
            repeated: 0,
            revision: [b' '; MAX_REVISION_LENGTH],
            reputation: 0,
        }
    }

    pub fn new_with_node_str(node_str: String) -> Node {
        let (_, node) = node_str.split_at(PROTOCOL_LENGTH);

        let (node_id, node_addr) = node.split_at(NODE_ID_LENGTH);
        let (_, node_addr) = node_addr.split_at(1);
        let node_addr: Vec<&str> = node_addr.split(":").collect();
        let node_ip = node_addr[0];
        let node_port = node_addr[1];

        let mut node = Node::new();
        if "00000000-0000-0000-0000-000000000000" == node_id.to_string() {
            let uuid = Uuid::new_v4();
            node.node_id
                .copy_from_slice(uuid.to_hyphenated().to_string().as_bytes());
        } else {
            node.node_id.copy_from_slice(node_id.as_bytes());
        }

        node.ip_addr
            .ip
            .copy_from_slice(convert_ip_string(node_ip.to_string()).as_slice());
        node.ip_addr.port = node_port.parse::<u32>().unwrap_or(30303);
        node.state_code = CONNECTED;

        node
    }

    pub fn new_with_addr(addr: SocketAddr) -> Node {
        let mut node = Node::new();

        let ip = addr.ip();
        node.ip_addr
            .ip
            .copy_from_slice(convert_ip_string(ip.to_string()).as_slice());
        let port = addr.port();
        node.ip_addr.port = port as u32;
        node.state_code = CONNECTED;

        node
    }

    pub fn create_node(node_id: &[u8], ip: &[u8], mut port: &[u8], node_hash: u64) -> Node {
        let mut node = Node::new();
        node.node_id.copy_from_slice(node_id);
        node.ip_addr.ip.copy_from_slice(ip);
        node.ip_addr.port = port.read_u32::<BigEndian>().unwrap_or(30303);
        node.node_hash = node_hash;
        node
    }

    pub fn update(&mut self, node_new: &Node) {
        self.node_id.copy_from_slice(&node_new.node_id);
        self.net_id = node_new.net_id;
        self.ip_addr.ip.copy_from_slice(&node_new.ip_addr.ip);
        self.ip_addr.port = node_new.ip_addr.port;
        self.ip_addr.is_server = node_new.ip_addr.is_server;
        self.node_hash = node_new.node_hash;
        self.state_code = node_new.state_code;
        self.best_block_num = node_new.best_block_num;
        self.synced_block_num = node_new.synced_block_num;
        self.best_hash = node_new.best_hash;
        self.genesis_hash = node_new.genesis_hash;
        self.target_total_difficulty = node_new.target_total_difficulty;
        self.current_total_difficulty = node_new.current_total_difficulty;
        self.last_request_timestamp = node_new.last_request_timestamp;
        self.requested_block_num = node_new.requested_block_num;
        self.last_broadcast_timestamp = node_new.last_broadcast_timestamp;
        self.is_from_boot_list = node_new.is_from_boot_list;
        self.tx = node_new.tx.clone();
        self.repeated = node_new.repeated;
        self.revision = node_new.revision;
        self.reputation = node_new.reputation;
    }

    pub fn set_ip_addr(&mut self, addr: SocketAddr) {
        let ip = addr.ip();
        self.ip_addr
            .ip
            .copy_from_slice(convert_ip_string(ip.to_string()).as_slice());
        let port = addr.port();
        self.ip_addr.port = port as u32;
    }

    pub fn get_node_id(&self) -> String {
        let node_id = String::from_utf8_lossy(&self.node_id);
        node_id.into()
    }

    pub fn get_node_string(&self) -> String {
        format!("p2p://{}@{}", self.get_node_id(), self.get_ip_addr())
    }

    pub fn get_ip_addr(&self) -> String { self.ip_addr.get_addr() }

    pub fn get_ip(&self) -> String { self.ip_addr.get_ip() }

    pub fn get_display_ip_addr(&self) -> String { self.ip_addr.get_display_addr() }

    pub fn is_over_repeated_threshold(&self) -> bool { return self.repeated > 5; }

    pub fn inc_repeated(&mut self) { self.repeated = self.repeated + 1; }

    pub fn reset_repeated(&mut self) { self.repeated = 0; }

    pub fn inc_reputation(&mut self, score: u64) { self.reputation += score; }

    pub fn dec_reputation(&mut self, score: u64) {
        if self.reputation >= score {
            self.reputation -= score;
        }
    }

    pub fn reset_reputation(&mut self) { self.reputation = 0; }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Node information: \n    Node id: ")?;
        write!(
            f,
            "    revision:{}\n",
            String::from_utf8_lossy(&self.node_id)
        )?;
        write!(f, "\n    net_id: {}\n", self.net_id)?;
        write!(f, "    {}\n", self.ip_addr)?;
        write!(f, "    node hash: {:064X}\n", self.node_hash)?;
        write!(f, "    state code: {:032b}\n", self.state_code)?;
        write!(f, "    best block number: {}\n", self.best_block_num)?;
        write!(f, "    synced block number: {}\n", self.synced_block_num)?;
        write!(f, "    boot node: {}\n", self.is_from_boot_list)?;
        write!(
            f,
            "    last request timestamp: {:?}\n",
            self.last_request_timestamp
        )?;
        write!(f, "    requested_block_num: {}\n", self.requested_block_num)?;
        write!(
            f,
            "    last broadcast timestamp: {:?}\n",
            self.last_broadcast_timestamp
        )?;
        write!(f, "    best hash: {:?}\n", self.best_hash)?;
        write!(f, "    genesis hash: {:?}\n", self.genesis_hash)?;
        write!(f, "    repeated: {:?}\n", self.repeated)?;
        write!(
            f,
            "    total difficulty: {}\n",
            self.target_total_difficulty
        )?;
        write!(
            f,
            "    current total difficulty: {}\n",
            self.current_total_difficulty
        )?;
        write!(
            f,
            "    revision:{}\n",
            String::from_utf8_lossy(&self.revision)
        )?;
        write!(f, "    reputation: {}\n", self.reputation)
    }
}

pub fn convert_ip_string(ip_str: String) -> Vec<u8> {
    let mut ip = Vec::new();
    let ip_vec: Vec<&str> = ip_str.split(".").collect();
    for sec in ip_vec.iter() {
        ip.push(0);
        ip.push(sec.parse::<u8>().unwrap_or(0));
    }
    ip
}

#[cfg(test)]
mod node_tests {
    use p2p::{Node, CONNECTED};

    #[test]
    fn new_with_node_str_test() {
        let node_str = "p2p://00000000-0000-0000-0000-000000000000@0.0.0.0:30303".to_string();
        let node = Node::new_with_node_str(node_str);

        println!("Node: {}", node);
    }

    #[test]
    fn new_with_addr_test() {
        let node = Node::new_with_addr("127.0.0.1:30303".to_string().parse().unwrap());

        println!("Node: {}", node);
        assert_eq!(node.ip_addr.get_addr(), "127.0.0.1:30303".to_string());
        assert_eq!(node.state_code, CONNECTED);
    }
}
