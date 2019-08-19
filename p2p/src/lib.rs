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
extern crate lru_cache;

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
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::{Mutex,RwLock};
use std::time::Duration;
use std::time::SystemTime;
use std::time::Instant;
use std::net::SocketAddr;
use lru_cache::LruCache;
use rand::random;
use futures::sync::mpsc;
use futures::{Future, Stream};
use futures::lazy;
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
use aion_types::H256;

pub use msg::ChannelBuffer;
pub use node::Node;
pub use config::Config;
pub use callable::Callable;

const INTERVAL_STATISICS: u64 = 5;
const INTERVAL_OUTBOUND_CONNECT: u64 = 10;
const INTERVAL_TIMEOUT: u64 = 5;
const INTERVAL_ACTIVE_NODES: u64 = 3;
const TIMEOUT_MAX: u64 = 30;
const TEMP_MAX: usize = 64;

pub struct Mgr {
    /// threading
    runtime: Arc<Runtime>,
    /// config
    config: Arc<Config>,
    /// temp queue storing seeds and active nodes queried from other nodes
    temp: Arc<Mutex<VecDeque<TempNode>>>,
    /// nodes
    pub nodes: Arc<RwLock<HashMap<u64, Node>>>,
}

impl Mgr {
    /// construct p2p instance
    pub fn new(config: Arc<Config>) -> Mgr {
        // construct seeds
        let mut temp_queue = VecDeque::<TempNode>::with_capacity(TEMP_MAX);
        for boot_node_str in config.boot_nodes.clone() {
            info!(target: "run", "        seed: {}", &boot_node_str);
            temp_queue.push_back(TempNode::new_from_str(boot_node_str.to_string()));
        }

        // return instance
        Mgr {
            runtime: Arc::new(Runtime::new().expect("tokio runtime")),
            config,
            temp: Arc::new(Mutex::new(temp_queue)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// run p2p instance
    pub fn run(
        &self,
        p2p: Arc<Mgr>, 
        callbacks: Arc<Callable>,
    ){
        // init
        let executor = Arc::new(self.runtime.executor());
        let binding: SocketAddr = self
            .config
            .get_id_and_binding()
            .1
            .parse::<SocketAddr>()
            .unwrap()
            .clone();


        // interval statisics
        let executor_statisics = executor.clone();
        let p2p_statisics = p2p.clone();
        executor_statisics.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_STATISICS)
            ).for_each(move |_| {
                match p2p_statisics.nodes.try_read() {
                    Ok(nodes) => {
                        let mut total: usize = 0;
                        let mut active: usize = 0;
                        if nodes.len() > 0 {
                            let mut active_nodes = vec![];
                            info!(target: "p2p", "{:-^127}","");
                            info!(target: "p2p","              td         bn          bh                    addr                 rev      conn  seed");
                            info!(target: "p2p", "{:-^127}","");

                            for (_hash, node) in nodes.iter(){
                                total += 1;
                                if node.state == STATE::ACTIVE {
                                    active += 1;
                                    active_nodes.push(node.clone());
                                }
                            }

                            if active_nodes.len() > 0 {
                                active_nodes.sort_by(|a, b| {
                                    if a.total_difficulty != b.total_difficulty {
                                        b.total_difficulty.cmp(&a.total_difficulty)
                                    } else {
                                        b.block_num.cmp(&a.block_num)
                                    }
                                });
                                for node in active_nodes.iter() {
                                    info!(target: "p2p",
                                        "{:>16}{:>11}{:>12}{:>24}{:>20}{:>10}{:>6}",
                                        format!("{}",node.total_difficulty),
                                        node.block_num,
                                        format!("{}",node.block_hash),
                                        node.addr.to_formatted_string(),
                                        String::from_utf8_lossy(&node.revision).trim(),
                                        format!("{}",node.connection),
                                        match node.if_seed{
                                            true => "y",
                                            _ => " "
                                        }
                                    );
                                }

                            }

                            info!(target: "p2p", "{:-^127}","");
                        }
                        info!(target: "p2p", "total/active {}/{}", total, active);
                    },
                    Err(err) => {
                        warn!(target:"p2p", "executor statisics: try read {:?}", err);
                    }
                }
                Ok(())
            }).map_err(|err| error!(target: "p2p", "executor statisics: {:?}", err))
        );

        // interval timeout
        let executor_timeout = executor.clone();   
        let p2p_timeout = p2p.clone();  
        executor_timeout.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_TIMEOUT)
            ).for_each(move|_|{
                let now = SystemTime::now();
                if let Ok(mut write) = p2p_timeout.nodes.try_write(){
                    let mut index: Vec<u64> = vec![];
                    for (hash, node) in write.iter_mut() {
                        if now.duration_since(node.update).expect("SystemTime::duration_since failed").as_secs() >= TIMEOUT_MAX {
                            index.push(*hash);
                            match node.tx.close(){
                                Ok(_) => {
                                    debug!(target: "p2p", "tx close");
                                },
                                Err(err) => {
                                    error!(target: "p2p", "tx close: {}", err);
                                }
                            }
                        }
                        // else if node.state == STATE::CONNECTED && node.connection == Connection::INBOUND {
                        //     handshake::send(&hash, node.id, net_id, ip, port, nodes_outbound_6);
                        // }
                    }

                    for i in 0 .. index.len() {
                        match write.remove(&index[i]) {
                            Some(mut node) => {
                                node.tx.close().unwrap();
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
        let p2p_outbound = p2p.clone();
        executor_outbound.spawn(
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
                                
                                // config stream
                                config_stream(&ts);

                                // construct node instance and store it
                                let (tx, rx) = mpsc::channel(409600);
                                let node = Node::new_outbound(ts.peer_addr().unwrap(), tx, temp_node.id, temp_node.if_seed);
                                let id = node.id.clone();
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
                                    if let Some(node) = p2p_outbound_0.get_node(&hash) {
                                        // handle(
                                        //     hash.clone(), 
                                        //     cb, 
                                        //     config_outbound_4,  
                                        //     handlers_out3.clone(),
                                        //     temp_outbound_2, 
                                        //     nodes_outbound_5.clone(), 
                                        //     headers_out.clone(),
                                        // );
                                    }
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
        let p2p_active_nodes = p2p.clone();
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
        let executor_inbound = executor.clone();
        let executor_inbound_0 = executor_inbound.clone();
        let p2p_inbound = p2p.clone();
        let listener = TcpListener::bind(&binding).expect("binding failed");
        let server = listener
            .incoming()
            .for_each(move |ts: TcpStream| {

                // counters
                let p2p_inbound_0 = p2p_inbound.clone();

                // TODO: black list check
                if p2p_inbound.get_active_nodes_len() >= p2p_inbound.config.max_peers {
                    debug!(target:"p2p", "max peers reached");
                    return Ok(());
                }

                // counters
                let executor_inbound_1 = executor_inbound_0.clone();
                let executor_inbound_2 = executor_inbound_0.clone();

                // config stream
                config_stream(&ts);

                // construct node instance and store it
                let (tx, rx) = mpsc::channel(409600);
                let addr = ts.peer_addr().unwrap();
                let node = Node::new_inbound(addr, tx, false);
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
                    if let Some(node) = p2p_inbound_0.get_node(&hash) {       
                        // handle(
                        //     hash, 
                        //     cb, 
                        //     config_inbound_2, 
                        //     handlers_in1.clone(),
                        //     temp_inbound_2, 
                        //     nodes_inbound_3, 
                        //     headers_in.clone(),


                        // );
                    }
                    Ok(())
                });
                executor_inbound_0.spawn(read.then(|_| { Ok(()) }));
                let write = sink.send_all(rx.map_err(|()| {
                    io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")
                }));
                executor_inbound_1.spawn(write.then(|_| { Ok(()) }));
                Ok(())
            }).map_err(|err| error!(target: "p2p", "executor server: {:?}", err));
        executor_inbound.spawn(server);
    }

    /// shutdown p2p instance
    // TODO: test
    pub fn shutdown(&self) {
        // let runtime = self.runtime.clone();
        // match runtime.shutdown_now().wait(){
        //     Ok(_) => {
        //         info!(target: "p2p", "shutdown");
        //     },
        //     Err(err) => {
        //         error!(target: "p2p", "shutdown failed: {:?}", err);
        //     }
        // }
    }

    /// rechieve a random node with td >= target_td
    pub fn get_node_by_td(&self, target_td: u64) -> u64 { 120 }

    /// send msg
    pub fn send(&self, p2p: Arc<Mgr>, hash: u64, cb: ChannelBuffer) {
        
        // TODO: solve issue msg lost
        match p2p.nodes.try_write() {
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

    pub fn get_random_active_node(&self) -> Option<Node> {
        let active: Vec<Node> = self.get_active_nodes();
        let len: usize = active.len();
        if len > 0 {
            let random = random::<usize>() % len;
            Some(active[random].clone())
        } else {
            None
        }
    }

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

    /// messages with module code other than p2p module
    /// should flow into external handlers
    fn handle(
        p2p: Arc<Mgr>,
        hash: u64,
        cb: ChannelBuffer,
        callable: Arc<Callable>
    ) {
        trace!(target: "p2p", "handle: hash/ver/ctrl/action {}/{}/{}/{}", &hash, cb.head.ver, cb.head.ctrl, cb.head.action);
        match VERSION::from(cb.head.ver) {
            VERSION::V0 => {
                match MODULE::from(cb.head.ctrl) {
                    MODULE::P2P => {
                        match ACTION::from(cb.head.action) {
                            ACTION::HANDSHAKEREQ   => handshake::receive_req(p2p, hash, cb),
                            ACTION::HANDSHAKERES   => handshake::receive_res(p2p, hash, cb),
                            ACTION::ACTIVENODESREQ => active_nodes::receive_req(p2p, hash),
                            ACTION::ACTIVENODESRES => active_nodes::receive_res(p2p, hash, cb),
                            _ => error!(target: "p2p", "invalid action {}", cb.head.action),
                        };
                    }
                    MODULE::EXTERNAL => {
                        callable.handle(hash, cb);
                        // handle(
                        //     p2p,
                        //     hash, 
                        //     cb, 
                        // );
                    }
                }
            }
            //VERSION::V1 => handshake::send(p2p, hash),
            _ => error!(target: "p2p", "invalid version code"),
        };
    }
}

fn config_stream(stream: &TcpStream) {
    stream
        .set_recv_buffer_size(1 << 24)
        .expect("set_recv_buffer_size failed");
    stream
        .set_keepalive(Some(Duration::from_secs(TIMEOUT_MAX)))
        .expect("set_keepalive failed");
}

fn split_frame(
    socket: TcpStream,
) -> (
    stream::SplitSink<Framed<TcpStream, Codec>>,
    stream::SplitStream<Framed<TcpStream, Codec>>,
) {
    Codec.framed(socket).split()
}
