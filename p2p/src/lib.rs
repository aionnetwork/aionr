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
mod event;
mod codec;
mod state;
mod handler;

use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::collections::VecDeque;
use std::sync::{Mutex,RwLock};
use std::time::Duration;
use std::time::SystemTime;
use std::time::Instant;
use std::net::SocketAddr;
use futures::sync::mpsc;
use futures::{Future, Stream};
use futures::lazy;
use tokio::net::{TcpListener,TcpStream};
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
pub use handler::external::Handler;
pub use msg::ChannelBuffer;
pub use node::Node;
pub use config::Config;

const INTERVAL_STATISICS: u64 = 5;
const INTERVAL_OUTBOUND_CONNECT: u64 = 10;
const INTERVAL_TIMEOUT: u64 = 5;
const INTERVAL_ACTIVE_NODES: u64 = 10;
const TIMEOUT_MAX: u64 = 30;
const TEMP_MAX: usize = 64;

pub struct Mgr{
    
    /// threading
    runtime: Runtime,
    /// config
    config: Config,
    /// temp queue storing seeds and active nodes queried from other nodes
    temp: Arc<Mutex<VecDeque<TempNode>>>, 
    /// nodes
    nodes: Arc<RwLock<HashMap<u64, Node>>>
}

impl Mgr {

    // construct p2p instance
    pub fn new(config: Config) -> Mgr{
        
        // construct seeds
        let mut temp_queue = VecDeque::<TempNode>::with_capacity(TEMP_MAX);
        for boot_node_str in config.boot_nodes.clone() {
            info!(target: "run", "        seed: {}", &boot_node_str);
            temp_queue.push_back(TempNode::new_from_str(boot_node_str.to_string()));
        }

        // return instance
        Mgr {
            runtime: Runtime::new().expect("tokio runtime"),
            config: config,
            temp: Arc::new(Mutex::new(temp_queue)),
            nodes: Arc::new(RwLock::new(HashMap::new()))
        }
    }

