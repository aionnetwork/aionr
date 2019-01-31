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
use acore_bytes::to_hex;
use bincode::config;
use bytes::BytesMut;
use futures::sync::mpsc;
use futures::{Future, Stream};
use rand::{thread_rng, Rng};
use state::Storage;
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io;
use std::net::Shutdown;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use tokio::runtime::TaskExecutor;
use tokio_codec::{Decoder, Encoder, Framed};
use tokio_threadpool::{Builder, ThreadPool};

mod error;
mod event;
mod msg;
mod node;

pub use self::error::*;
pub use self::event::*;
pub use self::msg::*;
pub use self::node::*;

lazy_static! {
    static ref LOCAL_NODE: Storage<Node> = Storage::new();
    static ref NETWORK_CONFIG: Storage<NetworkConfig> = Storage::new();
    static ref SOCKETS_MAP: Storage<Mutex<HashMap<u64, TcpStream>>> = Storage::new();
    static ref GLOBAL_NODES_MAP: RwLock<HashMap<u64, Node>> = { RwLock::new(HashMap::new()) };
    static ref TOP8_NODE_HASHES: RwLock<Vec<u64>> = { RwLock::new(Vec::new()) };
    static ref ENABLED: Storage<AtomicBool> = Storage::new();
    static ref TP: Storage<ThreadPool> = Storage::new();
}

#[derive(Clone, Copy)]
pub struct P2pMgr;

impl P2pMgr {
    pub fn enable(cfg: NetworkConfig) {
        let sockets_map: HashMap<u64, TcpStream> = HashMap::new();
        SOCKETS_MAP.set(Mutex::new(sockets_map));

        let local_node_str = cfg.local_node.clone();
        let mut local_node = Node::new_with_node_str(local_node_str);

        local_node.net_id = cfg.net_id;

        info!(target:"net","local node loaded: {}@{}", local_node.get_node_id(), local_node.get_ip_addr());

        LOCAL_NODE.set(local_node.clone());

        ENABLED.set(AtomicBool::new(true));

        TP.set(
            Builder::new()
                .pool_size((cfg.max_peers * 3) as usize)
                .build(),
        );

        NETWORK_CONFIG.set(cfg);
    }

    pub fn create_server(
        executor: &TaskExecutor,
        local_addr: &String,
        handle: fn(node: &mut Node, req: ChannelBuffer),
    )
    {
        if let Ok(addr) = local_addr.parse() {
            let listener = TcpListener::bind(&addr).expect("Failed to bind");
            info!(target: "net", "Listening on: {}", local_addr);
            let server = listener
                .incoming()
                .map_err(|e| error!(target: "net", "Failed to accept socket; error = {:?}", e))
                .for_each(move |socket| {
                    socket
                        .set_recv_buffer_size(1 << 24)
                        .expect("set_recv_buffer_size failed");

                    socket
                        .set_keepalive(Some(Duration::from_secs(30)))
                        .expect("set_keepalive failed");

                    Self::process_inbounds(socket, handle);

                    Ok(())
                });
            executor.spawn(server);
        } else {
            error!(target: "net", "Invalid ip address: {}", local_addr);
        }
    }

    pub fn create_client(peer_node: Node, handle: fn(node: &mut Node, req: ChannelBuffer)) {
        let node_ip_addr = peer_node.get_ip_addr();
        if let Ok(addr) = node_ip_addr.parse() {
            let thread_pool = Self::get_thread_pool();
            let node_id = peer_node.get_node_id();
            let connect = TcpStream::connect(&addr)
                .map(move |socket| {
                    socket
                        .set_recv_buffer_size(1 << 24)
                        .expect("set_recv_buffer_size failed");

                    socket
                        .set_keepalive(Some(Duration::from_secs(30)))
                        .expect("set_keepalive failed");

                    Self::process_outbounds(socket, peer_node, handle);
                })
                .map_err(
                    move |e| error!(target: "net", "Node: {}@{}, {}", node_ip_addr, node_id, e),
                );
            thread_pool.spawn(connect);
        }
    }

    pub fn get_thread_pool() -> &'static ThreadPool { TP.get() }

    pub fn get_network_config() -> &'static NetworkConfig { NETWORK_CONFIG.get() }

