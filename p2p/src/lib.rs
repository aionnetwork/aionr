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
use std::sync::Arc;
use std::collections::VecDeque;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::Duration;
use std::time::SystemTime;
use std::time::Instant;
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
use tokio::runtime::Runtime;
use tokio::timer::Interval;
use tokio_codec::{Decoder,Framed};
use codec::Codec;
use route::VERSION;
use route::MODULE;
use route::ACTION;
use state::STATE;
use handler::handshake;
use handler::active_nodes;
use node::TempNode;

pub use msg::ChannelBuffer;
pub use node::Node;
pub use config::Config;
pub use callable::Callable;

const INTERVAL_OUTBOUND_CONNECT: u64 = 10;
const INTERVAL_TIMEOUT: u64 = 5;
const INTERVAL_ACTIVE_NODES: u64 = 3;
const TIMEOUT_MAX: u64 = 30;
const TEMP_MAX: usize = 64;

#[derive(Clone)]
pub struct Mgr {
    /// threading
    //runtime: Arc<Runtime>,
    //runtime: Runtime,
    shutdown_hook: Arc<RwLock<Option<Sender<()>>>>,
    /// config
    config: Arc<Config>,
    /// temp queue storing seeds and active nodes queried from other nodes
    temp: Arc<Mutex<VecDeque<TempNode>>>,
    /// nodes
    nodes: Arc<RwLock<HashMap<u64, Node>>>,
    /// tokens rule
    tokens_rule: Arc<HashMap<u32, u32>>,
}