    // run p2p instance
    pub fn run(&self){
        
        // setup
        let executor = Arc::new(self.runtime.executor()); 
        let binding: SocketAddr = self.config.get_id_and_binding().1.parse::<SocketAddr>().unwrap().clone(); 
        let config = Arc::new(self.config.clone());
        let nodes = self.nodes.clone();
        let temp = self.temp.clone();
        
        // statisics
        let executor_statisics = executor.clone();
        let nodes_statisics = nodes.clone();
        executor_statisics.spawn(
            Interval::new(
                Instant::now(), 
                Duration::from_secs(INTERVAL_STATISICS)
            ).for_each(move |_| {
                if let Ok(nodes) = nodes_statisics.try_read() {
                    let mut total: usize = 0;
                    let mut active: usize = 0;
                    if nodes.len() > 0 {
                        let mut active_nodes = vec![];
                        info!(target: "p2p", "{:-^127}","");
                        info!(target: "p2p","              td         bn          bh                    addr                 rev      conn  seed");
                        info!(target: "p2p", "{:-^127}","");

                        for (hash, node) in nodes.iter(){
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
                            let mut count: u32 = 0;
                            for node in active_nodes.iter() {
                                info!(target: "p2p",
                                    "{:>16}{:>11}{:>12}{:>24}{:>20}{:>8}{:>6}",
                                    format!("{}",node.total_difficulty),
                                    node.block_num,
                                    format!("{}",node.block_hash),
                                    node.addr.to_formatted_string(),
                                    String::from_utf8_lossy(&node.revision).trim(),
                                    node.connection,
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
                }             
                Ok(())   
            }).map_err(|err| error!(target: "p2p", "executor statisics: {:?}", err))
        );
        
        // timeout
        let executor_timeout = executor.clone();
        let nodes_timeout = nodes.clone();
        executor_timeout.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_TIMEOUT)
            ).for_each(move|_|{
                let now = SystemTime::now();
                if let Ok(mut write) = nodes_timeout.try_write(){
                    let mut index: Vec<u64> = vec![];
                    for (hash, node) in write.iter_mut() {
                        if now.duration_since(node.update).expect("SystemTime::duration_since failed").as_secs() >= TIMEOUT_MAX { 
                            debug!(target: "p2p", "timeout {} {}", &hash, &node.addr.get_ip());
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
                    }

                    for i in 0 .. index.len() {
                        match write.remove(&index[i]) {
                            Some(node) => {
                                debug!(target: "p2p", "timeout {} {}", node.get_id_string(), node.addr.get_ip());
                            },
                            None => {}
                        }
                    }
                }
                Ok(())
            }).map_err(|err| error!(target: "p2p", "executor timeout: {:?}", err))
        );

        // outbound
        let config_outbound = config.clone();
        let executor_outbound = executor.clone();
        let executor_outbound_0 = executor_outbound.clone();       
        let temp_outbound_0 = temp.clone();
        let nodes_outbound_0 = nodes.clone(); 
        // TODO: batch outbound connecting     
        executor_outbound.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_OUTBOUND_CONNECT)
            ).for_each(move|_|{

                // counters
                let config_outbound_0 = config_outbound.clone();
                let temp_outbound_1 = temp_outbound_0.clone();
                let nodes_outbound_1 = nodes_outbound_0.clone();
                
                // exist lock immediately after poping temp node
                let mut temp_node_opt: Option<TempNode> = None;
                {
                    if let Ok(mut lock) = temp.try_lock() {
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
                    let hash = temp_node.get_hash();

                    // return if exist                        
                    {
                        if let Ok(read) = nodes_outbound_1.try_read(){
                            // return at node existing
                            if let Some(node) = read.get(&hash) {
                                debug!(target: "p2p", "exist {}", node.get_id_string());
                                return Ok(());
                            }
                        }
                    } 
            
                    // counters 
                    let nodes_outbound_2 = nodes_outbound_1.clone();
                    let executor_outbound_1 = executor_outbound_0.clone();
                    let executor_outbound_2 = executor_outbound_0.clone();
                    let executor_outbound_3 = executor_outbound_0.clone();
                      
                    if let Ok(addr) = temp_node.addr.to_string().parse() { 
                        debug!(target: "p2p", "connecting to: {}", &addr);                     
                        executor_outbound_0.spawn(lazy(move||{
                            let config_outbound_1 = config_outbound_0;
                            TcpStream::connect(&addr)                         
                            .map(move |ts: TcpStream| {     
                                debug!(target: "p2p", "connected to: {}", &temp_node.addr.to_string());  

                                // counters                              
                                let config_outbound_2 = config_outbound_1.clone();
                                let nodes_outbound_3 = nodes_outbound_2.clone();
                                let nodes_outbound_4 = nodes_outbound_2.clone();
                                let nodes_outbound_5 = nodes_outbound_2.clone();
                                let nodes_outbound_6 = nodes_outbound_2.clone();
                                
                                // config stream
                                config_stream(&ts);

                                // construct node instance and store it
                                let (tx, rx) = mpsc::channel(409600);
                                let node = Node::new_outbound(ts.peer_addr().unwrap(), tx, temp_node.id, temp_node.if_seed);
                                let id = node.id.clone();
                                if let Ok(mut write) = nodes_outbound_3.try_write() {
                                    if !write.contains_key(&hash) {
                                        let hash_0 = hash.clone();
                                        let id = node.get_id_string();
                                        let ip = node.addr.get_ip();
                                        if let None = write.insert(hash, node) {
                                            debug!(target: "p2p", "outbound node added: {} {} {}", hash_0, id, ip);
                                        }
                                    }
                                }                     

                                // binding io futures   
                                let config_outbound_3 = config_outbound_2.clone();  
                                let (sink, stream) = split_frame(ts);
                                let read = stream.for_each(move |cb| {
                                    
                                    // counters 
                                    let temp_outbound_2 = temp_outbound_1.clone();
                                    let config_outbound_4 = config_outbound_3.clone();

                                    if let Some(node) = get_node(&hash, &nodes_outbound_4) {
                                        handle(hash.clone(), cb, config_outbound_4, temp_outbound_2, nodes_outbound_5.clone());
                                    }
                                    Ok(())
                                }).map_err(move|err|{error!(target: "p2p", "read: {:?}", err)});
                                executor_outbound_1.spawn(read.then(|_|{ Ok(()) }));
                                let write = sink.send_all(
                                    rx.map_err(|()| io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")),
                                );
                                executor_outbound_2.spawn(write.then(|_| { Ok(()) }));
                                
                                // send handshake request  
                                let config_outbound_4 = config_outbound_2.clone();                
                                executor_outbound_3.spawn(lazy(move||{
                                    let net_id = config_outbound_4.net_id.clone();
                                    let (id, binding) = config_outbound_4.get_id_and_binding();
                                    let (ip, port) = config_outbound_4.get_ip_and_port();
                                    handshake::send(hash.clone(), id, net_id, ip, port, nodes_outbound_6);
                                    Ok(())
                                }));

                            }).map_err(move|err|{error!(target: "p2p", "connect node: {:?}", err)})
                        }));
                    }
                }
                Ok(())
            }).map_err(|err| error!(target: "p2p", "executor outbound: {:?}", err))
        );

        // active nodes
        let executor_active_nodes = executor.clone();
        let nodes_active_nodes = nodes.clone();
        executor_active_nodes.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(INTERVAL_ACTIVE_NODES)
            ).for_each(move|_|{
                let nodes_active_nodes_0 = nodes_active_nodes.clone();
                active_nodes::send(nodes_active_nodes_0);
                Ok(())
            }).map_err(|err| error!(target: "p2p", "executor active nodes: {:?}", err))
        );

