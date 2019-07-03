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

pub const MAX_VALID_ACTTION_VALUE: u8 = 7;

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Version {
    V0 = 0,
    V1 = 1,
    V2 = 2,
    UNKNOWN = 0xFFFF,
}


impl Version {
    pub fn value(&self) -> u16 {
        match *self {
            Version::V0 => 0 as u16,
            Version::V1 => 1 as u16,
            Version::V2 => 2 as u16,
            Version::UNKNOWN => 0xFFFF as u16,
        }
    }

    pub fn from(value: u16) -> Version {
        match value {
            0 => Version::V0,
            1 => Version::V1,
            2 => Version::V2,
            _ => Version::UNKNOWN,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            Version::V0 => "V0",
            Version::V1 => "V1",
            Version::V2 => "V2",
            Version::UNKNOWN => "UNKNOWN",
        };
        write!(f, "{}", printable)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Control {
    NET = 0,
    SYNC = 1,
    UNKNOWN = 0xFF,
}

impl Control {
    pub fn value(&self) -> u8 {
        match *self {
            Control::NET => 0 as u8,
            Control::SYNC => 1 as u8,
            Control::UNKNOWN => 0xFF as u8,
        }
    }

    pub fn from(value: u8) -> Control {
        match value {
            0 => Control::NET,
            1 => Control::SYNC,
            _ => Control::UNKNOWN,
        }
    }
}

impl fmt::Display for Control {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            Control::NET => "NET",
            Control::SYNC => "SYNC",
            Control::UNKNOWN => "UNKNOWN",
        };
        write!(f, "{}", printable)
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Head {
    pub ver: u16,
    pub ctrl: u8,
    pub action: u8,
    pub len: u32,
}

impl Head {
    pub fn new() -> Head {
        Head {
            ver: Version::UNKNOWN.value(),
            ctrl: Control::UNKNOWN.value(),
            action: 0xFF,
            len: 0,
        }
    }

    pub fn set_version(&mut self, ver: Version) { self.ver = ver.value(); }

    pub fn set_control(&mut self, ctrl: Control) { self.ctrl = ctrl.value(); }

    pub fn set_length(&mut self, len: u32) { self.len = len; }
}

impl fmt::Display for Head {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "(Version: {}, Control {}, Action {}, Length {})",
            self.ver, self.ctrl, self.action, self.len
        )
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
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
}

struct Array(Vec<u8>);

impl fmt::Display for Array {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Array(ref vec) = *self;
        for (count, v) in vec.iter().enumerate() {
            if count != 0 {
                try!(write!(f, " "));
            }
            try!(write!(f, "{:02X}", v));
        }
        write!(f, "\n")
    }
}

impl fmt::Display for ChannelBuffer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "(Head: {}, Body {})",
            self.head,
            Array(self.body.to_vec())
        )
    }
}

#[test]
fn display_event_test() {
    let msg = ChannelBuffer::new();
    println!("Message: {}", msg);
}
