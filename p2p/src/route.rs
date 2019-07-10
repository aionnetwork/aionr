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

/// p2p route util methods
///
/// there is is no restriction to upper layer modules they could chose same version code as
/// p2p layer since handlers would be grouped into one route with same route code
//pub fn from(version: u16, module: u8, action: u8) -> [u8; 4]{
//    let mut bytes: [u8; 4] = [0x00; 4];
//    bytes[0] = (version >> 8) as u8;
//    bytes[1] = version as u8;
//    bytes[2] = module;
//    bytes[3] = action;
//    bytes
//}

/// p2p routing version code, u16
///
/// routing version defined here only for p2p layer
#[derive(Debug, PartialEq)]
pub enum VERSION {
    V0,
    V1,
    V2,
}

impl VERSION {
    pub fn value(&self) -> u16 {
        match self {
            VERSION::V0 => 0u16,
            VERSION::V1 => 1u16,
            VERSION::V2 => 2u16,
        }
    }

    pub fn from(value: u16) -> VERSION {
        match value {
            0 => VERSION::V0,
            1 => VERSION::V1,
            2 => VERSION::V2,
            _ => VERSION::V2,
        }
    }
}

/// p2p routing module code, u8
///
/// routing modules defined here only for p2p layer which is 0u8
#[derive(Debug, PartialEq)]
pub enum MODULE {
    P2P,
    EXTERNAL
}

impl MODULE {
    pub fn value(&self) -> u8 {
        match self {
            MODULE::P2P => 0u8,
            MODULE::EXTERNAL => 1u8,
        }
    }
    pub fn from(value: u8) -> MODULE {
        match value {
            0 => MODULE::P2P,
            _ => MODULE::EXTERNAL
        }
    }
}

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

    use route::VERSION;
    use route::MODULE;
    use route::ACTION;
    use route::from;

    #[test]
    fn test_from(){
        assert_eq!(from(VERSION::V0.value(), MODULE::P2P.value(), ACTION::DISCONNECT.value()),     [0x00, 0x00, 0x00, 0x00]);
        assert_eq!(from(VERSION::V0.value(), MODULE::P2P.value(), ACTION::HANDSHAKEREQ.value()),   [0x00, 0x00, 0x00, 0x01]);
        assert_eq!(from(VERSION::V0.value(), MODULE::P2P.value(), ACTION::HANDSHAKERES.value()),   [0x00, 0x00, 0x00, 0x02]);
        assert_eq!(from(VERSION::V0.value(), MODULE::P2P.value(), ACTION::PING.value()),           [0x00, 0x00, 0x00, 0x03]);
        assert_eq!(from(VERSION::V0.value(), MODULE::P2P.value(), ACTION::PONG.value()),           [0x00, 0x00, 0x00, 0x04]);
        assert_eq!(from(VERSION::V0.value(), MODULE::P2P.value(), ACTION::ACTIVENODESREQ.value()), [0x00, 0x00, 0x00, 0x05]);
        assert_eq!(from(VERSION::V0.value(), MODULE::P2P.value(), ACTION::ACTIVENODESRES.value()), [0x00, 0x00, 0x00, 0x06]);
        assert_eq!(from(VERSION::V0.value(), MODULE::P2P.value(), ACTION::CONNECT.value()),        [0x00, 0x00, 0x00, 0x07]);
        assert_eq!(from(VERSION::V0.value(), MODULE::P2P.value(), ACTION::UNKNOWN.value()),        [0x00, 0x00, 0x00, 0xff]);
    }

    #[test]
    fn test_version_equal(){
        assert_eq!(VERSION::V0, VERSION::V0);
        assert_eq!(VERSION::V1, VERSION::V1);
        assert_eq!(VERSION::V2, VERSION::V2);
    }

    #[test]
    fn test_version_value(){
        assert_eq!(VERSION::V0.value(), 0);
        assert_eq!(VERSION::V1.value(), 1);
        assert_eq!(VERSION::V2.value(), 2);
    }

    #[test]
    fn test_version_from(){
        assert_eq!(VERSION::V0, VERSION::from(0));
        assert_eq!(VERSION::V1, VERSION::from(1));
        assert_eq!(VERSION::V2, VERSION::from(2));
        assert_eq!(VERSION::V2, VERSION::from(255));
    }

    #[test]
    fn test_module_equal() {
        assert_eq!(MODULE::P2P, MODULE::P2P);
        assert_eq!(MODULE::EXTERNAL, MODULE::EXTERNAL);
    }

    #[test]
    fn test_module_from() {
        assert_eq!(MODULE::P2P, MODULE::from(0));
        assert_eq!(MODULE::EXTERNAL, MODULE::from(1));
        assert_eq!(MODULE::EXTERNAL, MODULE::from(2));
        assert_eq!(MODULE::EXTERNAL, MODULE::from(255));
    }

    #[test]
    fn test_module_value() {
        assert_eq!(MODULE::P2P.value(), 0);
        assert_eq!(MODULE::EXTERNAL.value(), 1);
    }

    #[test]
    fn test_action_equal() {
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
    fn test_action_value() {
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
    fn test_action_from() {
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