        // inbound    
        let config_inbound = config.clone();
        let nodes_inbound_0 = nodes.clone();
        let listener = TcpListener::bind(&binding).expect("binding failed");        
        let server = listener
            .incoming()
            .for_each(move |ts: TcpStream| {

                // TODO: black list check
                if get_active_nodes_len(&nodes) >= config_inbound.max_peers {
                    debug!(target:"p2p", "max peers !!!");
                    return Ok(());
                }
                
                // config stream
                config_stream(&ts);

                // construct node instance and store it 
                let (tx, rx) = mpsc::channel(409600);
                let addr = ts.peer_addr().unwrap();
                let mut node = Node::new_inbound(addr, tx, false);
                if let Ok(mut write) = nodes_inbound_0.try_write() {
                    if let None = write.insert(node.get_hash(), node) {
                        error!(target: "p2p", "inbound add node failed");
                    }
                }    
                
                // binding io futures
                // let mut value = node.addr.get_ip();
                // let hash = calculate_hash(&value);
                // let hash_0 = hash.clone();
                // node.hash = hash;
                // trace!(target: "p2p", "inbound: {}", &node.addr.get_ip());

                // let (sink, stream) = split_frame(socket);
                // // inbound stream
                // let nodes_0 = nodes.clone();
                // let read = stream.for_each(move |msg| {
                //     //&self.handle(hash_0, msg);
                //     // if let Some(mut peer_node) = get_node(&hash, &nodes_0) {
                //     //     match VERSION::from(msg.head.ver) {
                //     //         VERSION::V0 => {
                //     //             match MODULE::from(msg.head.ctrl) {
                //     //                 MODULE::P2P => {
                //     //                     match ACTION::from(msg.head.action) {
                //     //                         ACTION::HANDSHAKEREQ => {
                //     //                             handshake::receive_req(&self, &mut node, msg);
                //     //                         }
                //     //                         ACTION::HANDSHAKERES => {
                //     //                             handshake::receive_res(&mut node, msg);
                //     //                         }
                //     //                         ACTION::ACTIVENODESREQ => {
                //     //                             active_nodes::receive_req(&self, &mut node);
                //     //                         }
                //     //                         ACTION::ACTIVENODESRES => {
                //     //                             active_nodes::receive_res(&self, &mut node, msg);
                //     //                         }
                //     //                         _ => {
                //     //                             error!(target: "p2p", "Invalid action {} received.", msg.head.action);
                //     //                         }
                //     //                     };
                //     //                 }
                //     //                 MODULE::EXTERNAL => {
                //     //                     trace!(target: "p2p", "external module message received, ctrl {}, act {}", msg.head.ctrl, msg.head.action);
                //     //                     // match HANDLERS.try_get() {
                //     //                     //     Some(handler) => {
                //     //                     //         handler.handle(node, req);
                //     //                     //     },
                //     //                     //     None => {}
                //     //                     // }
                //     //                 }
                //     //             }
                //     //         }
                //     //         VERSION::V1 => {
                //     //             handshake::send(&self, &mut node);
                //     //         }
                //     //         _ => {
                //     //             error!(target: "p2p", "invalid version code");
                //     //         }
                //     //     };
                //     //     //node_hash = calculate_hash(&peer_node.get_node_id());
                //     // }
                //     Ok(())
                // });
                // executor_0_0.spawn(read.then(|_| 
                //     Ok(())
                // ));

                // // outbound stream
                // let write =
                //     sink.send_all(rx.map_err(|()| {
                //         io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")
                //     }));
                // executor_0_0.spawn(write.then(|_| {
                //     Ok(())
                // }));

                Ok(())
            }).map_err(|err| error!(target: "p2p", "executor server: {:?}", err));
        let executor_inbound = executor.clone();
        executor_inbound.spawn(server);
    }

    // shutdown p2p instance
    pub fn shutdown(self) {
        match self.runtime.shutdown_now().wait(){
            Ok(_) => {
                info!(target: "p2p", "shutdown");
            },
            Err(err) => {
                error!(target: "p2p", "shutdown failed: {:?}", err);
            }
        }
    }
}

