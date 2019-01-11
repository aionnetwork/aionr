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

use p2p::{ALIVE, Node};

pub const HANDSHAKE_DONE: u32 = 1 << 2;

pub enum NetEvent {
    OnHandshakeReq,
    OnHandshakeRes,
    OnActiveNodesReq,
    OnActiveNodesRes,
    OnPing,
    OnPong,
}

impl NetEvent {
    pub fn update_node_state(node: &mut Node, event: NetEvent) {
        let state_code = node.state_code;

        match event {
            NetEvent::OnHandshakeReq | NetEvent::OnHandshakeRes => {
                node.state_code = state_code | HANDSHAKE_DONE | ALIVE;
            }
            NetEvent::OnActiveNodesReq | NetEvent::OnActiveNodesRes => {
                if state_code & HANDSHAKE_DONE == HANDSHAKE_DONE {
                    node.state_code = state_code | ALIVE;
                } else {
                    warn!(target: "net", "Invalid status. State code: {:032b}, Event Id: {}, node id: {}", state_code, event, node.get_node_id());
                }
            }
            NetEvent::OnPing | NetEvent::OnPong => {
                if state_code & HANDSHAKE_DONE == HANDSHAKE_DONE {

                } else {
                    warn!(target: "net", "Invalid status. State code: {:032b}, Event Id: {}, node id: {}", state_code, event, node.get_node_id());
                }
            }
        }
    }
}

impl fmt::Display for NetEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            NetEvent::OnHandshakeReq => "HandshakeReq",
            NetEvent::OnHandshakeRes => "HandshakeRes",
            NetEvent::OnActiveNodesReq => "ActiveNodesReq",
            NetEvent::OnActiveNodesRes => "ActiveNodesRes",
            NetEvent::OnPing => "Ping",
            NetEvent::OnPong => "Pong",
        };
        write!(f, "{}", printable)
    }
}

#[test]
fn display_event_test() {
    println!("NetEvent: {}", NetEvent::OnHandshakeReq);
    println!("NetEvent: {}", NetEvent::OnHandshakeRes);
    println!("NetEvent: {}", NetEvent::OnActiveNodesReq);
    println!("NetEvent: {}", NetEvent::OnActiveNodesRes);
    println!("NetEvent: {}", NetEvent::OnPing);
    println!("NetEvent: {}", NetEvent::OnPong);
}