    pub fn load_boot_nodes(boot_nodes_str: Vec<String>) -> Vec<Node> {
        let mut boot_nodes = Vec::new();
        if let Ok(mut top8) = TOP8_NODE_HASHES.write() {
            for boot_node_str in boot_nodes_str {
                if boot_node_str.len() != 0 {
                    let mut boot_node = Node::new_with_node_str(boot_node_str.to_string());
                    top8.push(boot_node.node_hash.clone());
                    boot_node.is_from_boot_list = true;
                    boot_nodes.push(boot_node);
                }
            }
            if top8.len() > 8 {
                top8.split_off(8);
            }
        }
        boot_nodes
    }

    pub fn get_local_node() -> &'static Node { LOCAL_NODE.get() }

    pub fn disable() {
        ENABLED.get().store(false, Ordering::SeqCst);
        Self::reset();
    }

    pub fn reset() {
        if let Ok(mut sockets_map) = SOCKETS_MAP.get().lock() {
            for (_, socket) in sockets_map.iter_mut() {
                if let Err(e) = socket.shutdown() {
                    error!(target: "net", "Invalid socket， {}", e);
                }
            }
        }
        if let Ok(mut nodes_map) = GLOBAL_NODES_MAP.write() {
            nodes_map.clear();
        }
    }

    pub fn get_peer(node_hash: u64) -> Option<TcpStream> {
        if let Ok(mut socktes_map) = SOCKETS_MAP.get().lock() {
            return socktes_map.remove(&node_hash);
        }

        None
    }

