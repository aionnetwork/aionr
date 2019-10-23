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

#![warn(unused_extern_crates)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate futures;
extern crate bincode;
extern crate rand;
extern crate tokio;
extern crate tokio_codec;
extern crate tokio_reactor;
extern crate acore_bytes;
extern crate aion_types;
extern crate uuid;
extern crate aion_version as version;
extern crate bytes;
extern crate byteorder;

#[cfg(test)]
mod test;
mod config;
mod route;
mod msg;
mod node;
mod codec;
mod state;
mod handler;
mod callable;

use std::io;
use std::sync::{Arc,Weak};
use std::collections::{VecDeque,HashMap,HashSet};
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;
use std::net::TcpStream as StdTcpStream;
use std::net::Shutdown;
use std::net::SocketAddr;
use rand::random;
use futures::prelude::*;
use futures::sync::mpsc;
use futures::{Future, Stream};
use futures::lazy;
use futures::sync::oneshot;
use futures::sync::oneshot::Sender;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::runtime::TaskExecutor;
use tokio::timer::Interval;
use tokio_reactor::Handle;
use tokio_codec::{Decoder,Framed};
use codec::Codec;
use route::Version;
use route::Action;
use state::STATE;
use handler::handshake;
use handler::active_nodes;
use node::TempNode;

pub use msg::ChannelBuffer;
pub use node::Node;
pub use config::Config;
pub use callable::Callable;

const INTERVAL_OUTBOUND_CONNECT: u64 = 1;
const INTERVAL_TIMEOUT: u64 = 5;
const INTERVAL_ACTIVE_NODES: u64 = 3;
const TIMEOUT_MAX: u64 = 30;
const TEMP_MAX: usize = 64;

pub const PROTOCAL_VERSION: u16 = Version::V0 as u16;
pub use route::Module;

#[derive(Clone)]
pub struct Mgr {
    /// shutdown hook
    shutdown_hooks: Arc<Mutex<Vec<Sender<()>>>>,
    /// callback
    callback: Arc<RwLock<Option<Weak<Callable>>>>,
    /// config
    config: Arc<Config>,
    /// temp queue storing seeds and active nodes queried from other nodes
    temp: Arc<Mutex<VecDeque<TempNode>>>,
    /// nodes
    nodes: Arc<RwLock<HashMap<u64, Node>>>,
    /// tokens rule
    tokens_rule: Arc<HashMap<u32, u32>>,
    /// nodes ID
    nodes_id: Arc<Mutex<HashSet<String>>>,
}

