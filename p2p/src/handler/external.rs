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

/// call back registered for upper layer modules which use p2p module
use ChannelBuffer;
use Node;

// pub type Callback = fn(node: &mut Node, cb: ChannelBuffer);

// #[derive(Clone, Copy)]
// pub struct Handler {
//     pub callback: Callback,
// }

// impl Handler {
//     pub fn set_callback(&mut self, c: Callback) { self.callback = c; }
//     pub fn handle(&self, node: &mut Node, cb: ChannelBuffer) { (self.callback)(node, cb); }
// }