    pub fn add_peer(node: Node, ref_socket: &TcpStream) {
        if let Ok(socket) = ref_socket.try_clone() {
            if let Ok(mut sockets_map) = SOCKETS_MAP.get().lock() {
                match sockets_map.get(&node.node_hash) {
                    Some(_) => {
                        warn!(target: "net", "Known node, ...");
                    }
                    None => {
                        if let Ok(mut peer_nodes) = GLOBAL_NODES_MAP.write() {
                            let max_peers_num = NETWORK_CONFIG.get().max_peers as usize;
                            if peer_nodes.len() < max_peers_num {
                                match peer_nodes.get(&node.node_hash) {
                                    Some(_) => {
                                        warn!(target: "net", "Known node...");
                                    }
                                    None => {
                                        sockets_map.insert(node.node_hash, socket);
                                        peer_nodes.insert(node.node_hash, node);
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Err(e) = ref_socket.shutdown(Shutdown::Both) {
            error!(target: "net", "{}", e);
        }
    }

    pub fn remove_peer(node_hash: u64) -> Option<Node> {
        if let Ok(mut sockets_map) = SOCKETS_MAP.get().lock() {
            if let Some(socket) = sockets_map.remove(&node_hash) {
                if let Err(e) = socket.shutdown(Shutdown::Both) {
                    trace!(target: "net", "remove_peer， invalid socket， {}", e);
                }
            }
        }
        if let Ok(mut peer_nodes) = GLOBAL_NODES_MAP.write() {
            // if let Some(node) = peer_nodes.remove(&node_hash) {
            //     info!(target: "p2p", "Node {}@{} removed.", node.get_node_id(), node.get_ip_addr());
            //     return Some(node);
            // }
            // info!(target: "net", "remove_peer， peer_node hash: {}", node_hash);
            return peer_nodes.remove(&node_hash);
        }

        None
    }

    pub fn add_node(node: Node) {
        let max_peers_num = NETWORK_CONFIG.get().max_peers as usize;
        if let Ok(mut nodes_map) = GLOBAL_NODES_MAP.write() {
            if nodes_map.len() < max_peers_num {
                match nodes_map.get(&node.node_hash) {
                    Some(_) => {
                        warn!(target: "net", "Known node...");
                    }
                    None => {
                        nodes_map.insert(node.node_hash, node);
                        return;
                    }
                }
            }
        }
    }

    fn get_tx(node_hash: u64) -> Option<Tx> {
        if let Ok(nodes_map) = GLOBAL_NODES_MAP.read() {
            if let Some(node) = nodes_map.get(&node_hash) {
                return node.tx.clone();
            }
        }
        None
    }

    pub fn is_connected(node_id_hash: u64) -> bool {
        let all_nodes = P2pMgr::get_all_nodes();
        for node in all_nodes.iter() {
            if node_id_hash == Self::calculate_hash(&node.get_node_id()) {
                return true;
            }
        }
        false
    }

    pub fn get_nodes_count(state_code: u32) -> usize {
        let mut nodes_count = 0;
        if let Ok(nodes_map) = GLOBAL_NODES_MAP.read() {
            for val in nodes_map.values() {
                if val.state_code & state_code == state_code {
                    nodes_count += 1;
                }
            }
        }
        nodes_count
    }

    pub fn get_nodes_count_with_mode(mode: Mode) -> usize {
        let mut nodes_count = 0;
        if let Ok(nodes_map) = GLOBAL_NODES_MAP.read() {
            for val in nodes_map.values() {
                if val.state_code & ALIVE == ALIVE && val.mode == mode {
                    nodes_count += 1;
                }
            }
        }
        nodes_count
    }

    pub fn get_nodes_count_all_modes() -> (usize, usize, usize, usize, usize) {
        let mut normal_nodes_count = 0;
        let mut backward_nodes_count = 0;
        let mut forward_nodes_count = 0;
        let mut lightning_nodes_count = 0;
        let mut thunder_nodes_count = 0;
        if let Ok(nodes_map) = GLOBAL_NODES_MAP.read() {
            for val in nodes_map.values() {
                if val.state_code & ALIVE == ALIVE {
                    match val.mode {
                        Mode::NORMAL => normal_nodes_count += 1,
                        Mode::BACKWARD => backward_nodes_count += 1,
                        Mode::FORWARD => forward_nodes_count += 1,
                        Mode::LIGHTNING => lightning_nodes_count += 1,
                        Mode::THUNDER => thunder_nodes_count += 1,
                    }
                }
            }
        }
        (
            normal_nodes_count,
            backward_nodes_count,
            forward_nodes_count,
            lightning_nodes_count,
            thunder_nodes_count,
        )
    }

    pub fn get_all_nodes_count() -> u16 {
        let mut count = 0;
        if let Ok(nodes_map) = GLOBAL_NODES_MAP.read() {
            for _ in nodes_map.values() {
                count += 1;
            }
        }
        count
    }

    pub fn get_all_nodes() -> Vec<Node> {
        let mut nodes = Vec::new();
        if let Ok(nodes_map) = GLOBAL_NODES_MAP.read() {
            for val in nodes_map.values() {
                let node = val.clone();
                nodes.push(node);
            }
        }
        nodes
    }

    pub fn get_nodes(state_code_mask: u32) -> Vec<Node> {
        let mut nodes = Vec::new();
        if let Ok(nodes_map) = GLOBAL_NODES_MAP.read() {
            for val in nodes_map.values() {
                let node = val.clone();
                if node.state_code & state_code_mask == state_code_mask {
                    nodes.push(node);
                }
            }
        }
        nodes
    }

    pub fn get_an_inactive_node() -> Option<Node> {
        let nodes = Self::get_nodes(DISCONNECTED);
        let mut normal_nodes = Vec::new();
        for node in nodes.iter() {
            if node.is_from_boot_list {
                continue;
            } else {
                normal_nodes.push(node);
            }
        }
        let normal_nodes_count = normal_nodes.len();
        if normal_nodes_count == 0 {
            return None;
        }
        let mut rng = thread_rng();
        let random_index: usize = rng.gen_range(0, normal_nodes_count);
        let node = &normal_nodes[random_index];

        Self::remove_peer(node.node_hash)
    }

    pub fn get_an_active_node() -> Option<Node> {
        if let Ok(refresh_top8_nodes) = TOP8_NODE_HASHES.read() {
            let node_count = refresh_top8_nodes.len();
            let mut rng = thread_rng();
            let random_index: usize = rng.gen_range(0, node_count);
            if let Some(node_hash) = refresh_top8_nodes.get(random_index) {
                return Self::get_node(*node_hash);
            }
        }
        None
    }

    pub fn get_node(node_hash: u64) -> Option<Node> {
        if let Ok(nodes_map) = GLOBAL_NODES_MAP.read() {
            if let Some(node) = nodes_map.get(&node_hash) {
                return Some(node.clone());
            }
        }
        None
    }

    pub fn update_node_with_mode(node_hash: u64, node: &Node) {
        if let Ok(mut nodes_map) = GLOBAL_NODES_MAP.write() {
            if let Some(n) = nodes_map.get_mut(&node_hash) {
                n.update(node);
            }
        }
    }

    pub fn update_node(node_hash: u64, node: &mut Node) {
        if let Ok(mut nodes_map) = GLOBAL_NODES_MAP.write() {
            if let Some(n) = nodes_map.get_mut(&node_hash) {
                node.mode = n.mode.clone();
                n.update(node);
            }
        }
    }

    pub fn replace_top8_node_hashes(node_hashes: Vec<u64>) {
        if let Ok(mut refresh_top8_nodes) = TOP8_NODE_HASHES.write() {
            refresh_top8_nodes.clear();
            refresh_top8_nodes.extend(node_hashes);
        }
    }

    pub fn refresh_top8_node_hashes(node_hashes: Vec<u64>) {
        if let Ok(mut refresh_top8_nodes) = TOP8_NODE_HASHES.write() {
            refresh_top8_nodes.retain(|node_hash| !node_hashes.contains(node_hash));
            refresh_top8_nodes.splice(..0, node_hashes.iter().cloned());
            if refresh_top8_nodes.len() > 8 {
                refresh_top8_nodes.split_off(8);
            }
        }
    }

    pub fn get_top8_node_hashes() -> HashSet<u64> {
        if let Ok(top8) = TOP8_NODE_HASHES.read() {
            top8.iter().map(|hash| *hash).collect::<HashSet<_>>()
        } else {
            HashSet::new()
        }
    }

    pub fn process_inbounds(socket: TcpStream, handle: fn(node: &mut Node, req: ChannelBuffer)) {
        if let Ok(peer_addr) = socket.peer_addr() {
            let mut peer_node = Node::new_with_addr(peer_addr);
            let peer_ip = peer_node.ip_addr.get_ip();
            let local_ip = P2pMgr::get_local_node().ip_addr.get_ip();
            let network_config = P2pMgr::get_network_config();
            if P2pMgr::get_nodes_count(ALIVE) < network_config.max_peers as usize
                && !network_config.ip_black_list.contains(&peer_ip)
            {
                let mut value = peer_node.ip_addr.get_addr();
                value.push_str(&local_ip);
                peer_node.node_hash = P2pMgr::calculate_hash(&value);
                peer_node.state_code = CONNECTED;
                trace!(target: "net", "New incoming connection: {}", peer_addr);

                let (tx, rx) = mpsc::channel(409600);
                let thread_pool = P2pMgr::get_thread_pool();

                peer_node.tx = Some(tx);
                peer_node.state_code = CONNECTED;
                peer_node.ip_addr.is_server = false;

                trace!(target: "net", "A new peer added: {}", peer_node);

                let mut node_hash = peer_node.node_hash;
                P2pMgr::add_peer(peer_node, &socket);
                // process request from the incoming stream
                let (sink, stream) = P2pMgr::split_frame(socket);
                let read = stream.for_each(move |msg| {
                    if let Some(mut peer_node) = P2pMgr::get_node(node_hash) {
                        handle(&mut peer_node, msg.clone());
                        node_hash = P2pMgr::calculate_hash(&peer_node.get_node_id());
                    }

                    Ok(())
                });

                thread_pool.spawn(read.then(|_| Ok(())));

                // send everything in rx to sink
                let write = sink.send_all(rx.map_err(|()| {
                    io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")
                }));
                thread_pool.spawn(write.then(move |_| {
                    trace!(target:"net", "Connection with {:?} closed.", peer_ip);
                    Ok(())
                }));
            }
        } else {
            error!(target: "net", "Invalid socket: {:?}", socket);
        }
    }

    fn process_outbounds(
        socket: TcpStream,
        peer_node: Node,
        handle: fn(node: &mut Node, req: ChannelBuffer),
    )
    {
        let mut peer_node = peer_node.clone();
        peer_node.node_hash = P2pMgr::calculate_hash(&peer_node.get_node_id());
        let node_hash = peer_node.node_hash;

        if let Some(node) = P2pMgr::get_node(node_hash) {
            if node.state_code == DISCONNECTED {
                trace!(target: "net", "update known peer node {}@{}...", node.get_node_id(), node.get_ip_addr());
                P2pMgr::remove_peer(node_hash);
            } else {
                return;
            }
        }

        let (tx, rx) = mpsc::channel(409600);
        peer_node.tx = Some(tx);
        peer_node.state_code = CONNECTED | IS_SERVER;
        peer_node.ip_addr.is_server = true;
        let peer_ip = peer_node.get_ip_addr().clone();
        trace!(target: "net", "A new peer added: {}@{}", peer_node.get_node_id(), peer_node.get_ip_addr());

        P2pMgr::add_peer(peer_node.clone(), &socket);

        // process request from the outcoming stream
        let (sink, stream) = P2pMgr::split_frame(socket);

        // OnConnect
        let mut req = ChannelBuffer::new();
        req.head.set_version(Version::V1);
        handle(&mut peer_node, req);

        let read = stream.for_each(move |msg| {
            if let Some(mut peer_node) = P2pMgr::get_node(node_hash) {
                handle(&mut peer_node, msg);
            }

            Ok(())
        });
        let thread_pool = P2pMgr::get_thread_pool();
        thread_pool.spawn(read.then(|_| Ok(())));

        // send everything in rx to sink
        let write = sink.send_all(
            rx.map_err(|()| io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")),
        );
        thread_pool.spawn(write.then(move |_| {
            trace!(target:"net", "Connection with {:?} closed.", peer_ip);
            Ok(())
        }));
    }

    pub fn send(node_hash: u64, msg: ChannelBuffer) {
        match Self::get_tx(node_hash) {
            Some(mut tx) => {
                match tx.try_send(msg) {
                    Ok(()) => {}
                    Err(e) => {
                        Self::remove_peer(node_hash);
                        trace!(target: "net", "Failed to send the msg, Err: {}", e);
                    }
                }
            }
            None => {
                Self::remove_peer(node_hash);
                trace!(target: "net", "Invalid peer !, node_hash: {}", node_hash);
            }
        }
    }

    pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    pub fn split_frame(
        socket: TcpStream,
    ) -> (
        stream::SplitSink<Framed<TcpStream, P2pCodec>>,
        stream::SplitStream<Framed<TcpStream, P2pCodec>>,
    ) {
        P2pCodec.framed(socket).split()
    }
}

pub struct P2pCodec;

impl Encoder for P2pCodec {
    type Item = ChannelBuffer;
    type Error = io::Error;

    fn encode(&mut self, item: ChannelBuffer, dst: &mut BytesMut) -> io::Result<()> {
        let mut encoder = config();
        let encoder = encoder.big_endian();
        if let Ok(encoded) = encoder.serialize(&item.head) {
            dst.extend_from_slice(encoded.as_slice());
            dst.extend_from_slice(item.body.as_slice());
        }

        Ok(())
    }
}

impl Decoder for P2pCodec {
    type Item = ChannelBuffer;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<ChannelBuffer>> {
        let len = src.len();
        if len >= HEADER_LENGTH {
            let mut decoder = config();
            let decoder = decoder.big_endian();
            let mut invalid = false;
            let mut decoded = ChannelBuffer::new();
            {
                let (head_raw, _) = src.split_at(HEADER_LENGTH);
                if let Ok(head) = decoder.deserialize(head_raw) {
                    decoded.head = head;
                    if decoded.head.ver > Version::V2.value()
                        || decoded.head.ctrl > Control::SYNC.value()
                        || decoded.head.action > MAX_VALID_ACTTION_VALUE
                    {
                        invalid = true;
                    } else if decoded.head.len as usize + HEADER_LENGTH > len {
                        return Ok(None);
                    }
                }
            }

            if invalid {
                src.split_to(len);
                Ok(None)
            } else {
                let buf = src.split_to(decoded.head.len as usize + HEADER_LENGTH);
                let (_, body) = buf.split_at(HEADER_LENGTH);
                decoded.body.extend_from_slice(body);
                Ok(Some(decoded))
            }
        } else {
            if len > 0 {
                debug!(target: "net", "len = {}, {}", len, to_hex(src));
            }
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Network service configuration
pub struct NetworkConfig {
    /// List of initial node addresses
    pub boot_nodes: Vec<String>,
    /// Max number of connected peers to maintain
    pub max_peers: u32,
    /// net id
    pub net_id: u32,
    /// local node
    pub local_node: String,
    /// if only sync from bootnodes
    pub sync_from_boot_nodes_only: bool,
    /// IP black list
    pub ip_black_list: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self { NetworkConfig::new() }
}

impl NetworkConfig {
    /// Create a new instance of default settings.
    pub fn new() -> Self {
        NetworkConfig {
            boot_nodes: Vec::new(),
            max_peers: 64,
            local_node: String::from("p2p://00000000-0000-0000-0000-000000000000@0.0.0.0:30303"),
            net_id: 0,
            sync_from_boot_nodes_only: false,
            ip_black_list: Vec::new(),
        }
    }
}
