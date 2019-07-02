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

//#[derive(Serialize, Deserialize, PartialEq)]
#[derive(PartialEq)]
pub enum SyncAction {
    STATUSREQ = 0,
    STATUSRES = 1,
    BLOCKSHEADERSREQ = 2,
    BLOCKSHEADERSRES = 3,
    BLOCKSBODIESREQ = 4,
    BLOCKSBODIESRES = 5,
    BROADCASTTX = 6,
    BROADCASTBLOCK = 7,
    UNKNOWN = 0xFF,
}

impl SyncAction {
    pub fn value(&self) -> u8 {
        match *self {
            SyncAction::STATUSREQ => 0 as u8,
            SyncAction::STATUSRES => 1 as u8,
            SyncAction::BLOCKSHEADERSREQ => 2 as u8,
            SyncAction::BLOCKSHEADERSRES => 3 as u8,
            SyncAction::BLOCKSBODIESREQ => 4 as u8,
            SyncAction::BLOCKSBODIESRES => 5 as u8,
            SyncAction::BROADCASTTX => 6 as u8,
            SyncAction::BROADCASTBLOCK => 7 as u8,
            SyncAction::UNKNOWN => 0xFF as u8,
        }
    }

    pub fn from(value: u8) -> SyncAction {
        match value {
            0 => SyncAction::STATUSREQ,
            1 => SyncAction::STATUSRES,
            2 => SyncAction::BLOCKSHEADERSREQ,
            3 => SyncAction::BLOCKSHEADERSRES,
            4 => SyncAction::BLOCKSBODIESREQ,
            5 => SyncAction::BLOCKSBODIESRES,
            6 => SyncAction::BROADCASTTX,
            7 => SyncAction::BROADCASTBLOCK,
            _ => SyncAction::UNKNOWN,
        }
    }
}

impl fmt::Display for SyncAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            SyncAction::STATUSREQ => "STATUSREQ",
            SyncAction::STATUSRES => "STATUSRES",
            SyncAction::BLOCKSHEADERSREQ => "BLOCKSHEADERSREQ",
            SyncAction::BLOCKSHEADERSRES => "BLOCKSHEADERSRES",
            SyncAction::BLOCKSBODIESREQ => "BLOCKSBODIESREQ",
            SyncAction::BLOCKSBODIESRES => "BLOCKSBODIESRES",
            SyncAction::BROADCASTTX => "BROADCASTTX",
            SyncAction::BROADCASTBLOCK => "BROADCASTBLOCK",
            SyncAction::UNKNOWN => "UNKNOWN",
        };
        write!(f, "{}", printable)
    }
}
