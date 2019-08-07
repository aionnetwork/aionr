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

use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::Hash;
use std::hash::Hasher;
use std::io;
use std::sync::Arc;
use std::collections::VecDeque;
use std::net::Shutdown;
use std::sync::{Mutex,RwLock};
use std::time::{Duration, Instant};
use rand::{thread_rng, Rng};
use futures::sync::mpsc;
use futures::{Future, Stream};
use futures::lazy;
use tokio::net::{TcpListener,TcpStream};
use tokio::prelude::*;
use tokio::runtime::{Runtime,TaskExecutor};
use tokio::timer::Interval;
use tokio_codec::{Decoder,Framed};
use codec::Codec;
use route::VERSION;
use route::MODULE;
use route::ACTION;
use state::STATE;
use handler::handshake;
use handler::active_nodes;
use node::PROTOCOL_LENGTH;
use node::NODE_ID_LENGTH;
use node::TempNode;
pub use handler::external::Handler;
pub use msg::ChannelBuffer;
pub use node::Node;
pub use config::Config;

const RECONNECT_BOOT_NOEDS_INTERVAL: u64 = 10;
const RECONNECT_NORMAL_NOEDS_INTERVAL: u64 = 1;
const NODE_ACTIVE_REQ_INTERVAL: u64 = 10;
const TEMP_MAX: usize = 64;

pub struct Mgr{
    