impl Mgr {
    /// constructor
    pub fn new(config: Arc<Config>, tokens_pairs: Vec<[u32; 2]>) -> Mgr {
        // load seeds
        let mut temp_queue = VecDeque::<TempNode>::with_capacity(TEMP_MAX);
        for boot_node_str in config.boot_nodes.clone() {
            info!(target: "run", "        seed: {}", &boot_node_str);
            temp_queue.push_back(TempNode::new_from_str(boot_node_str.to_string()));
        }

        // parse token rules
        let mut tokens_rule: HashMap<u32, u32> = HashMap::new();
        for pair in tokens_pairs {
            if pair[0] != pair[1] {
                if !tokens_rule.contains_key(&pair[0]) {
                    tokens_rule.insert(pair[0], pair[1]);
                }
                if !tokens_rule.contains_key(&pair[1]) {
                    tokens_rule.insert(pair[1], pair[0]);
                }
            }
        }

        Mgr {
            shutdown_hook: Arc::new(RwLock::new(None)),
            config,
            temp: Arc::new(Mutex::new(temp_queue)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            tokens_rule: Arc::new(tokens_rule),
        }
    }

    /// verify inbound msg route through token collection
    /// 1. pass in incoming msg route through token rules against token collection on indivisual node
    /// 2. return
    /// 3. should
    pub fn token_check(&self) {}

    /// run p2p instance
    pub fn run(&mut self, callback: Arc<Callable>) {
        // init
        let mut rt = Runtime::new().unwrap();
        let executor = rt.executor();
        let binding: SocketAddr = self
            .config
            .get_id_and_binding()
            .1
            .parse::<SocketAddr>()
            .unwrap()
            .clone();

        let callback_in = callback.clone();
        let callback_out = callback.clone();

        // interval timeout
        let executor_timeout = executor.clone();
        let callback_timeout = callback.clone();
        let p2p_timeout = self.clone();
        executor_timeout.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_TIMEOUT)
            ).for_each(move|_|{

                let now = SystemTime::now();
                let mut index: Vec<u64> = vec![];
                if let Ok(mut write) = p2p_timeout.nodes.try_write(){
                    for (hash, node) in write.iter_mut() {
                        if now.duration_since(node.update).expect("SystemTime::duration_since failed").as_secs() >= TIMEOUT_MAX {
                            index.push(*hash);
                        }
                        // resend handshake
                        // else if node.state == STATE::CONNECTED && node.connection == Connection::INBOUND {
                        //     handshake::send(&hash, node.id, net_id, ip, port, nodes_outbound_6);
                        // }
                    }

                    for i in 0 .. index.len() {
                        let hash = index[i];
                        match write.remove(&hash) {
                            Some(mut node) => {
                                node.tx.close().unwrap();

                                // dispatch node remove event
                                callback_timeout.disconnect(hash.clone());
                                debug!(target: "p2p", "timeout hash/id/ip {}/{}/{}", &node.get_hash(), &node.get_id_string(), &node.addr.get_ip());
                            },
                            None => {}
                        }
                    }
                }
                Ok(())
            }).map_err(|err| error!(target: "p2p", "executor timeout: {:?}", err))
        );

        // interval outbound
        let executor_outbound = executor.clone();
        let executor_outbound_0 = executor_outbound.clone();
        let p2p_outbound = self.clone();
        executor_outbound.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_OUTBOUND_CONNECT)
            ).for_each(move|_|{

                let p2p_outbound_0 = p2p_outbound.clone();
                let callback_out = callback_out.clone();

                // exist lock immediately after poping temp node
                let mut temp_node_opt: Option<TempNode> = None;
                {
                    if let Ok(mut lock) = p2p_outbound_0.temp.try_lock() {

                        if let Some(temp_node) = lock.pop_front() {
                            temp_node_opt = Some(temp_node.clone());

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
                        executor_outbound_0.spawn(lazy(move||{
                            TcpStream::connect(&addr)
                            .map(move |ts: TcpStream| {
                                debug!(target: "p2p", "connected to: {}", &temp_node.addr.to_string());

                                let p2p_outbound_1 = p2p_outbound_0.clone();
                                let p2p_outbound_2 = p2p_outbound_0.clone();

                                // config stream
                                config_stream(&ts);

                                // construct node instance and store it
                                let (tx, rx) = mpsc::channel(409600);
                                let ts_0 = ts.try_clone().unwrap();
                                let node = Node::new_outbound(
                                    ts.peer_addr().unwrap(),
                                    ts_0,
                                    tx,
                                    temp_node.id,
                                    temp_node.if_seed
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

                                // binding io futures
                                let (sink, stream) = split_frame(ts);
                                let read = stream.for_each(move |cb| {
                                    p2p_outbound_2.handle(hash.clone(),cb,callback_out.clone());
                                    Ok(())
                                }).map_err(move|err|{error!(target: "p2p", "read: {:?}", err)});

                                executor_outbound_1.spawn(read.then(|_|{ Ok(()) }));
                                let write = sink.send_all(
                                    rx.map_err(|()| io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")),
                                );
                                executor_outbound_2.spawn(write.then(|_| { Ok(()) }));

                                // send handshake request
                                executor_outbound_3.spawn(lazy(move||{
                                    handshake::send(p2p_outbound_1, hash);
                                    Ok(())
                                }));

                            }).map_err(move|err|{error!(target: "p2p", "connect node: {:?}", err)})
                        }));
                    }
                }
                Ok(())
            }).map_err(|err| error!(target: "p2p", "executor outbound: {:?}", err))
        );

        // interval active nodes
        let executor_active_nodes = executor.clone();
        let p2p_active_nodes = self.clone();
        executor_active_nodes.spawn(
            Interval::new(Instant::now(), Duration::from_secs(INTERVAL_ACTIVE_NODES))
                .for_each(move |_| {
                    let p2p_active_nodes_0 = p2p_active_nodes.clone();
                    active_nodes::send(p2p_active_nodes_0);
                    Ok(())
                })
                .map_err(|err| error!(target: "p2p", "executor active nodes: {:?}", err)),
        );

        // interval inbound
        let executor_inbound_0 = executor.clone();
        let executor_inbound_1 = executor.clone();
        let p2p_inbound = self.clone();
        let listener = TcpListener::bind(&binding).expect("binding failed");
        let server = listener
            .incoming()
            .for_each(move |ts: TcpStream| {
                // counters
                let p2p_inbound_0 = p2p_inbound.clone();
                let p2p_inbound_1 = p2p_inbound.clone();
                let callback_in = callback_in.clone();

                // TODO: black list check
                if p2p_inbound.get_active_nodes_len() >= p2p_inbound.config.max_peers {
                    debug!(target:"p2p", "max peers reached");
                    return Ok(());
                }

                // config stream
                config_stream(&ts);

                // construct node instance and store it
                let (tx, rx) = mpsc::channel(409600);
                let addr = ts.peer_addr().unwrap();
                let ts_0 = ts.try_clone().unwrap();
                let node = Node::new_inbound(
                    addr,
                    ts_0,
                    tx,
                    false
                );
                let hash = node.get_hash();

                if let Ok(mut write) = p2p_inbound.nodes.try_write() {
                    let id: String = node.get_id_string();
                    let binding: String = node.addr.to_string();
                    if !write.contains_key(&node.get_hash()){
                        if let None = write.insert(node.get_hash(), node) {
                            debug!(target: "p2p", "inbound node added: hash/id/ip {:?}/{:?}/{:?}", &hash, &id, &binding);
                        }
                    }
                }

                // binding io futures
                let (sink, stream) = split_frame(ts);
                let read = stream.for_each(move |cb| {
                    p2p_inbound_1.handle(hash.clone(),cb,callback_in.clone());
                    Ok(())
                });
                executor_inbound_0.spawn(read.then(|_| { Ok(()) }));
                let write = sink.send_all(rx.map_err(|()| {
                    io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")
                }));
                executor_inbound_1.spawn(write.then(|_| { Ok(()) }));
                Ok(())
            }).map_err(|err| error!(target: "p2p", "executor server: {:?}", err));

        // bind shutdown hook
        let (tx, rx) = oneshot::channel::<()>();
        {
            match self.shutdown_hook.write() {
                Ok(mut guard) => *guard = Some(tx),
                Err(_error) => {}
            }
        }

        // clear
        rt.block_on(rx.map_err(|_| ())).unwrap();
        rt.shutdown_now().wait().unwrap();
        drop(server);
        drop(executor_timeout);
        drop(executor_active_nodes);
        drop(executor_outbound);
        drop(executor);
        debug!(target:"p2p", "shutdown executors");
    }

    /// shutdown routine
    pub fn shutdown(&self) {
        if let Ok(mut lock) = self.nodes.write() {
            for (_hash, mut node) in lock.iter_mut() {
                match node.ts.shutdown(Shutdown::Both) {
                    Ok(_) => {
                        trace!(target: "p2p", "close connection id/ip {}/{}", &node.get_id_string(), &node.get_id_string());
                    }
                    Err(_err) => {}
                }
            }
            lock.clear();
        }

        if let Ok(mut lock) = self.shutdown_hook.write() {
            if lock.is_some() {
                let tx = lock.take().unwrap();
                match tx.send(()) {
                    Ok(_) => {
                        debug!(target: "p2p", "shutdown signal sent");
                    }
                    Err(err) => {
                        error!(target: "p2p", "shutdown: {:?}", err);
                    }
                }
            }
        }
    }

    /// rechieve a random node with td >= target_td
    pub fn get_node_by_td(&self, _target_td: u64) -> u64 { 120 }

    /// send msg
    pub fn send(&self, hash: u64, cb: ChannelBuffer) {
        let mut nodes = &self.nodes;
        match nodes.try_write() {
            Ok(mut lock) => {
                let mut flag = true;
                if let Some(node) = lock.get(&hash) {
                    let mut tx = node.tx.clone();
                    match tx.try_send(cb) {
                        Ok(_) => trace!(target: "p2p", "p2p/send: {}", node.addr.get_ip()),
                        Err(err) => {
                            flag = false;
                            error!(target: "p2p", "p2p/send ip:{} err:{}", node.addr.get_ip(), err);
                        }
                    }
                } else {
                    warn!(target:"p2p", "send: node not found hash {}", hash);
                }
                if !flag {
                    if let Some(node) = lock.remove(&hash) {
                        trace!(target: "p2p", "failed send, remove hash/id {}/{}", node.get_id_string(), node.addr.get_ip());
                    }
                }
            }
            Err(err) => {
                warn!(target:"p2p", "send: nodes read {:?}", err);
            }
        }
    }

    /// get copy of active nodes as vec
    pub fn get_active_nodes(&self) -> Vec<Node> {
        let mut active_nodes: Vec<Node> = Vec::new();
        if let Ok(read) = &self.nodes.try_read() {
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
        if let Ok(mut nodes) = self.nodes.write() {
            if let Some(mut node) = nodes.get_mut(hash) {
                node.update();
            } else {
                warn!(target:"p2p", "node {} is timeout before update", hash)
            }
        }
    }

    /// messages with module code other than p2p module
    /// should flow into external handlers
    fn handle(&self, hash: u64, cb: ChannelBuffer, callable: Arc<Callable>) {
        trace!(target: "p2p", "handle: hash/ver/ctrl/action {}/{}/{}/{}", &hash, cb.head.ver, cb.head.ctrl, cb.head.action);
        let p2p = self.clone();
        match VERSION::from(cb.head.ver) {
            VERSION::V0 => {
                match MODULE::from(cb.head.ctrl) {
                    MODULE::P2P => {
                        match ACTION::from(cb.head.action) {
                            ACTION::HANDSHAKEREQ => handshake::receive_req(p2p, hash, cb),
                            ACTION::HANDSHAKERES => handshake::receive_res(p2p, hash, cb),
                            ACTION::ACTIVENODESREQ => active_nodes::receive_req(p2p, hash),
                            ACTION::ACTIVENODESRES => active_nodes::receive_res(p2p, hash, cb),
                            ACTION::DISCONNECT => {}
                            _ => error!(target: "p2p", "invalid action {}", cb.head.action),
                        };
                    }
                    MODULE::EXTERNAL => {
                        callable.handle(hash, cb);
                    }
                }
            }
            //VERSION::V1 => handshake::send(p2p, hash),
            _ => error!(target: "p2p", "invalid version code"),
        };
    }
}

/// helper function for config inbound & outbound stream
fn config_stream(stream: &TcpStream) {
    stream
        .set_recv_buffer_size(1 << 24)
        .expect("set_recv_buffer_size failed");
    stream
        .set_keepalive(Some(Duration::from_secs(TIMEOUT_MAX)))
        .expect("set_keepalive failed");
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
