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

use p2p::*;

use net::event::HANDSHAKE_DONE;

pub const STATUS_GOT: u32 = 1 << 4;
pub const HEADERS_DOWNLOADED: u32 = 1 << 5;
pub const BODIES_DOWNLOADED: u32 = 1 << 6;

pub enum SyncEvent {
    OnStatusReq,
    OnStatusRes,
    OnBlockHeadersRes,
    OnBlockBodiesReq,
    OnBlockBodiesRes,
}

impl SyncEvent {
    pub fn update_node_state(node: &mut Node, event: SyncEvent) {
        let state_code = node.state_code;

        match event {
            SyncEvent::OnStatusReq | SyncEvent::OnStatusRes => {
                if state_code & HANDSHAKE_DONE == HANDSHAKE_DONE {
                    node.state_code = state_code | STATUS_GOT;
                } else {
                    warn!(target: "sync", "Invalid status. State code: {:032b}, Event Id: {}, node id: {}", state_code, event, node.get_node_id());
                }
            }
            SyncEvent::OnBlockHeadersRes => {
                if state_code & STATUS_GOT == STATUS_GOT {
                    node.state_code = state_code | HEADERS_DOWNLOADED;
                } else {
                    warn!(target: "sync", "Invalid status. State code: {:032b}, Event Id: {}, node id: {}", state_code, event, node.get_node_id());
                }
            }
            SyncEvent::OnBlockBodiesReq | SyncEvent::OnBlockBodiesRes => {
                if state_code & HEADERS_DOWNLOADED == HEADERS_DOWNLOADED {
                    node.state_code = (state_code | BODIES_DOWNLOADED) ^ HEADERS_DOWNLOADED;
                } else {
                    // TBD
                }
            }
        }
    }
}

impl fmt::Display for SyncEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            SyncEvent::OnStatusReq => "OnStatusReq",
            SyncEvent::OnStatusRes => "OnStatusRes",
            SyncEvent::OnBlockHeadersRes => "OnBlockHeadersRes",
            SyncEvent::OnBlockBodiesReq => "OnBlockBodiesReq",
            SyncEvent::OnBlockBodiesRes => "OnBlockBodiesRes",
        };
        write!(f, "{}", printable)
    }
}

#[test]
fn display_event_test() {
    println!("SyncEvent: {}", SyncEvent::OnStatusReq);
    println!("SyncEvent: {}", SyncEvent::OnStatusRes);
    println!("SyncEvent: {}", SyncEvent::OnBlockHeadersRes);
    println!("SyncEvent: {}", SyncEvent::OnBlockBodiesReq);
    println!("SyncEvent: {}", SyncEvent::OnBlockBodiesRes);
}
