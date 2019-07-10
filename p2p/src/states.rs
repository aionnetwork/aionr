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

/// p2p node state
/// states defined here only for p2p layer
//#[derive(Debug, PartialEq)]
//pub enum STATE {
//    CONNECTED,
//    ACTIVE,
//}
//
//impl STATE {
//    pub fn value(&self) -> u8 {
//        match self {
//            STATE::CONNECTED => 0u8,
//            STATE::ACTIVE => 1u8,
//        }
//    }
//    pub fn from(value: u8) -> STATE {
//        match value {
//            0 => STATE::CONNECTED,
//            1 => STATE::ACTIVE,
//            _ => STATE::CONNECTED,
//        }
//    }
//}

pub const HANDSHAKE_DONE: u32 = 1 << 2;
pub const DISCONNECTED: u32 = 1 << 10;

#[cfg(test)]
mod tests {

//    use states::STATE;
//
//    #[test]
//    fn equal() {
//        assert_eq!(STATE::CONNECTED, STATE::CONNECTED);
//        assert_eq!(STATE::ACTIVE, STATE::ACTIVE);
//    }
//
//    #[test]
//    fn value() {
//        assert_eq!(STATE::CONNECTED.value(), 0);
//        assert_eq!(STATE::ACTIVE.value(), 1);
//    }
//
//    #[test]
//    fn from() {
//        assert_eq!(STATE::CONNECTED, STATE::from(0));
//        assert_eq!(STATE::ACTIVE, STATE::from(1));
//        assert_eq!(STATE::CONNECTED, STATE::from(2));
//        assert_eq!(STATE::CONNECTED, STATE::from(255));
//    }
}