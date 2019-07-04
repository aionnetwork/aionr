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

/// p2p routing action code, u8
///
/// routing actions defined here only for p2p layer
#[derive(Debug, PartialEq)]
pub enum ACTION {
    DISCONNECT,
    HANDSHAKEREQ,
    HANDSHAKERES,
    PING,
    PONG,
    ACTIVENODESREQ,
    ACTIVENODESRES,
    CONNECT,
    UNKNOWN,
}

impl ACTION {
    pub fn value(&self) -> u8 {
        match self {
            ACTION::DISCONNECT => 0u8,
            ACTION::HANDSHAKEREQ => 1u8,
            ACTION::HANDSHAKERES => 2u8,
            ACTION::PING => 3u8,
            ACTION::PONG => 4u8,
            ACTION::ACTIVENODESREQ => 5u8,
            ACTION::ACTIVENODESRES => 6u8,
            ACTION::CONNECT => 7u8,
            ACTION::UNKNOWN => 255u8,
        }
    }
    pub fn from(value: u8) -> ACTION {
        match value {
            0 => ACTION::DISCONNECT,
            1 => ACTION::HANDSHAKEREQ,
            2 => ACTION::HANDSHAKERES,
            3 => ACTION::PING,
            4 => ACTION::PONG,
            5 => ACTION::ACTIVENODESREQ,
            6 => ACTION::ACTIVENODESRES,
            7 => ACTION::CONNECT,
            _ => ACTION::UNKNOWN,
        }
    }
}

#[cfg(test)]
mod tests {

    use route_actions::ACTION;

    #[test]
    fn equal() {
        assert_eq!(ACTION::DISCONNECT, ACTION::DISCONNECT);
        assert_eq!(ACTION::HANDSHAKEREQ, ACTION::HANDSHAKEREQ);
        assert_eq!(ACTION::HANDSHAKERES, ACTION::HANDSHAKERES);
        assert_eq!(ACTION::PING, ACTION::PING);
        assert_eq!(ACTION::PONG, ACTION::PONG);
        assert_eq!(ACTION::ACTIVENODESREQ, ACTION::ACTIVENODESREQ);
        assert_eq!(ACTION::ACTIVENODESRES, ACTION::ACTIVENODESRES);
        assert_eq!(ACTION::CONNECT, ACTION::CONNECT);
        assert_eq!(ACTION::UNKNOWN, ACTION::UNKNOWN);
    }

    #[test]
    fn value() {
        assert_eq!(ACTION::DISCONNECT.value(), 0);
        assert_eq!(ACTION::HANDSHAKEREQ.value(), 1);
        assert_eq!(ACTION::HANDSHAKERES.value(), 2);
        assert_eq!(ACTION::PING.value(), 3);
        assert_eq!(ACTION::PONG.value(), 4);
        assert_eq!(ACTION::ACTIVENODESREQ.value(), 5);
        assert_eq!(ACTION::ACTIVENODESRES.value(), 6);
        assert_eq!(ACTION::CONNECT.value(), 7);
        assert_eq!(ACTION::UNKNOWN.value(), 255);
    }

    #[test]
    fn from() {
        assert_eq!(ACTION::DISCONNECT, ACTION::from(0));
        assert_eq!(ACTION::HANDSHAKEREQ, ACTION::from(1));
        assert_eq!(ACTION::HANDSHAKERES, ACTION::from(2));
        assert_eq!(ACTION::PING, ACTION::from(3));
        assert_eq!(ACTION::PONG, ACTION::from(4));
        assert_eq!(ACTION::ACTIVENODESREQ, ACTION::from(5));
        assert_eq!(ACTION::ACTIVENODESRES, ACTION::from(6));
        assert_eq!(ACTION::CONNECT, ACTION::from(7));
        assert_eq!(ACTION::UNKNOWN, ACTION::from(8));
        assert_eq!(ACTION::UNKNOWN, ACTION::from(255));
    }
}