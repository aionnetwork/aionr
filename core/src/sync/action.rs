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

#[derive(PartialEq)]
pub enum Action {
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

impl Action {
    pub fn value(&self) -> u8 {
        match *self {
            Action::STATUSREQ => 0 as u8,
            Action::STATUSRES => 1 as u8,
            Action::HEADERSREQ => 2 as u8,
            Action::HEADERSRES => 3 as u8,
            Action::BODIESREQ => 4 as u8,
            Action::BODIESRES => 5 as u8,
            Action::BROADCASTTX => 6 as u8,
            Action::BROADCASTBLOCK => 7 as u8,
            Action::UNKNOWN => 0xFF as u8,
        }
    }

    pub fn from(value: u8) -> Action {
        match value {
            0 => Action::STATUSREQ,
            1 => Action::STATUSRES,
            2 => Action::HEADERSREQ,
            3 => Action::HEADERSRES,
            4 => Action::BODIESREQ,
            5 => Action::BODIESRES,
            6 => Action::BROADCASTTX,
            7 => Action::BROADCASTBLOCK,
            _ => Action::UNKNOWN,
        }
    }
}
