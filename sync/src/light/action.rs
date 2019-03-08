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

#[derive(Serialize, Deserialize, PartialEq)]
pub enum LightAction {
    ONDEMANDREQ = 0,
    ONDEMANDRES = 1,
    UNKNOWN = 0xFF,
}

impl LightAction {
    pub fn value(&self) -> u8 {
        match *self {
            LightAction::ONDEMANDREQ => 0 as u8,
            LightAction::ONDEMANDRES => 1 as u8,
            LightAction::UNKNOWN => 0xFF as u8,
        }
    }

    pub fn from(value: u8) -> LightAction {
        match value {
            0 => LightAction::ONDEMANDREQ,
            1 => LightAction::ONDEMANDRES,
            _ => LightAction::UNKNOWN,
        }
    }
}

impl fmt::Display for LightAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            LightAction::ONDEMANDREQ => "ONDEMANDREQ",
            LightAction::ONDEMANDRES => "ONDEMANDRES",
            LightAction::UNKNOWN => "UNKNOWN",
        };
        write!(f, "{}", printable)
    }
}
