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

#[derive(Clone, Copy)]
pub struct LightSyncManager;

impl LightSyncManager {
    fn handle(node: &mut Node, req: ChannelBuffer) {
        match Version::from(req.head.ver) {
            Version::V0 => {
                trace!(target: "net", "Ver 0 package received.");

                match Control::from(req.head.ctrl) {
                    Control::NET => {
                        trace!(target: "net", "P2P NET message received.");
                    }
                    Control::SYNC => {
                        trace!(target: "net", "P2P SYNC message received.");
                    }
                    Control::LIGHT => {
                        trace!(target: "net", "P2P LIGHT message received.");
                    }
                    _ => {
                        error!(target: "net", "Invalid message received: {}", req.head);
                    }
                }
            }
            Version::V1 => {
                trace!(target: "net", "Ver 1 package received.");
            }
            _ => {
                error!(target: "net", "Invalid Version.");
            }
        };
    }
}
