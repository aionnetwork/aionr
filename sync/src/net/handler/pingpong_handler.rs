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

use bytes::BufMut;

use super::super::action::NetAction;
use super::super::event::NetEvent;
use p2p::*;

pub struct PingPongHandler;

impl PingPongHandler {
    pub fn handle_ping(node: &mut Node, _req: ChannelBuffer) {
        trace!(target: "net", "PING received.");

        let mut res = ChannelBuffer::new();
        let node_hash = node.node_hash;

        res.head.set_version(Version::V0);
        res.head.set_control(Control::NET);
        res.head.action = NetAction::PONG.value();
        res.body
            .put_slice("POWEquihash pong".to_string().as_bytes());
        res.head.set_length(res.body.len() as u32);
        NetEvent::update_node_state(node, NetEvent::OnPing);
        P2pMgr::update_node(node_hash, node);
        P2pMgr::send(node_hash, res);
    }

    pub fn handle_pong(node: &mut Node, _req: ChannelBuffer) {
        trace!(target: "net", "PONG received.");

        let node_hash = node.node_hash;

        NetEvent::update_node_state(node, NetEvent::OnPong);
        P2pMgr::update_node(node_hash, node);
    }
}
