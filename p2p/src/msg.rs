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

use route::VERSION;
use route::MODULE;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Head {
    pub ver: u16,
    pub ctrl: u8,
    pub action: u8,
    pub len: u32,
}

impl Head {
    pub fn new() -> Head {
        Head {
            ver: VERSION::V2.value(),
            ctrl: MODULE::P2P.value(),
            action: 0xFF,
            len: 0,
        }
    }
    /// temporiy name it for it now
    pub fn new1(ver: u16, ctrl: u8, action: u8, len: u32) -> Head {
        Head {
            ver,
            ctrl,
            action,
            len,
        }
    }

    /// get route
    pub fn get_route(&self) -> u32 {
        return ((self.ver as u32) << 16) + ((self.ctrl as u32) << 8) + (self.action as u32);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelBuffer {
    pub head: Head,
    pub body: Vec<u8>,
}

impl ChannelBuffer {
    pub fn new() -> ChannelBuffer {
        ChannelBuffer {
            head: Head::new(),
            body: Vec::new(),
        }
    }
    // temporiy name it for it now
    pub fn new1(ver: u16, ctrl: u8, action: u8, len: u32) -> ChannelBuffer {
        ChannelBuffer {
            head: Head::new1(ver, ctrl, action, len),
            body: Vec::new(),
        }
    }

    pub fn to_route(ver: u16, ctrl: u8, action: u8) -> u32 {
        ((ver as u32) << 16) + ((ctrl as u32) << 8) + (action as u32)
    }
}

// TODO
#[cfg(test)]
mod tests {

    use msg::Head;
    use route::VERSION;
    use route::MODULE;
    use route::ACTION;

    #[test]
    pub fn test_head() {
        let mut head = Head::new();

        head.ver = VERSION::V0.value();
        head.ctrl = MODULE::P2P.value();

        head.action = ACTION::HANDSHAKEREQ.value();
        assert_eq!(head.get_route(), 1);
        head.action = ACTION::HANDSHAKERES.value();
        assert_eq!(head.get_route(), 2);
        head.action = ACTION::ACTIVENODESREQ.value();
        assert_eq!(head.get_route(), 5);
        head.action = ACTION::ACTIVENODESRES.value();
        assert_eq!(head.get_route(), 6);

        head.ver = VERSION::V1.value();
        head.action = ACTION::HANDSHAKEREQ.value();
        assert_eq!(head.get_route(), 65537);
        head.action = ACTION::HANDSHAKERES.value();
        assert_eq!(head.get_route(), 65538);
        head.action = ACTION::ACTIVENODESREQ.value();
        assert_eq!(head.get_route(), 65541);
        head.action = ACTION::ACTIVENODESRES.value();
        assert_eq!(head.get_route(), 65542);
    }

}
