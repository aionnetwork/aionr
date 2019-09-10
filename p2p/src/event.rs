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
use node::Node;
// use states::STATE::ALIVE;
// use states::STATE::HANDSHAKEDONE;

pub enum Event {
    OnHandshakeReq,
    OnHandshakeRes,
    OnActiveNodesReq,
    OnActiveNodesRes,
}

impl Event {
    pub fn update_node_state(node: &mut Node, event: Event) {
        // let state_code = node.state_code;
        // match event {
        //     Event::OnHandshakeReq | Event::OnHandshakeRes => {
        //         node.state_code = state_code | HANDSHAKEDONE.value() | ALIVE.value();
        //     }
        //     Event::OnActiveNodesReq | Event::OnActiveNodesRes => {
        //         if state_code & HANDSHAKEDONE.value() == HANDSHAKEDONE.value() {
        //             node.state_code = state_code | ALIVE.value();
        //         } else {
        //             warn!(target: "p2p", "Invalid status. State code: {:032b}, Event Id: {}, node id: {}", state_code, event, node.get_node_id());
        //         }
        //     }
        // }
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            Event::OnHandshakeReq => "HandshakeReq",
            Event::OnHandshakeRes => "HandshakeRes",
            Event::OnActiveNodesReq => "ActiveNodesReq",
            Event::OnActiveNodesRes => "ActiveNodesRes",
        };
        write!(f, "{}", printable)
    }
}