    /// threading
    runtime: Runtime,
    /// config
    config: Arc<Config>,
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
            info!(target: "run", "seed: {}", &boot_node_str);
            temp_queue.push_back(TempNode::new_from_str(boot_node_str.to_string()));
        }

        // return instance
        Mgr {
            runtime: Runtime::new().expect("tokio runtime"),
            config: Arc::new(config),
            temp: Arc::new(Mutex::new(temp_queue)),
            nodes: Arc::new(RwLock::new(HashMap::new()))
        }
    }

    // run p2p instance
    pub fn run(&self){
        
        // setup counters
        let executor = Arc::new(self.runtime.executor()); 
        let config = self.config.clone();
        let nodes = self.nodes.clone();
        let temp = self.temp.clone();
        
        // outbound
        let executor_outbound = executor.clone();
        let executor_outbound_0 = executor_outbound.clone();
        let nodes_outbound_0 = nodes.clone(); 
        executor_outbound.spawn(
            Interval::new(
                Instant::now(),
                Duration::from_secs(RECONNECT_BOOT_NOEDS_INTERVAL)
            ).for_each(move|_|{

                // setup counters
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

                    // setup counters
                    let executor_outbound_1 = executor_outbound_0.clone();

                    // process outbound connection
                    let temp_node = temp_node_opt.unwrap();                  
                    executor_outbound_0.spawn(lazy(move||{
                        let nodes_outbound_2 = nodes_outbound_1.clone();
                        let executor_outbound_2 = executor_outbound_1.clone();
                        let executor_outbound_3 = executor_outbound_1.clone();
                            
                        debug!(target: "p2p", "connecting to: {}", &temp_node.addr.to_string());                            
                        if let Ok(addr) = temp_node.addr.to_string().parse() {                      
                            executor_outbound_1.spawn(lazy(move||{
                                debug!(target: "p2p", "fuck !!!!!!!!!!!!!");
                                TcpStream::connect(&addr)                         
                                .map(move |mut stream: TcpStream| {     
                                    debug!(target: "p2p", "connected to: {}", &temp_node.addr.to_string());  
                                    
                                    let hash = calculate_hash(&temp_node.get_id_string());
                                    let nodes_outbound_3 = nodes_outbound_2.clone();
                                    {
                                        if let Ok(read) = nodes_outbound_2.try_read(){
                                            // return at node existing
                                            if let Some(node) = read.get(&hash) {
                                                return;
                                            }
                                        }
                                    }

                                    // config stream
                                    config_stream(&stream);

                                    // construct node instance
                                    let (tx, rx) = mpsc::channel(409600);
                                    let node = Node::new_outbound(tx);
                                    let hash = node.hash.clone();

                                    // binding io futures
                                    let (sink, stream) = split_frame(stream);
                                    let read = stream.for_each(move |msg| {
                                        if let Some(node) = get_node(&hash, &nodes_outbound_3) {
                                            //self_ref_0_0.handle(&mut peer_node, msg);
                                        }
                                        Ok(())
                                    });
                                    executor_outbound_2.spawn(read.then(|_|{ Ok(()) }));
                                    let write = sink.send_all(
                                        rx.map_err(|()| io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")),
                                    );
                                    executor_outbound_3.spawn(write.then(|_| { Ok(()) }));

                                    // send handshake request
                                    // let mut req = ChannelBuffer::new();
                                    // req.head.ver = VERSION::V1.value();

                                })
                                .map_err(move|err|{
                                    error!(target: "p2p", "connect node: {:?}", err)
                                })
                            }))
                        }
                        Ok(())
                    }));   
                }
                Ok(())
            }).map_err(|err| error!("interval errored; err={:?}", err))
        );
        
        // inbound
        let (_, node_str) = &self.config.local_node.split_at(PROTOCOL_LENGTH);
        // TODO: check NODE_ID_LENGTH + 1
        let (id_str, addr_str) = node_str.split_at(NODE_ID_LENGTH + 1);
        if let Ok(addr) = addr_str.parse() {
            let listener = TcpListener::bind(&addr).expect("binding failed");
            
            let server = listener
                .incoming()
                .for_each(move |socket| {

                    // {
                    //     // TODO: black list check
                    //     if get_active_nodes_len(&nodes) >= config.max_peers {
                    //         debug!(target:"p2p", "max peers !!!");
                    //         return Ok(());
                    //     }
                    // }

                    // // config incoming socket
                    // let (tx, rx) = mpsc::channel(409600);
                    // let addr = socket.peer_addr().unwrap();
                    // let mut node = Node::new_inbound(addr, tx);
                  
                    // // TODO: constrats
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
                })
                .map_err(|err| error!(target: "p2p", "socket accept failed: {:?}", err));
            let executor_inbound = executor.clone();
            executor_inbound.spawn(server);
        } else {
            error!(target: "p2p", "binding parse failed: {}", &addr_str);
        }
    }

    // messages with module code other than p2p module
    // should flow into external handlers
    fn handle(&self, hash: u64, cb: ChannelBuffer) {
        match VERSION::from(cb.head.ver) {
            VERSION::V0 => {
                match MODULE::from(cb.head.ctrl) {
                    MODULE::P2P => {
                        match ACTION::from(cb.head.action) {
                            ACTION::HANDSHAKEREQ => {
                                handshake::receive_req(self, hash, cb);
                            }
                            ACTION::HANDSHAKERES => {
                                handshake::receive_res(self, hash, cb);
                            }
                            ACTION::ACTIVENODESREQ => {
                                active_nodes::receive_req(self, hash);
                            }
                            ACTION::ACTIVENODESRES => {
                                active_nodes::receive_res(self, hash, cb);
                            }
                            _ => {
                                error!(target: "p2p", "Invalid action {} received.", cb.head.action);
                            }
                        };
                    }
                    MODULE::EXTERNAL => {
                        trace!(target: "p2p", "external module message received, ctrl {}, act {}", cb.head.ctrl, cb.head.action);
                        // match HANDLERS.try_get() {
                        //     Some(handler) => {
                        //         handler.handle(node, req);
                        //     },
                        //     None => {}
                        // }
                    }
                }
            }
            VERSION::V1 => {
                // handshake::send(self, node);
            }
            _ => {
                error!(target: "p2p", "invalid version code");
            }
        };
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

    pub fn get_active_nodes(&self) -> Vec<Node> {
        get_active_nodes(&self.nodes.clone())
    }

    pub fn get_active_nodes_len(&self) -> u32 {
        get_active_nodes_len(&self.nodes.clone())
    }

    pub fn test(&self){
        println!("test");
    }
}

