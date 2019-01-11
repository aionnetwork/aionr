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

#[derive(Serialize, Deserialize, PartialEq)]
pub enum NetAction {
    DISCONNECT = 0,
    HANDSHAKEREQ = 1,
    HANDSHAKERES = 2,
    PING = 3,
    PONG = 4,
    ACTIVENODESREQ = 5,
    ACTIVENODESRES = 6,
    CONNECT = 7,
    UNKNOWN = 0xFF,
}

impl NetAction {
    pub fn value(&self) -> u8 {
        match *self {
            NetAction::DISCONNECT => 0 as u8,
            NetAction::HANDSHAKEREQ => 1 as u8,
            NetAction::HANDSHAKERES => 2 as u8,
            NetAction::PING => 3 as u8,
            NetAction::PONG => 4 as u8,
            NetAction::ACTIVENODESREQ => 5 as u8,
            NetAction::ACTIVENODESRES => 6 as u8,
            NetAction::CONNECT => 7 as u8,
            NetAction::UNKNOWN => 0xFF as u8,
        }
    }

    pub fn from(value: u8) -> NetAction {
        match value {
            0 => NetAction::DISCONNECT,
            1 => NetAction::HANDSHAKEREQ,
            2 => NetAction::HANDSHAKERES,
            3 => NetAction::PING,
            4 => NetAction::PONG,
            5 => NetAction::ACTIVENODESREQ,
            6 => NetAction::ACTIVENODESRES,
            7 => NetAction::CONNECT,
            _ => NetAction::UNKNOWN,
        }
    }
}

impl fmt::Display for NetAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            NetAction::DISCONNECT => "DISCONNECT",
            NetAction::HANDSHAKEREQ => "HANDSHAKEREQ",
            NetAction::HANDSHAKERES => "HANDSHAKERES",
            NetAction::PING => "PING",
            NetAction::PONG => "PONG",
            NetAction::ACTIVENODESREQ => "ACTIVENODESREQ",
            NetAction::ACTIVENODESRES => "ACTIVENODESRES",
            NetAction::CONNECT => "CONNECT",
            NetAction::UNKNOWN => "UNKNOWN",
        };
        write!(f, "{}", printable)
    }
}
