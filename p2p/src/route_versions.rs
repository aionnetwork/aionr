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

/// p2p routing version code, u16
///
/// routing version defined here only for p2p layer
#[derive(Debug, PartialEq)]
pub enum VERSION {
    V0,
    V1,
    V2,
}

impl VERSION {
    pub fn value(&self) -> u16 {
        match self {
            VERSION::V0 => 0u16,
            VERSION::V1 => 1u16,
            VERSION::V2 => 1u16,
        }
    }

    pub fn from(value: u16) -> VERSION {
        match value {
            0 => VERSION::V0,
            1 => VERSION::V1,
            2 => VERSION::V2,
            _ => VERSION::V2,
        }
    }
}

#[cfg(test)]
mod tests{

    use route_versions::VERSION;

    #[test]
    pub fn equal(){
        assert_eq!(VERSION::V0, VERSION::V0);
        assert_eq!(VERSION::V1, VERSION::V1);
        assert_eq!(VERSION::V2, VERSION::V2);
    }

    #[test]
    pub fn value(){
        assert_eq!(VERSION::V0.value(), 0);
        assert_eq!(VERSION::V1.value(), 1);
        assert_eq!(VERSION::V2.value(), 2);
    }

    #[test]
    pub fn from(){
        assert_eq!(VERSION::V0, VERSION::from(0));
        assert_eq!(VERSION::V1, VERSION::from(1));
        assert_eq!(VERSION::V2, VERSION::from(2));
        assert_eq!(VERSION::V2, VERSION::from(255));
    }
}