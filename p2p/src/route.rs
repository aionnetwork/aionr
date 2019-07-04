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
pub fn from(version: u16, module: u8, action: u8) -> [u8; 4]{
    let mut bytes: [u8; 4] = [0x00; 4];
    bytes[0] = (version >> 8) as u8;
    bytes[1] = version as u8;
    bytes[2] = module;
    bytes[3] = action;
    bytes
}

#[cfg(test)]
mod tests {

    use route_versions::VERSION;
    use route_modules::MODULE;
    use route_actions::ACTION;
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
}