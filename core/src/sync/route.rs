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

/// sync related messages routing info defined here

#[derive(Debug, PartialEq)]
pub enum VERSION {
    V0,
    V1,
}

impl VERSION {
    pub fn value(&self) -> u16 {
        match self {
            VERSION::V0 => 0u16,
            VERSION::V1 => 1u16,
        }
    }

    pub fn from(value: u16) -> VERSION {
        match value {
            0 => VERSION::V0,
            1 => VERSION::V1,
            _ => VERSION::V1,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum MODULE {
    SYNC,
}

impl MODULE {
    pub fn value(&self) -> u8 {
        match self {
            MODULE::SYNC => 1u8,
        }
    }
}

#[derive(PartialEq)]
pub enum ACTION {
    STATUSREQ = 0,
    STATUSRES = 1,
    HEADERSREQ = 2,
    HEADERSRES = 3,
    BODIESREQ = 4,
    BODIESRES = 5,
    BROADCASTTX = 6,
    BROADCASTBLOCK = 7,
    UNKNOWN = 0xFF,
}

impl ACTION {
    pub fn value(&self) -> u8 {
        match *self {
            ACTION::STATUSREQ => 0 as u8,
            ACTION::STATUSRES => 1 as u8,
            ACTION::HEADERSREQ => 2 as u8,
            ACTION::HEADERSRES => 3 as u8,
            ACTION::BODIESREQ => 4 as u8,
            ACTION::BODIESRES => 5 as u8,
            ACTION::BROADCASTTX => 6 as u8,
            ACTION::BROADCASTBLOCK => 7 as u8,
            ACTION::UNKNOWN => 0xFF as u8,
        }
    }

    pub fn from(value: u8) -> ACTION {
        match value {
            0 => ACTION::STATUSREQ,
            1 => ACTION::STATUSRES,
            2 => ACTION::HEADERSREQ,
            3 => ACTION::HEADERSRES,
            4 => ACTION::BODIESREQ,
            5 => ACTION::BODIESRES,
            6 => ACTION::BROADCASTTX,
            7 => ACTION::BROADCASTBLOCK,
            _ => ACTION::UNKNOWN,
        }
    }
}