impl Mgr {
    /// constructor
    pub fn new(mut config: Config, tokens_pairs: Vec<[u32; 2]>) -> Mgr {
        // load local node
        let temp_local = TempNode::new_from_str(config.local_node);
        config.local_node = format!(
            "p2p://{}@{}",
            temp_local.get_id_string(),
            temp_local.addr.to_string()
        );
        let mut id_set = HashSet::new();
        id_set.insert(temp_local.get_id_string());

        // load seeds
        let mut temp_queue = VecDeque::<TempNode>::with_capacity(TEMP_MAX);
        for boot_node_str in config.boot_nodes.clone() {
            info!(target: "run", "        seed: {}", &boot_node_str);
            temp_queue.push_back(TempNode::new_from_str(boot_node_str.to_string()));
        }

        // parse token rules
        let mut tokens_rule: HashMap<u32, u32> = HashMap::new();
        for pair in tokens_pairs {
            // pair[1]: receive token, pair[0]: send token
            tokens_rule.insert(pair[1], pair[0]);
        }

        let p2p_rule_base =
            ((Version::V0.value() as u32) << 16) + ((Module::P2P.value() as u32) << 8);

        tokens_rule.insert(
            p2p_rule_base + Action::ACTIVENODESRES.value() as u32,
            p2p_rule_base + Action::ACTIVENODESREQ.value() as u32,
        );

        Mgr {
            shutdown_hooks: Arc::new(Mutex::new(Vec::new())),
            callback: Arc::new(RwLock::new(None)),
            config: Arc::new(config),
            temp: Arc::new(Mutex::new(temp_queue)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            tokens_rule: Arc::new(tokens_rule),
            nodes_id: Arc::new(Mutex::new(id_set)),
        }
    }

    pub fn register_callback(&self, callback: Weak<Callable>) {
        if let Ok(mut lock) = self.callback.write() {
            *lock = Some(callback);
        }
    }

    pub fn clear_callback(&self) {
        if let Ok(mut lock) = self.callback.write() {
            *lock = None;
        }
    }

    /// verify inbound msg route through token collection
    /// token_pair: [u32, u32]
    /// token_pair[0]: flag_token, set on indivisual node tokens collection when sending msg
    /// token_pair[1]: clear_token, check on indivisual node when receiving msg
    /// 1. pass in clear_token from incoming msg route(ChannelBuffer::HEAD::get_route)
    ///    through token rules against token collection on indivisual node
    /// 2. return true if exsit send_token and remove it
    /// 3. return false if not exist flag_token
    /// 4. always return true if there is no token rule applied
    pub fn token_check(&self, clear_token: u32, node: &mut Node) -> bool {
        match &self.tokens_rule.get(&clear_token) {
            Some(&flag_token) => node.tokens.remove(&flag_token),
            None => true,
        }
    }

    /// send msg
    pub fn send(&self, hash: u64, cb: ChannelBuffer) -> bool {
        let nodes = &self.nodes;
        trace!(target: "p2p", "send: hash/ver/ctrl/action/route {}/{}/{}/{}/{}",
            &hash,
            cb.head.ver,
            cb.head.ctrl,
            cb.head.action,
            cb.head.get_route()
        );

        // TODO: need more thoughts on write or try_write
        match nodes.write() {
            Ok(mut lock) => {
                let mut send_success = true;
                if let Some(mut node) = lock.get_mut(&hash) {
                    let mut tx = node.tx.clone();
                    let route = cb.head.get_route();
                    match tx.try_send(cb) {
                        Ok(_) => {
                            // add flag token
                            node.tokens.insert(route);
                            trace!(target: "p2p", "p2p/send: {}", node.addr.get_ip());
                        }
                        Err(err) => {
                            send_success = false;
                            trace!(target: "p2p", "p2p/send: ip:{} err:{}", node.addr.get_ip(), err);
                        }
                    }
                } else {
                    warn!(target:"p2p", "send: node not found hash {}", hash);
                    return false;
                }
                if !send_success {
                    if let Some(node) = lock.remove(&hash) {
                        trace!(target: "p2p", "failed send, remove hash/id {}/{}", node.get_id_string(), node.addr.get_ip());
                        self.disconnect(hash, node.get_id_string());
                    }
                }
                send_success
            }
            Err(err) => {
                warn!(target:"p2p", "send: nodes read {:?}", err);
                false
            }
        }
    }

    /// run p2p instance
    pub fn run(&mut self, executor: TaskExecutor) {
        // init
        let binding: SocketAddr = self
            .config
            .get_id_and_binding()
            .1
            .parse::<SocketAddr>()
            .unwrap()
            .clone();

        // interval timeout
        let p2p_timeout = self.clone();
        let (tx, rx) = oneshot::channel::<()>();
        executor.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_TIMEOUT)
            ).for_each(move|_|{
                let mut index: Vec<u64> = vec![];
                if let Ok(mut write) = p2p_timeout.nodes.try_write(){
                    for (hash, node) in write.iter_mut() {
                        if let Ok(interval) = node.update.elapsed() {
                            if interval.as_secs() >= TIMEOUT_MAX {
                                index.push( * hash);
                            }
                        }
                    }

                    for i in 0 .. index.len() {
                        let hash = index[i];
                        match write.remove(&hash) {
                            Some(mut node) => {
                                node.tx.close().unwrap();

                                // dispatch node remove event
                                p2p_timeout.disconnect(hash, node.get_id_string());
                                debug!(target: "p2p", "timeout hash/id/ip {}/{}/{}", &node.get_hash(), &node.get_id_string(), &node.addr.to_string());
                            },
                            None => {}
                        }
                    }
                }
                Ok(())
            })
            .map_err(|err| error!(target: "p2p", "executor timeout: {:?}", err))
            .select(rx.map_err(|_| {}))
            .map(|_| ())
            .map_err(|_| ())
        );
        if let Ok(mut shutdown_hooks) = self.shutdown_hooks.lock() {
            shutdown_hooks.push(tx);
        }

        // interval outbound
        let executor_outbound_0 = executor.clone();
        let p2p_outbound = self.clone();
        let (tx, rx) = oneshot::channel::<()>();
        executor.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_OUTBOUND_CONNECT)
            ).for_each(move|_|{
                let p2p_outbound_0 = p2p_outbound.clone();

                // exist lock immediately after poping temp node
                let mut temp_node_opt: Option<TempNode> = None;
                {
                    if let Ok(mut lock) = p2p_outbound_0.temp.try_lock() {

                        if let Some(temp_node) = lock.pop_front() {
                            temp_node_opt = Some(temp_node.clone());

                            if let Ok(id_set) = p2p_outbound.nodes_id.lock(){
                                if id_set.contains(&temp_node.get_id_string()){
                                    temp_node_opt = None;
                                }
                            }
                            // store back if seed node immediately
                            if temp_node.if_seed {
                                lock.push_back(temp_node);
                            }
                        }
                    }
                }

                if temp_node_opt.is_some() {

                    // process outbound connection
                    let temp_node = temp_node_opt.unwrap();

                    // return if exist
                    let hash = temp_node.get_hash();
                    {
                        match p2p_outbound_0.nodes.try_read() {
                            Ok(read) => {
                                // return at node existing
                                if let Some(node) = read.get(&hash) {
                                    debug!(target: "p2p", "exist hash/id/ip {}/{}/{}", &hash, node.get_id_string(), node.addr.to_string());
                                    return Ok(());
                                }
                            },
                            Err(_err) => {
                                // return if read lock is unable to be rechieved
                                return Ok(())
                            }
                        }
                    }

                    // counters
                    let executor_outbound_1 = executor_outbound_0.clone();
                    let executor_outbound_2 = executor_outbound_0.clone();
                    let executor_outbound_3 = executor_outbound_0.clone();

                    if let Ok(addr) = temp_node.addr.to_string().parse() {
                        debug!(target: "p2p", "connecting to: {}", &addr);

                        match StdTcpStream::connect_timeout(&addr, Duration::from_millis(1000)) {
                            Ok(stdts)=>{
                                if let Ok(ts) = TcpStream::from_std(stdts, &Handle::default()) {
                                    debug!(target: "p2p", "connected to: {}", &temp_node.addr.to_string());

                                    let p2p_outbound_1 = p2p_outbound_0.clone();
                                    let p2p_outbound_2 = p2p_outbound_0.clone();

                                    // config stream
                                    match config_stream(&ts){
                                        Err(e) => {
                                            error!(target: "p2p", "fail to connect to {} : {}",&temp_node.addr.to_string(),e);
                                            return Ok(())
                                        }
                                        _ => ()
                                    }

                                    // construct node instance and store it
                                    let (mut tx, rx) = mpsc::channel(409600);
                                    let (mut tx_thread, rx_thread) = oneshot::channel::<()>();
                                    if let Ok(ts_0) = ts.try_clone() {
                                        let node = Node::new_outbound(
                                            ts_0,
                                            tx,
                                            temp_node.id,
                                            temp_node.if_seed,
                                            tx_thread,
                                        );

                                        if let Ok(mut write) = p2p_outbound_0.nodes.try_write() {
                                            if !write.contains_key(&hash) {
                                                let id = node.get_id_string();
                                                let ip = node.addr.get_ip();
                                                if let None = write.insert(hash.clone(), node) {
                                                    debug!(target: "p2p", "outbound node added: {} {} {}", hash, id, ip);
                                                }
                                            }
                                        }
                                    } else {
                                        trace!(target: "p2p", "failed to clone TcpStream, stop connecting to {}",&temp_node.addr.to_string());
                                        return Ok(())
                                    }

                                    // binding io futures
                                    let (sink, stream) = split_frame(ts);
                                    let read = stream.for_each(move |cb| {
                                        p2p_outbound_2.handle(hash.clone(), cb);
                                        Ok(())
                                    })
                                    .map_err(|err| trace!(target: "p2p", "tcp outbound read: {:?}", err))
                                    .select(rx_thread.map_err(|_| {}))
                                    .map(|_| ())
                                    .map_err(|_| ());

                                    executor_outbound_1.spawn(read.then(|_|{
                                        Ok(())
                                    }));

                                    let write = sink.send_all(
                                        rx.map_err(|()| io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")),
                                    );
                                    executor_outbound_2.spawn(write.then(|_| { Ok(()) }));

                                    // send handshake request
                                    executor_outbound_3.spawn(lazy(move||{
                                        handshake::send(p2p_outbound_1, hash);
                                        Ok(())
                                    }));
                                }
                            },
                            Err(_err) => {

                            }
                        }
                    }
                }
                Ok(())
            })
            .map_err(|err| error!(target: "p2p", "executor outbound: {:?}", err))
            .select(rx.map_err(|_| {}))
            .map(|_| ())
            .map_err(|_| ())
        );
        if let Ok(mut shutdown_hooks) = self.shutdown_hooks.lock() {
            shutdown_hooks.push(tx);
        }

        // interval active nodes
        let p2p_active_nodes = self.clone();
        let (tx, rx) = oneshot::channel::<()>();
        executor.spawn(
            Interval::new(Instant::now(), Duration::from_secs(INTERVAL_ACTIVE_NODES))
                .for_each(move |_| {
                    let p2p_active_nodes_0 = p2p_active_nodes.clone();
                    active_nodes::send(p2p_active_nodes_0);
                    Ok(())
                })
                .map_err(|err| error!(target: "p2p", "executor active nodes: {:?}", err))
                .select(rx.map_err(|_| {}))
                .map(|_| ())
                .map_err(|_| ()),
        );
        if let Ok(mut shutdown_hooks) = self.shutdown_hooks.lock() {
            shutdown_hooks.push(tx);
        }

        // interval inbound
        let executor_inbound_0 = executor.clone();
        let executor_inbound_1 = executor.clone();
        let p2p_inbound = self.clone();
        let (tx, rx) = oneshot::channel::<()>();
        let listener = TcpListener::bind(&binding).expect("binding failed");
        let tcp_executor = listener
            .incoming()
            .for_each(move |ts: TcpStream| {
                // counters
                let p2p_inbound_1 = p2p_inbound.clone();

                // TODO: black list check
                if p2p_inbound.get_active_nodes_len() >= p2p_inbound.config.max_peers {
                    debug!(target:"p2p", "max peers reached");
                    return Ok(());
                }

                // config stream
                match config_stream(&ts){
                    Err(e) => {
                        error!(target: "p2p", "fail to connect to {} : {}",&ts.peer_addr().unwrap().to_string(),e);
                        return Ok(())
                    }
                    _ => ()
                }

                // construct node instance and store it
                let (tx_channel, rx_channel) = mpsc::channel(409600);
                let (tx_thread, rx_thread) = oneshot::channel::<()>();
                if let Ok(ts_0) = ts.try_clone() {
                    let node = Node::new_inbound(
                        ts_0,
                        tx_channel,
                        false,
                        tx_thread,
                    );
                    let hash = node.get_hash();

                    if let Ok(mut write) = p2p_inbound.nodes.try_write() {
                        let id: String = node.get_id_string();
                        let binding: String = node.addr.to_string();
                        if !write.contains_key(&node.get_hash()) {
                            if let None = write.insert(node.get_hash(), node) {
                                info!(target: "p2p", "inbound node added: hash/id/ip {:?}/{:?}/{:?}", &hash, &id, &binding);
                            }
                        }
                    }

                    // binding io futures
                    let (sink, stream) = split_frame(ts);
                    let read = stream.for_each(move |cb| {
                        p2p_inbound_1.handle(hash.clone(), cb);
                        Ok(())
                    })
                        .map_err(|err| trace!(target: "p2p", "tcp inbound read: {:?}", err))
                        .select(rx_thread.map_err(|_| {}))
                        .map(|_| ())
                        .map_err(|_| ());
                    executor_inbound_0.spawn(read.then(|_| { Ok(()) }));
                    let write = sink.send_all(rx_channel.map_err(|()| {
                        io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")
                    }));
                    executor_inbound_1.spawn(write.then(|_| { Ok(()) }));
                } else {
                    trace!(target: "p2p", "failed to clone TcpStream, stop connecting to {}", &ts.peer_addr().unwrap().to_string());
                }
                Ok(())
            })
            .map_err(|err| error!(target: "p2p", "executor server: {:?}", err))
            .select(rx.map_err(|_| {}))
            .map(|_| ())
            .map_err(|_| ());
        executor.spawn(tcp_executor);
        if let Ok(mut shutdown_hooks) = self.shutdown_hooks.lock() {
            shutdown_hooks.push(tx);
        }
    }

    fn disconnect(&self, hash: u64, id: String) {
        if let Ok(mut id_set) = self.nodes_id.lock() {
            id_set.remove(&id);
        }
        if let Ok(lock) = self.callback.read() {
            if let Some(ref callback) = *lock {
                match Weak::upgrade(callback) {
                    Some(arc_callback) => arc_callback.disconnect(hash),
                    None => warn!(target: "p2p", "sync has been shutdown?" ),
                }
            }
        }
    }

    /// shutdown routine
    pub fn shutdown(&self) {
        info!(target: "p2p" , "p2p shutdown start");
        // Shutdown runtime tasks
        if let Ok(mut shutdown_hooks) = self.shutdown_hooks.lock() {
            while !shutdown_hooks.is_empty() {
                if let Some(shutdown_hook) = shutdown_hooks.pop() {
                    match shutdown_hook.send(()) {
                        Ok(_) => {
                            info!(target: "p2p", "shutdown signal sent");
                        }
                        Err(err) => {
                            info!(target: "p2p", "shutdown err: {:?}", err);
                        }
                    }
                }
            }
        }

        // Disconnect nodes
        if let Ok(mut lock) = self.nodes.write() {
            for (_hash, mut node) in lock.iter_mut() {
                match node.ts.shutdown(Shutdown::Both) {
                    Ok(_) => {
                        info!(target: "p2p", "close connection id/ip {}/{}", &node.get_id_string(), &node.get_id_string());
                    }
                    Err(err) => {
                        info!(target: "p2p", "shutdown err: {:?}", err);
                    }
                }

                match node.shutdown_tcp_thread() {
                    Ok(_) => {
                        info!(target: "p2p", "tcp connection thread shutdown signal sent");
                    }
                    Err(err) => {
                        info!(target: "p2p", "shutdown err: {:?}", err);
                    }
                }
            }
            lock.clear();
        }

        info!(target: "p2p" , "p2p shutdown finished");
    }

    pub fn get_net_id(&self) -> u32 { self.config.net_id }

    /// rechieve a random node with td >= target_td
    pub fn get_node_by_td(&self, _target_td: u64) -> u64 { 120 }

    /// get copy of active nodes as vec
    pub fn get_active_nodes(&self) -> Vec<Node> {
        let mut active_nodes: Vec<Node> = Vec::new();
        if let Ok(read) = &self.nodes.read() {
            for node in read.values() {
                if node.state == STATE::ACTIVE {
                    active_nodes.push(node.clone())
                }
            }
        }
        active_nodes
    }

    /// get randome active node hash
    pub fn get_random_active_node_hash(&self) -> Option<u64> {
        let active: Vec<Node> = self.get_active_nodes();
        let len: usize = active.len();
        if len > 0 {
            let random = random::<usize>() % len;
            Some(active[random].get_hash())
        } else {
            None
        }
    }

    /// get random active node
    pub fn get_random_active_node(&self, filter: &[u64]) -> Option<Node> {
        let active: Vec<Node> = self.get_active_nodes();
        let free_node: Vec<_> = active
            .iter()
            .filter(move |x| !filter.contains(&x.hash))
            .map(|x| x)
            .collect();
        let len: usize = free_node.len();
        if len > 0 {
            let random = random::<usize>() % len;
            Some(free_node[random].clone())
        } else {
            None
        }
    }

    /// get total nodes count
    pub fn get_statics_info(&self) -> (usize, HashMap<u64, (String, String, String, &str)>) {
        let mut len = 0;
        let mut statics_info = HashMap::new();
        if let Ok(read) = self.nodes.read() {
            len = read.len();
            for node in read.values() {
                if node.state == STATE::ACTIVE {
                    statics_info.insert(
                        node.get_hash(),
                        (
                            node.addr.to_formatted_string(),
                            format!("{}", String::from_utf8_lossy(&node.revision).trim()),
                            format!("{}", node.connection),
                            match node.if_seed {
                                true => "y",
                                _ => " ",
                            },
                        ),
                    );
                }
            }
        }
        (len, statics_info)
    }

    /// get total active nodes count
    pub fn get_active_nodes_len(&self) -> u32 {
        let mut len: u32 = 0;
        if let Ok(read) = &self.nodes.try_read() {
            for node in read.values() {
                if node.state == STATE::ACTIVE {
                    len += 1;
                }
            }
        }
        len
    }

    /// get node by hash
    pub fn get_node(&self, hash: &u64) -> Option<Node> {
        match &self.nodes.read() {
            Ok(read) => {
                match read.get(hash) {
                    Some(node) => Some(node.clone()),
                    None => {
                        warn!(target: "p2p", "get_node: node not found: hash {}", hash);
                        None
                    }
                }
            }
            Err(err) => {
                warn!(target: "p2p", "get_node: {:?}", err);
                None
            }
        }
    }

    /// refresh node timestamp in order to keep target in loop
    /// otherwise, target will be timeout and removed
    pub fn update_node(&self, hash: &u64) {
        if let Ok(mut nodes) = self.nodes.try_write() {
            if let Some(mut node) = nodes.get_mut(hash) {
                node.update();
            } else {
                warn!(target:"p2p", "node {} is timeout before update", hash)
            }
        }
    }

    pub fn get_local_node_info(&self) -> &String { &self.config.local_node }

    /// messages with module code other than p2p module
    /// should flow into external handlers
    fn handle(&self, hash: u64, cb: ChannelBuffer) {
        let p2p = self.clone();
        debug!(target: "p2p", "handle: hash/ver/ctrl/action/route {}/{}/{}/{}/{}", &hash, cb.head.ver, cb.head.ctrl, cb.head.action, cb.head.get_route());
        // verify if flag token has been set
        let mut pass = false;
        {
            if let Ok(mut lock) = self.nodes.write() {
                if let Some(mut node) = lock.get_mut(&hash) {
                    let clear_token = cb.head.get_route();
                    pass = self.token_check(clear_token, node);
                }
            }
        }

        if pass {
            match Version::from(cb.head.ver) {
                Version::V0 => {
                    match Module::from(cb.head.ctrl) {
                        Module::P2P => {
                            match Action::from(cb.head.action) {
                                Action::HANDSHAKEREQ => handshake::receive_req(p2p, hash, cb),
                                Action::HANDSHAKERES => handshake::receive_res(p2p, hash, cb),
                                Action::ACTIVENODESREQ => {
                                    active_nodes::receive_req(p2p, hash, cb.head.ver)
                                }
                                Action::ACTIVENODESRES => active_nodes::receive_res(p2p, hash, cb),
                                _ => trace!(target: "p2p", "invalid action {}", cb.head.action),
                            };
                        }
                        Module::SYNC => {
                            if let Ok(lock) = p2p.callback.read() {
                                if let Some(ref callback) = *lock {
                                    match Weak::upgrade(callback) {
                                        Some(arc_callback) => arc_callback.handle(hash, cb),
                                        None => warn!(target: "p2p", "sync has been shutdown?" ),
                                    }
                                }
                            }
                        }
                        Module::UNKNOWN => trace!(target: "p2p", "invalid ctrl {}", cb.head.ctrl),
                    }
                }
                //Version::V1 => handshake::send(p2p, hash),
                _ => trace!(target: "p2p", "invalid version {}", cb.head.ver),
            };
        } else {
            warn!(target: "p2p", "handle: hash/ver/ctrl/action {}/{}/{}/{}", &hash, cb.head.ver, cb.head.ctrl, cb.head.action);
        }
    }
}

