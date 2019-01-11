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

/// Type of EVM to use.
#[derive(Debug, PartialEq, Clone)]
pub enum VMType {
    /// Aion FastVM
    FastVM,

    /// Aion Virtual Machine
    AVM,
}

impl fmt::Display for VMType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                VMType::FastVM => "FastVM",
                VMType::AVM => "AVM",
            }
        )
    }
}

impl Default for VMType {
    fn default() -> Self { VMType::FastVM }
}
