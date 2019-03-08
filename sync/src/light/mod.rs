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
use super::p2p::*;
pub mod action;
pub mod error;
pub mod event;
pub mod handler;
pub mod kind;

use self::action::LightAction;
use self::handler::on_demand_handler::OnDemandHandler;
use net::event::HANDSHAKE_DONE;

#[derive(Clone, Copy)]
pub struct LightSyncManager;

impl LightSyncManager {
    pub fn handle(node: &mut Node, req: ChannelBuffer) {
        if node.state_code & HANDSHAKE_DONE != HANDSHAKE_DONE {
            return;
        }

        match Version::from(req.head.ver) {
            Version::V0 => {
                trace!(target: "light", "Ver 0 package received.");

                match Control::from(req.head.ctrl) {
                    Control::NET | Control::SYNC => {
                        error!(target: "light", "Unreachable control!");
                    }
                    Control::LIGHT => {
                        trace!(target: "light", "P2P LIGHT message received.");
                        match LightAction::from(req.head.action) {
                            LightAction::ONDEMANDREQ => {
                                OnDemandHandler::handle_on_demand_req(node, req);
                            }
                            LightAction::ONDEMANDRES => {
                                error!(target: "light","Unreachable action.")
                            }
                            _ => {
                                trace!(target: "light", "UNKNOWN received.");
                            }
                        }
                    }
                    _ => {
                        error!(target: "light", "Invalid message received: {}", req.head);
                    }
                }
            }
            Version::V1 => {
                trace!(target: "light", "Ver 1 package received.");
            }
            _ => {
                error!(target: "light", "Invalid Version.");
            }
        };
    }
}