/// helper function for setting inbound & outbound stream
fn config_stream(stream: &TcpStream) -> Result<(), io::Error> {
    stream.set_recv_buffer_size(1 << 24)?;
    stream.set_keepalive(Some(Duration::from_secs(TIMEOUT_MAX)))?;

    Ok(())
}

/// helper function for tokio io frame
fn split_frame(
    socket: TcpStream,
) -> (
    stream::SplitSink<Framed<TcpStream, Codec>>,
    stream::SplitStream<Framed<TcpStream, Codec>>,
) {
    Codec.framed(socket).split()
}

#[cfg(test)]
mod tests {

    use std::net::SocketAddr;
    use futures::Future;
    use tokio::net::TcpStream;
    use futures::sync::{mpsc,oneshot};
    use Mgr;
    use node::Node;
    use config::Config;
    use super::PROTOCAL_VERSION;

    #[test]
    fn test_version() {
        assert_eq!(PROTOCAL_VERSION, 0u16);
    }

    #[test]
    pub fn test_tokens() {
        let addr = "168.62.170.146:30303".parse::<SocketAddr>().unwrap();
        let stream = TcpStream::connect(&addr);
        let _ = stream.map(move |ts| {
            let mut tokens_rules: Vec<[u32; 2]> = vec![];
            let flat_token_0: u32 = (0 << 16) + (0 << 8) + 0;
            let clear_token_0: u32 = (0 << 16) + (0 << 8) + 1;
            tokens_rules.push([flat_token_0, clear_token_0]);
            let p2p = Mgr::new(Config::new(), tokens_rules);

            let (tx, _rx) = mpsc::channel(409600);
            let (tx_thread, _rx_thread) = oneshot::channel::<()>();
            let mut node = Node::new_outbound(
                ts,
                tx,
                [
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                ],
                false,
                tx_thread,
            );
            node.tokens.insert(flat_token_0);

            let node_hash = node.get_hash();

            let nodes_0 = p2p.nodes.clone();
            if let Ok(mut lock) = nodes_0.write() {
                lock.insert(node_hash.clone(), node);
            }

            let nodes_1 = p2p.nodes.clone();
            if let Ok(mut lock) = nodes_1.write() {
                if let Some(mut node) = lock.get_mut(&node_hash) {
                    p2p.token_check(clear_token_0, node);
                    assert_eq!(node.tokens.len(), 0);
                }
            }

            p2p.shutdown();
        });
    }

}
