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

/// p2p routing module code, u8
///
/// routing modules defined here only for p2p layer which is 0u8
#[derive(Debug, PartialEq)]
pub enum MODULE {
    P2P,
}

impl MODULE {
    pub fn value(&self) -> u8 {
        match self {
            MODULE::P2P => 0u8,
        }
    }
}

#[cfg(test)]
mod tests {

    use route_modules::MODULE;

    #[test]
    fn equal() {
        assert_eq!(MODULE::P2P, MODULE::P2P);
    }

    #[test]
    fn value() {
        assert_eq!(MODULE::P2P.value(), 0);
    }

    #[test]
    fn from() {
        assert_eq!(MODULE::P2P, MODULE::from(0));
        assert_eq!(MODULE::P2P, MODULE::from(1));
        assert_eq!(MODULE::P2P, MODULE::from(255));
    }
}