pub fn send(
    hash: u64, 
    msg: ChannelBuffer,
    nodes: Arc<RwLock<HashMap<u64, Node>>>
){
    if let Ok(lock) = nodes.try_read() {
        if let Some(ref node) = lock.get(&hash) {
            let mut tx = node.tx.clone();
            if let Err(err) = tx.try_send(msg) {
                debug!(target: "p2p", "send: {:?}", err);
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

fn get_node(hash: &u64, nodes: &Arc<RwLock<HashMap<u64, Node>>>) -> Option<Node>{
    if let Ok(read) = nodes.read() {
        if let Some(node) = read.get(hash){
            return Some(node.clone());
        } 
    } 
    None
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

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

pub fn split_frame(
    socket: TcpStream,
) -> (
    stream::SplitSink<Framed<TcpStream, Codec>>,
    stream::SplitStream<Framed<TcpStream, Codec>>,
) {
    Codec.framed(socket).split()
}

// messages with module code other than p2p module
// should flow into external handlers
// fn handle(p2p: Arc<&Mgr>, node: &mut Node, req: ChannelBuffer) {
//     match VERSION::from(req.head.ver) {
//         VERSION::V0 => {
//             match MODULE::from(req.head.ctrl) {
//                 MODULE::P2P => {
//                     match ACTION::from(req.head.action) {
//                         ACTION::HANDSHAKEREQ => {
//                             handshake::receive_req(p2p, node, req);
//                         }
//                         ACTION::HANDSHAKERES => {
//                             handshake::receive_res(node, req);
//                         }
//                         ACTION::ACTIVENODESREQ => {
//                             active_nodes::receive_req(p2p, node);
//                         }
//                         ACTION::ACTIVENODESRES => {
//                             active_nodes::receive_res(p2p, node, req);
//                         }
//                         _ => {
//                             error!(target: "p2p", "Invalid action {} received.", req.head.action);
//                         }
//                     };
//                 }
//                 MODULE::EXTERNAL => {
//                     trace!(target: "p2p", "external module message received, ctrl {}, act {}", req.head.ctrl, req.head.action);
//                     // match HANDLERS.try_get() {
//                     //     Some(handler) => {
//                     //         handler.handle(node, req);
//                     //     },
//                     //     None => {}
//                     // }
//                 }
//             }
//         }
//         VERSION::V1 => {
//             handshake::send(p2p, node);
//         }
//         _ => {
//             error!(target: "p2p", "invalid version code");
//         }
//     };
// }

// pub fn process_inbounds(socket: TcpStream, handle: fn(node: &mut Node, req: ChannelBuffer)) {
//     if let Ok(peer_addr) = socket.peer_addr() {
//         let mut peer_node = Node::new_with_addr(peer_addr);
//         let peer_ip = peer_node.ip_addr.get_ip();
//         let local_ip = get_local_node().ip_addr.get_ip();
//         let config = get_config();
//         if get_nodes_count(ALIVE.value()) < config.max_peers as usize
//             && !config.ip_black_list.contains(&peer_ip){
//             let mut value = peer_node.ip_addr.get_addr();
//             value.push_str(&local_ip);
//             peer_node.node_hash = calculate_hash(&value);
//             peer_node.state_code = CONNECTED.value();
//             trace!(target: "p2p", "New incoming connection: {}", peer_addr);

//             let (tx, rx) = mpsc::channel(409600);
//             let executor = WORKERS.get().executor();

//             peer_node.tx = Some(tx);
//             peer_node.state_code = CONNECTED.value();
//             peer_node.ip_addr.is_server = false;

//             trace!(target: "p2p", "A new peer added: {}", peer_node);

//             let mut node_hash = peer_node.node_hash;
//             add_peer(peer_node, &socket);

//             let (sink, stream) = split_frame(socket);
//             // inbound stream
//             let read = stream.for_each(move |msg| {
//                 if let Some(mut peer_node) = get_node(node_hash) {
//                     handle(&mut peer_node, msg.clone());
//                     node_hash = calculate_hash(&peer_node.get_node_id());
//                 }

//                 Ok(())
//             });
//             executor.spawn(read.then(|_| Ok(())));
//             // outbound stream
//             let write =
//                 sink.send_all(rx.map_err(|()| {
//                     io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")
//                 }));
//             executor.spawn(write.then(move |_| {
//                 trace!(target: "p2p", "Connection with {:?} closed.", peer_ip);
//                 Ok(())
//             }));
//         }
//     } else {
//         error!(target: "p2p", "Invalid socket: {:?}", socket);
//     }
// }



// fn process_outbounds(
//     socket: TcpStream,
//     peer_node: Node,
//     handle: fn(node: &mut Node, req: ChannelBuffer),
// ){
//     let mut peer_node = peer_node.clone();
//     peer_node.node_hash = calculate_hash(&peer_node.get_node_id());
//     let node_hash = peer_node.node_hash;

//     if let Some(node) = get_node(node_hash) {
//         if node.state_code == DISCONNECTED.value() {
//             trace!(target: "p2p", "update known peer node {}@{}...", node.get_node_id(), node.get_ip_addr());
//             remove_peer(node_hash);
//         } else {
//             return;
//         }
//     }

//     let (tx, rx) = mpsc::channel(409600);
//     peer_node.tx = Some(tx);
//     peer_node.state_code = CONNECTED.value() | ISSERVER.value();
//     peer_node.ip_addr.is_server = true;
//     let peer_ip = peer_node.get_ip_addr().clone();
//     trace!(target: "p2p", "A new peer added: {}@{}", peer_node.get_node_id(), peer_node.get_ip_addr());

//     add_peer(peer_node.clone(), &socket);

//     let (sink, stream) = split_frame(socket);
//     let mut req = ChannelBuffer::new();
//     req.head.ver = VERSION::V1.value();
//     handle(&mut peer_node, req);

//     let read = stream.for_each(move |msg| {
//         if let Some(mut peer_node) = get_node(node_hash) {
//             handle(&mut peer_node, msg);
//         }

//         Ok(())
//     });
//     let executor = WORKERS.get().executor();
//     executor.spawn(read.then(|_| Ok(())));

//     let write = sink.send_all(
//         rx.map_err(|()| io::Error::new(io::ErrorKind::Other, "rx shouldn't have an error")),
//     );
//     executor.spawn(write.then(move |_| {
//         trace!(target: "p2p", "connection with {:?} closed.", peer_ip);
//         Ok(())
//     }));
// }

// chris
// pub fn send(node_hash: u64, msg: ChannelBuffer) {
//     match NODES.read() {
//         Ok(nodes) => {
//             match nodes.get(&node_hash) {
//                 Some(ref node) => {
//                     let tx = node.tx.clone();
//                     // tx should be contructed at begin lifecycle of any node in NODES
//                     if tx.is_some() {
//                         match tx.unwrap().try_send(msg) {
//                             Ok(_) => {}
//                             Err(err) => {
//                                 // TODO: dispatch node not found event for upper modules
//                                 remove_peer(node_hash);
//                                 trace!(target: "p2p", "fail sending msg, {}", err);
//                             }
//                         }
//                     }
//                 }
//                 None => {
//                     // TODO: dispatch node not found event for upper modules
//                     remove_peer(node_hash);
//                     trace!(target: "p2p", "peer not found, {}", node_hash);
//                 }
//             }
//         }
//         Err(_err) => {
//             // TODO: dispatch node not found event for upper modules
//         }
//     }
// }

// pub fn register(handler: Handler) { HANDLERS.set(handler); }

// fn connect_peer(peer_node: Node) {
//     trace!(target: "p2p", "Try to connect to node {}", peer_node.get_ip_addr());
//     let node_hash = calculate_hash(&peer_node.get_node_id());
//     remove_peer(node_hash);
//     create_client(peer_node, handle);
// }

// pub fn enable(cfg: Config) {
//     WORKERS.set(Runtime::new().expect("tokio runtime"));
//     SOCKETS.set(Mutex::new(HashMap::new()));
//     let local_node_str = cfg.local_node.clone();
//     let mut local_node = Node::new_with_node_str(local_node_str);
//     local_node.net_id = cfg.net_id;
//     info!(target: "p2p", "        node: {}@{}", local_node.get_node_id(), local_node.get_ip_addr());
//     CONFIG.set(cfg);
//     LOCAL.set(local_node.clone());

//     let executor = WORKERS.get().executor();
//     let local_addr = get_local_node().get_ip_addr();
//     create_server(&executor, &local_addr, handle);
//     let local_node = get_local_node();
//     let local_node_id_hash = calculate_hash(&local_node.get_node_id());
//     let config = get_config();
//     let boot_nodes = load_boot_nodes(config.boot_nodes.clone());
//     let max_peers_num = config.max_peers as usize;
//     let client_ip_black_list = config.ip_black_list.clone();
//     let sync_from_boot_nodes_only = config.sync_from_boot_nodes_only;

//     // task: connect seeds
//     executor.spawn(Interval::new(
//         Instant::now(),
//         Duration::from_secs(RECONNECT_BOOT_NOEDS_INTERVAL),
//     ).for_each(move |_| {
//         for boot_node in boot_nodes.iter() {
//             let node_hash = calculate_hash(&boot_node.get_node_id());
//             if let Some(node) = get_node(node_hash) {
//                 if node.state_code == DISCONNECTED.value() {
//                     trace!(target: "p2p", "boot node reconnected: {}@{}", boot_node.get_node_id(), boot_node.get_ip_addr());
//                     connect_peer(boot_node.clone());
//                 }
//             } else {
//                 trace!(target: "p2p", "boot node loaded: {}@{}", boot_node.get_node_id(), boot_node.get_ip_addr());
//                 connect_peer(boot_node.clone());
//             }
//         }
//         Ok(())
//     }).map_err(|e| error!("interval errored; err={:?}", e)));

//     // task: reconnect
//     executor.spawn(
//         Interval::new(
//             Instant::now(),
//             Duration::from_secs(RECONNECT_NORMAL_NOEDS_INTERVAL),
//         )
//         .for_each(move |_| {
//             let active_nodes_count = get_nodes_count(ALIVE.value());
//             if !sync_from_boot_nodes_only && active_nodes_count < max_peers_num {
//                 if let Some(peer_node) = get_an_inactive_node() {
//                     let peer_node_id_hash = calculate_hash(&peer_node.get_node_id());
//                     if peer_node_id_hash != local_node_id_hash {
//                         let peer_ip = peer_node.ip_addr.get_ip();
//                         if !client_ip_black_list.contains(&peer_ip) {
//                             connect_peer(peer_node);
//                         }
//                     }
//                 };
//             }

//             Ok(())
//         })
//         .map_err(|e| error!("interval errored; err={:?}", e)),
//     );

//     // task: fetch active nodes
//     executor.spawn(
//         Interval::new(
//             Instant::now(),
//             Duration::from_secs(NODE_ACTIVE_REQ_INTERVAL),
//         )
//         .for_each(move |_| {
//             active_nodes::send();
//             Ok(())
//         })
//         .map_err(|e| error!("interval errored; err={:?}", e)),
//     );
// }

// pub fn create_server(
//     executor: &TaskExecutor,
//     local_addr: &String,
//     handle: fn(node: &mut Node, req: ChannelBuffer),
// )
// {
//     if let Ok(addr) = local_addr.parse() {
//         let listener = TcpListener::bind(&addr).expect("Failed to bind");
//         let server = listener
//             .incoming()
//             .map_err(|e| error!(target: "p2p", "Failed to accept socket; error = {:?}", e))
//             .for_each(move |socket| {
//                 socket
//                     .set_recv_buffer_size(1 << 24)
//                     .expect("set_recv_buffer_size failed");
//                 socket
//                     .set_keepalive(Some(Duration::from_secs(30)))
//                     .expect("set_keepalive failed");
//                 process_inbounds(socket, handle);
//                 Ok(())
//             });
//         executor.spawn(server);
//     } else {
//         error!(target: "p2p", "Invalid ip address: {}", local_addr);
//     }
// }

// pub fn create_client(peer_node: Node, handle: fn(node: &mut Node, req: ChannelBuffer)) {
//     let node_ip_addr = peer_node.get_ip_addr();
//     if let Ok(addr) = node_ip_addr.parse() {
//         let executor = WORKERS.get().executor();
//         let node_id = peer_node.get_node_id();
//         executor.spawn(
//             TcpStream::connect(&addr)
//                 .map(move |socket| {
//                     socket
//                         .set_recv_buffer_size(1 << 24)
//                         .expect("set_recv_buffer_size failed");
//                     socket
//                         .set_keepalive(Some(Duration::from_secs(30)))
//                         .expect("set_keepalive failed");
//                     process_outbounds(socket, peer_node, handle);
//                 })
//                 .map_err(
//                     move |e| error!(target: "p2p", "    node: {}@{}, {}", node_ip_addr, node_id, e),
//                 ),
//         );
//     }
// }

// pub fn load_boot_nodes(boot_nodes_str: Vec<String>) -> Vec<Node> {
//     let mut boot_nodes = Vec::new();
//     for boot_node_str in boot_nodes_str {
//         if boot_node_str.len() != 0 {
//             let mut boot_node = Node::new_with_node_str(boot_node_str.to_string());
//             boot_node.is_from_boot_list = true;
//             boot_nodes.push(boot_node);
//         }
//     }
//     boot_nodes
// }

// pub fn get_config() -> &'static Config { CONFIG.get() }

// pub fn get_local_node() -> &'static Node { LOCAL.get() }

// pub fn reset() {
//     if let Ok(mut sockets) = SOCKETS.get().lock() {
//         for (_, socket) in sockets.iter_mut() {
//             if let Err(e) = socket.shutdown() {
//                 error!(target: "p2p", "Invalid socket， {}", e);
//             }
//         }
//     }
//     if let Ok(mut nodes) = NODES.write() {
//         nodes.clear();
//     }
// }

// pub fn get_peer(node_hash: u64) -> Option<TcpStream> {
//     if let Ok(mut socktes_map) = SOCKETS.get().lock() {
//         return socktes_map.remove(&node_hash);
//     }

//     None
// }

// pub fn add_peer(node: Node, socket: &TcpStream) {
//     if let Ok(socket) = socket.try_clone() {
//         if let Ok(mut sockets) = SOCKETS.get().lock() {
//             match sockets.get(&node.node_hash) {
//                 Some(_) => {
//                     warn!(target: "p2p", "Known node, ...");
//                 }
//                 None => {
//                     if let Ok(mut peer_nodes) = NODES.write() {
//                         let max_peers_num = CONFIG.get().max_peers as usize;
//                         if peer_nodes.len() < max_peers_num {
//                             match peer_nodes.get(&node.node_hash) {
//                                 Some(_) => {
//                                     warn!(target: "p2p", "Known node...");
//                                 }
//                                 None => {
//                                     sockets.insert(node.node_hash, socket);
//                                     peer_nodes.insert(node.node_hash, node);
//                                     return;
//                                 }
//                             }
//                         }
//                     }
//                 }
//             }
//         }
//     }

//     if let Err(e) = socket.shutdown(Shutdown::Both) {
//         error!(target: "p2p", "{}", e);
//     }
// }

// pub fn remove_peer(node_hash: u64) -> Option<Node> {
//     if let Ok(mut sockets) = SOCKETS.get().lock() {
//         if let Some(socket) = sockets.remove(&node_hash) {
//             if let Err(e) = socket.shutdown(Shutdown::Both) {
//                 trace!(target: "p2p", "remove_peer， invalid socket， {}", e);
//             }
//         }
//     }
//     if let Ok(mut peer_nodes) = NODES.write() {
//         // if let Some(node) = peer_nodes.remove(&node_hash) {
//         //     info!(target: "p2p", "Node {}@{} removed.", node.get_node_id(), node.get_ip_addr());
//         //     return Some(node);
//         // }
//         // info!(target: "p2p", "remove_peer， peer_node hash: {}", node_hash);
//         return peer_nodes.remove(&node_hash);
//     }

//     None
// }

// pub fn add_node(node: Node) {
//     let max_peers_num = CONFIG.get().max_peers as usize;
//     if let Ok(mut nodes_map) = NODES.write() {
//         if nodes_map.len() < max_peers_num {
//             match nodes_map.get(&node.node_hash) {
//                 Some(_) => {
//                     warn!(target: "p2p", "Known node...");
//                 }
//                 None => {
//                     nodes_map.insert(node.node_hash, node);
//                     return;
//                 }
//             }
//         }
//     }
// }

// pub fn is_connected(node_id_hash: u64) -> bool {
//     let all_nodes = get_all_nodes();
//     for node in all_nodes.iter() {
//         if node_id_hash == calculate_hash(&node.get_node_id()) {
//             return true;
//         }
//     }
//     false
// }

// pub fn get_nodes_count(state_code: u32) -> usize {
//     let mut nodes_count = 0;
//     if let Ok(nodes_map) = NODES.read() {
//         for val in nodes_map.values() {
//             if val.state_code & state_code == state_code {
//                 nodes_count += 1;
//             }
//         }
//     }
//     nodes_count
// }

// pub fn get_nodes_count_with_mode(mode: Mode) -> usize {
//     let mut nodes_count = 0;
//     if let Ok(nodes_map) = NODES.read() {
//         for val in nodes_map.values() {
//             if val.state_code & ALIVE.value() == ALIVE.value() && val.mode == mode {
//                 nodes_count += 1;
//             }
//         }
//     }
//     nodes_count
// }

// pub fn get_nodes_count_all_modes() -> (usize, usize, usize, usize, usize) {
//     let mut normal_nodes_count = 0;
//     let mut backward_nodes_count = 0;
//     let mut forward_nodes_count = 0;
//     let mut lightning_nodes_count = 0;
//     let mut thunder_nodes_count = 0;
//     if let Ok(nodes_map) = NODES.read() {
//         for val in nodes_map.values() {
//             if val.state_code & ALIVE.value() == ALIVE.value() {
//                 match val.mode {
//                     Mode::NORMAL => normal_nodes_count += 1,
//                     Mode::BACKWARD => backward_nodes_count += 1,
//                     Mode::FORWARD => forward_nodes_count += 1,
//                     Mode::LIGHTNING => lightning_nodes_count += 1,
//                     Mode::THUNDER => thunder_nodes_count += 1,
//                 }
//             }
//         }
//     }
//     (
//         normal_nodes_count,
//         backward_nodes_count,
//         forward_nodes_count,
//         lightning_nodes_count,
//         thunder_nodes_count,
//     )
// }

// pub fn get_all_nodes_count() -> u16 {
//     let mut count = 0;
//     if let Ok(nodes_map) = NODES.read() {
//         for _ in nodes_map.values() {
//             count += 1;
//         }
//     }
//     count
// }

// pub fn get_all_nodes() -> Vec<Node> {
//     let mut nodes = Vec::new();
//     if let Ok(nodes_map) = NODES.read() {
//         for val in nodes_map.values() {
//             let node = val.clone();
//             nodes.push(node);
//         }
//     }
//     nodes
// }

// pub fn get_nodes(state_code_mask: u32) -> Vec<Node> {
//     let mut nodes = Vec::new();
//     if let Ok(nodes_map) = NODES.read() {
//         for val in nodes_map.values() {
//             let node = val.clone();
//             if node.state_code & state_code_mask == state_code_mask {
//                 nodes.push(node);
//             }
//         }
//     }
//     nodes
// }

// pub fn get_an_inactive_node() -> Option<Node> {
//     let nodes = get_nodes(DISCONNECTED.value());
//     let mut normal_nodes = Vec::new();
//     for node in nodes.iter() {
//         if node.is_from_boot_list {
//             continue;
//         } else {
//             normal_nodes.push(node);
//         }
//     }
//     let normal_nodes_count = normal_nodes.len();
//     if normal_nodes_count == 0 {
//         return None;
//     }
//     let mut rng = thread_rng();
//     let random_index: usize = rng.gen_range(0, normal_nodes_count);
//     let node = &normal_nodes[random_index];

//     remove_peer(node.node_hash)
// }

// pub fn get_an_active_node() -> Option<Node> {
//     let nodes = get_nodes(ALIVE.value());
//     let node_count = nodes.len();
//     if node_count > 0 {
//         let mut rng = thread_rng();
//         let random_index: usize = rng.gen_range(0, node_count);
//         return get_node(nodes[random_index].node_hash);
//     } else {
//         None
//     }
// }

// pub fn get_node(node_hash: u64) -> Option<Node> {
//     if let Ok(nodes_map) = NODES.read() {
//         if let Some(node) = nodes_map.get(&node_hash) {
//             return Some(node.clone());
//         }
//     }
//     None
// }

// pub fn update_node_with_mode(node_hash: u64, node: &Node) {
//     if let Ok(mut nodes_map) = NODES.write() {
//         if let Some(n) = nodes_map.get_mut(&node_hash) {
//             n.update(node);
//         }
//     }
// }

// pub fn update_node(node_hash: u64, node: &mut Node) {
//     if let Ok(mut nodes_map) = NODES.write() {
//         if let Some(n) = nodes_map.get_mut(&node_hash) {
//             node.mode = n.mode.clone();
//             n.update(node);
//         }
//     }
// }