// messages with module code other than p2p module
// should flow into external handlers
fn handle(
    hash: u64, 
    cb: ChannelBuffer,
    config: Arc<Config>,
    temp: Arc<Mutex<VecDeque<TempNode>>>,
    nodes: Arc<RwLock<HashMap<u64, Node>>>
) {
    trace!(target: "p2p", "handle: {} {}/{}/{}", &hash, cb.head.ctrl, cb.head.ctrl, cb.head.action);
    match VERSION::from(cb.head.ver) {
        VERSION::V0 => {
            match MODULE::from(cb.head.ctrl) {
                MODULE::P2P => {
                    match ACTION::from(cb.head.action) {
                        ACTION::HANDSHAKEREQ => handshake::receive_req(hash, cb, config, nodes),
                        ACTION::HANDSHAKERES => handshake::receive_res(hash, cb, nodes),
                        ACTION::ACTIVENODESREQ => active_nodes::receive_req(hash, nodes),
                        ACTION::ACTIVENODESRES => active_nodes::receive_res(hash, cb, temp, nodes),
                        _ => error!(target: "p2p", "invalid action {}", cb.head.action)
                    };
                }
                MODULE::EXTERNAL => {                      
                    // match HANDLERS.try_get() {
                    //     Some(handler) => {
                    //         handler.handle(node, req);
                    //     },
                    //     None => {}
                    // }
                }
            }
        },
        //VERSION::V1 => handshake::send(p2p, hash),
        _ => error!(target: "p2p", "invalid version code")
    };
}

pub fn send(
    hash: &u64, 
    cb: ChannelBuffer,
    nodes: Arc<RwLock<HashMap<u64, Node>>>
){
    if let Ok(lock) = nodes.try_read() {
        if let Some(ref node) = lock.get(hash) {
            let mut tx = node.tx.clone();
            match tx.try_send(cb) {
                Ok(_) => trace!(target: "p2p", "send to {}", node.addr.get_ip()),
                Err(err) => error!(target: "p2p", "send to {}: {:?}", node.addr.get_ip(), err)
            }
        }
    } 
}

fn config_stream(stream: &TcpStream){
    stream
        .set_recv_buffer_size(1 << 24)
        .expect("set_recv_buffer_size failed");
    stream
        .set_keepalive(Some(Duration::from_secs(30)))
        .expect("set_keepalive failed");
}

fn get_active_nodes(nodes: &Arc<RwLock<HashMap<u64, Node>>>) -> Vec<Node> {
    let mut active_nodes: Vec<Node> = Vec::new();
    if let Ok(read) = nodes.read() {
        for node in read.values() {
            if node.state == STATE::ACTIVE {
                active_nodes.push(node.clone())
            }
        }
    }
    active_nodes
}

fn get_active_nodes_len(nodes: &Arc<RwLock<HashMap<u64, Node>>>) -> u32 {
    let mut len: u32 = 0;
    if let Ok(read) = nodes.read() {
        for node in read.values() {
            if node.state == STATE::ACTIVE {
                len += 1;
            }
        }
    }
    len
}

fn get_node(hash: &u64, nodes: &Arc<RwLock<HashMap<u64, Node>>>) -> Option<Node>{
    if let Ok(read) = nodes.read() {
        if let Some(node) = read.get(hash){
            return Some(node.clone());
        } 
    } 
    None
}

fn remove(_hash: u64, nodes: &mut Arc<RwLock<HashMap<u64, Node>>>){
    match nodes.try_write() {
        Ok(_write) => {
            // TODO
        },
        Err(err) => {
            error!(target: "p2p", "remove: {}", err);
        }
    }
}

pub fn split_frame(socket: TcpStream) -> (stream::SplitSink<Framed<TcpStream, Codec>>, stream::SplitStream<Framed<TcpStream, Codec>>) {
    Codec.framed(socket).split()
}