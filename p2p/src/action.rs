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

/// p2p msg header action code, u8
#[derive(Debug, PartialEq)]
pub enum Action {
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

impl Action {
    pub fn value(&self) -> u8 {
        match *self {
            Action::DISCONNECT => 0u8,
            Action::HANDSHAKEREQ => 1u8,
            Action::HANDSHAKERES => 2u8,
            Action::PING => 3u8,
            Action::PONG => 4u8,
            Action::ACTIVENODESREQ => 5u8,
            Action::ACTIVENODESRES => 6u8,
            Action::CONNECT => 7u8,
            Action::UNKNOWN => 255u8,
        }
    }
    pub fn from(value: u8) -> Action {
        match value {
            0 => Action::DISCONNECT,
            1 => Action::HANDSHAKEREQ,
            2 => Action::HANDSHAKERES,
            3 => Action::PING,
            4 => Action::PONG,
            5 => Action::ACTIVENODESREQ,
            6 => Action::ACTIVENODESRES,
            7 => Action::CONNECT,
            _ => Action::UNKNOWN,
        }
    }
}

#[cfg(test)]
mod tests {

    use action::Action;

    #[test]
    fn equal() {
        assert_eq!(Action::DISCONNECT, Action::DISCONNECT);
        assert_eq!(Action::HANDSHAKEREQ, Action::HANDSHAKEREQ);
        assert_eq!(Action::HANDSHAKERES, Action::HANDSHAKERES);
        assert_eq!(Action::PING, Action::PING);
        assert_eq!(Action::PONG, Action::PONG);
        assert_eq!(Action::ACTIVENODESREQ, Action::ACTIVENODESREQ);
        assert_eq!(Action::ACTIVENODESRES, Action::ACTIVENODESRES);
        assert_eq!(Action::CONNECT, Action::CONNECT);
        assert_eq!(Action::UNKNOWN, Action::UNKNOWN);
    }

    #[test]
    fn value() {
        assert_eq!(Action::DISCONNECT.value(), 0);
        assert_eq!(Action::HANDSHAKEREQ.value(), 1);
        assert_eq!(Action::HANDSHAKERES.value(), 2);
        assert_eq!(Action::PING.value(), 3);
        assert_eq!(Action::PONG.value(), 4);
        assert_eq!(Action::ACTIVENODESREQ.value(), 5);
        assert_eq!(Action::ACTIVENODESRES.value(), 6);
        assert_eq!(Action::CONNECT.value(), 7);
        assert_eq!(Action::UNKNOWN.value(), 255);
    }

    #[test]
    fn from() {
        assert_eq!(Action::DISCONNECT, Action::from(0));
        assert_eq!(Action::HANDSHAKEREQ, Action::from(1));
        assert_eq!(Action::HANDSHAKERES, Action::from(2));
        assert_eq!(Action::PING, Action::from(3));
        assert_eq!(Action::PONG, Action::from(4));
        assert_eq!(Action::ACTIVENODESREQ, Action::from(5));
        assert_eq!(Action::ACTIVENODESRES, Action::from(6));
        assert_eq!(Action::CONNECT, Action::from(7));
        assert_eq!(Action::UNKNOWN, Action::from(255));
    }
}