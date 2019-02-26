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
use futures::future::lazy;
use futures::{Future, Stream};
use p2p::*;
use state::Storage;
use std::sync::RwLock;
use std::thread;
use std::time::{Duration, Instant};
use tokio::runtime::{Builder, Runtime};
use tokio::timer::Interval;

mod action;
mod error;
pub mod event;
pub mod handler;

use self::action::NetAction;
// use self::error::*;
use self::handler::active_nodes_handler::ActiveNodesHandler;
use self::handler::default_handler::DefaultHandler;
use self::handler::handshake_handler::HandshakeHandler;
use self::handler::pingpong_handler::PingPongHandler;

lazy_static! {
    static ref DEFAULT_HANDLER: Storage<DefaultHandler> = Storage::new();
    static ref NET_RUNTIME: Storage<RwLock<Runtime>> = Storage::new();
}

const RECONNECT_BOOT_NOEDS_INTERVAL: u64 = 30;
const RECONNECT_NORMAL_NOEDS_INTERVAL: u64 = 30;

#[derive(Clone, Copy)]
pub struct NetManager;

impl NetManager {
    pub fn enable(handler: DefaultHandler) {
        DEFAULT_HANDLER.set(handler);
        NET_RUNTIME.set(RwLock::new(
            Builder::new()
                .core_threads(3)
                .name_prefix("NET-Task")
                .build()
                .expect("Tokio Runtime"),
        ));

        Self::enable_p2p_server();
        Self::enable_clients_for_normal_nodes();
        Self::enable_clients_with_boot_nodes();
    }

    fn enable_p2p_server() {
        thread::sleep(Duration::from_secs(5));
        NET_RUNTIME
            .get()
            .write()
            .expect("Tokio Runtime")
            .spawn(lazy(|| {
                let local_addr = P2pMgr::get_local_node().get_ip_addr();
                P2pMgr::create_server(local_addr, Self::handle);
                Ok(())
            }));
    }

    fn enable_clients_with_boot_nodes() {
        let network_config = P2pMgr::get_network_config();
        let boot_nodes = P2pMgr::load_boot_nodes(network_config.boot_nodes.clone());
        let connect_boot_nodes_task = Interval::new(
            Instant::now(),
            Duration::from_secs(RECONNECT_BOOT_NOEDS_INTERVAL),
        ).for_each(move |_| {
            for boot_node in boot_nodes.iter() {
                let node_hash = P2pMgr::calculate_hash(&boot_node.get_node_id());
                if let Some(node) = P2pMgr::get_node(node_hash) {
                    if node.state_code == DISCONNECTED {
                        trace!(target: "net", "boot node reconnected: {}@{}", boot_node.get_node_id(), boot_node.get_ip_addr());
                        Self::connet_peer(boot_node.clone());
                    }
                } else {
                    trace!(target: "net", "boot node loaded: {}@{}", boot_node.get_node_id(), boot_node.get_ip_addr());
                    Self::connet_peer(boot_node.clone());
                }
            }

            Ok(())
        })
            .map_err(|e| error!("interval errored; err={:?}", e));
        NET_RUNTIME
            .get()
            .write()
            .expect("Tokio Runtime")
            .spawn(connect_boot_nodes_task);
    }

    fn enable_clients_for_normal_nodes() {
        let local_node = P2pMgr::get_local_node();
        let local_node_id_hash = P2pMgr::calculate_hash(&local_node.get_node_id());
        let network_config = P2pMgr::get_network_config();
        let max_peers_num = network_config.max_peers as usize;
        let client_ip_black_list = network_config.ip_black_list.clone();
        let sync_from_boot_nodes_only = network_config.sync_from_boot_nodes_only;

        let connect_normal_nodes_task = Interval::new(
            Instant::now(),
            Duration::from_secs(RECONNECT_NORMAL_NOEDS_INTERVAL),
        ).for_each(move |_| {
            let active_nodes_count = P2pMgr::get_nodes_count(ALIVE);
            if !sync_from_boot_nodes_only && active_nodes_count < max_peers_num {
                if let Some(peer_node) = P2pMgr::get_an_inactive_node() {
                    let peer_node_id_hash = P2pMgr::calculate_hash(&peer_node.get_node_id());
                    if peer_node_id_hash != local_node_id_hash {
                        let peer_ip = peer_node.ip_addr.get_ip();
                        if !client_ip_black_list.contains(&peer_ip) {
                            Self::connet_peer(peer_node);
                        }
                    }
                };
            }

            Ok(())
        })
            .map_err(|e| error!("interval errored; err={:?}", e));
        NET_RUNTIME
            .get()
            .write()
            .expect("Tokio Runtime")
            .spawn(connect_normal_nodes_task);
    }

    fn connet_peer(peer_node: Node) {
        trace!(target: "net", "Try to connect to node {}", peer_node.get_ip_addr());
        let node_hash = P2pMgr::calculate_hash(&peer_node.get_node_id());
        P2pMgr::remove_peer(node_hash);
        P2pMgr::create_client(peer_node, Self::handle);
    }

    fn handle(node: &mut Node, req: ChannelBuffer) {
        match Version::from(req.head.ver) {
            Version::V0 => {
                trace!(target: "net", "Ver 0 package received.");

                match Control::from(req.head.ctrl) {
                    Control::NET => {
                        trace!(target: "net", "P2P NET message received.");

                        match NetAction::from(req.head.action) {
                            NetAction::DISCONNECT => {
                                trace!(target: "net", "DISCONNECT received.");
                            }
                            NetAction::HANDSHAKEREQ => {
                                HandshakeHandler::handle_handshake_req(node, req);
                                ActiveNodesHandler::send_activenodes_req();
                            }
                            NetAction::HANDSHAKERES => {
                                HandshakeHandler::handle_handshake_res(node, req);
                                ActiveNodesHandler::send_activenodes_req();
                            }
                            NetAction::PING => {
                                PingPongHandler::handle_ping(node, req);
                            }
                            NetAction::PONG => {
                                PingPongHandler::handle_pong(node, req);
                            }
                            NetAction::ACTIVENODESREQ => {
                                ActiveNodesHandler::handle_active_nodes_req(node);
                            }
                            NetAction::ACTIVENODESRES => {
                                ActiveNodesHandler::handle_active_nodes_res(node, req);
                            }
                            _ => {
                                error!(target: "net", "Invalid action {} received.", req.head.action);
                            }
                        };
                    }
                    Control::SYNC => {
                        trace!(target: "net", "P2P SYNC message received.");

                        let handler = DEFAULT_HANDLER.get();
                        handler.handle(node, req);
                    }
                    _ => {
                        error!(target: "net", "Invalid message received: {}", req.head);
                    }
                }
            }
            Version::V1 => {
                trace!(target: "net", "Ver 1 package received.");
                HandshakeHandler::send_handshake_req(node);
            }
            _ => {
                error!(target: "net", "Invalid Version.");
            }
        };
    }
}
