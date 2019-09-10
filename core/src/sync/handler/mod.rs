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

pub mod status;
pub mod headers;
pub mod bodies;
pub mod broadcast;
pub mod import;

use p2p::{Module, PROTOCAL_VERSION, ChannelBuffer};

fn channel_buffer_template(action: u8) -> ChannelBuffer {
    ChannelBuffer::new1(PROTOCAL_VERSION, Module::SYNC.value(), action, 0u32)
}

fn channel_buffer_template_with_version(version: u16, action: u8) -> ChannelBuffer {
    ChannelBuffer::new1(version, Module::SYNC.value(), action, 0u32)
}
