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
pub enum Version {
    V0 = 0,
    V1 = 1,
    V2 = 2,
}

impl Version {
    pub fn value(&self) -> u16 {
        match self {
            Version::V0 => 0u16,
            Version::V1 => 1u16,
            Version::V2 => 2u16,
        }
    }

    pub fn from(value: u16) -> Version {
        match value {
            0 => Version::V0,
            1 => Version::V1,
            _ => Version::V2,
        }
    }
}

/// p2p routing module code
///
/// routing modules enum for task distribution
#[derive(Debug, PartialEq)]
pub enum Module {
    /// P2p module
    P2P,
    /// Sync module
    SYNC,
    /// Unknown module
    UNKNOWN,
}

impl Module {
    /// convert to u8, 0u8 for P2P, 1u8 for SYNC
    pub fn value(&self) -> u8 {
        match self {
            Module::P2P => 0u8,
            Module::SYNC => 1u8,
            Module::UNKNOWN => 0xffu8,
        }
    }
    /// convert from u8, 0u8 for P2P, 1u8 for SYNC
    pub fn from(value: u8) -> Module {
        match value {
            0 => Module::P2P,
            1 => Module::SYNC,
            _ => Module::UNKNOWN,
        }
    }
}

/// p2p routing action code
///
/// routing actions defined here only for p2p layer
#[derive(Debug, PartialEq)]
pub enum Action {
    DISCONNECT,
    HANDSHAKEREQ,
    HANDSHAKERES,
    // PING,
    // PONG,
    ACTIVENODESREQ,
    ACTIVENODESRES,
    UNKNOWN,
}

impl Action {
    pub fn value(&self) -> u8 {
        match self {
            Action::DISCONNECT => 0u8,
            Action::HANDSHAKEREQ => 1u8,
            Action::HANDSHAKERES => 2u8,
            // Action::PING => 3u8,
            // Action::PONG => 4u8,
            Action::ACTIVENODESREQ => 5u8,
            Action::ACTIVENODESRES => 6u8,
            Action::UNKNOWN => 255u8,
        }
    }
    pub fn from(value: u8) -> Action {
        match value {
            0 => Action::DISCONNECT,
            1 => Action::HANDSHAKEREQ,
            2 => Action::HANDSHAKERES,
            // 3 => Action::PING,
            // 4 => Action::PONG,
            5 => Action::ACTIVENODESREQ,
            6 => Action::ACTIVENODESRES,
            _ => Action::UNKNOWN,
        }
    }
}

#[cfg(test)]
mod tests {

    use route::Version;
    use route::Module;
    use route::Action;

    /// p2p route util methods
    ///
    /// there is is no restriction to upper layer modules they could chose same version code as
    /// p2p layer since handlers would be grouped into one route with same route code
    pub fn from(version: u16, module: u8, action: u8) -> [u8; 4] {
        let mut bytes: [u8; 4] = [0x00; 4];
        bytes[0] = (version >> 8) as u8;
        bytes[1] = version as u8;
        bytes[2] = module;
        bytes[3] = action;
        bytes
    }

    #[test]
    fn test_from() {
        assert_eq!(
            from(
                Version::V0.value(),
                Module::P2P.value(),
                Action::HANDSHAKEREQ.value()
            ),
            [0x00, 0x00, 0x00, 0x01]
        );
        assert_eq!(
            from(
                Version::V0.value(),
                Module::P2P.value(),
                Action::HANDSHAKERES.value()
            ),
            [0x00, 0x00, 0x00, 0x02]
        );
        assert_eq!(
            from(
                Version::V0.value(),
                Module::P2P.value(),
                Action::ACTIVENODESREQ.value()
            ),
            [0x00, 0x00, 0x00, 0x05]
        );
        assert_eq!(
            from(
                Version::V0.value(),
                Module::P2P.value(),
                Action::ACTIVENODESRES.value()
            ),
            [0x00, 0x00, 0x00, 0x06]
        );
        assert_eq!(
            from(
                Version::V0.value(),
                Module::P2P.value(),
                Action::UNKNOWN.value()
            ),
            [0x00, 0x00, 0x00, 0xff]
        );
    }

    #[test]
    fn test_version_equal() {
        assert_eq!(Version::V0, Version::V0);
        assert_eq!(Version::V1, Version::V1);
        assert_eq!(Version::V2, Version::V2);
    }

    #[test]
    fn test_version_value() {
        assert_eq!(Version::V0.value(), 0);
        assert_eq!(Version::V1.value(), 1);
        assert_eq!(Version::V2.value(), 2);
    }

    #[test]
    fn test_version_from() {
        assert_eq!(Version::V0, Version::from(0));
        assert_eq!(Version::V1, Version::from(1));
        assert_eq!(Version::V2, Version::from(2));
        assert_eq!(Version::V2, Version::from(255));
    }

    #[test]
    fn test_module_equal() {
        assert_eq!(Module::P2P, Module::P2P);
        assert_eq!(Module::SYNC, Module::SYNC);
    }

    #[test]
    fn test_module_from() {
        assert_eq!(Module::P2P, Module::from(0));
        assert_eq!(Module::SYNC, Module::from(1));
        assert_eq!(Module::UNKNOWN, Module::from(2));
        assert_eq!(Module::UNKNOWN, Module::from(255));
    }

    #[test]
    fn test_module_value() {
        assert_eq!(Module::P2P.value(), 0);
        assert_eq!(Module::SYNC.value(), 1);
    }

    #[test]
    fn test_action_equal() {
        assert_eq!(Action::HANDSHAKEREQ, Action::HANDSHAKEREQ);
        assert_eq!(Action::HANDSHAKERES, Action::HANDSHAKERES);
        assert_eq!(Action::ACTIVENODESREQ, Action::ACTIVENODESREQ);
        assert_eq!(Action::ACTIVENODESRES, Action::ACTIVENODESRES);
        assert_eq!(Action::UNKNOWN, Action::UNKNOWN);
    }

    #[test]
    fn test_action_value() {
        assert_eq!(Action::DISCONNECT.value(), 0);
        assert_eq!(Action::HANDSHAKEREQ.value(), 1);
        assert_eq!(Action::HANDSHAKERES.value(), 2);
        assert_eq!(Action::ACTIVENODESREQ.value(), 5);
        assert_eq!(Action::ACTIVENODESRES.value(), 6);
        assert_eq!(Action::UNKNOWN.value(), 255);
    }

    #[test]
    fn test_action_from() {
        assert_eq!(Action::HANDSHAKEREQ, Action::from(1));
        assert_eq!(Action::HANDSHAKERES, Action::from(2));
        assert_eq!(Action::ACTIVENODESREQ, Action::from(5));
        assert_eq!(Action::ACTIVENODESRES, Action::from(6));
        assert_eq!(Action::UNKNOWN, Action::from(8));
        assert_eq!(Action::UNKNOWN, Action::from(255));
    }
}
