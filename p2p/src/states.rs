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
#[derive(Debug, PartialEq)]
pub enum STATE {
    CONNECTED,
    ISSERVER,
    HANDSHAKEDONE,
    ALIVE,
    DISCONNECTED,
}

impl STATE {
    pub fn value(&self) -> u32 {
        match self {
            STATE::CONNECTED => 1,
            STATE::ISSERVER => 1 << 1,
            STATE::HANDSHAKEDONE => 1 << 2,
            STATE::ALIVE => 1 << 3,
            STATE::DISCONNECTED => 1 << 4,
        }
    }
    pub fn from(value: u32) -> STATE {
        match value {
            0 => STATE::CONNECTED,
            2 => STATE::ISSERVER,
            4 => STATE::HANDSHAKEDONE,
            8 => STATE::ALIVE,
            16 => STATE::DISCONNECTED,
            _ => STATE::CONNECTED,
        }
    }
}

#[cfg(test)]
mod tests {

    use states::STATE;

    #[test]
    fn equal() {
        assert_eq!(STATE::CONNECTED, STATE::CONNECTED);
        assert_eq!(STATE::ACTIVE, STATE::ACTIVE);
    }

    #[test]
    fn value() {
        assert_eq!(STATE::CONNECTED.value(), 0);
        assert_eq!(STATE::ALIVE.value(), 1);
    }

    #[test]
    fn from() {
        assert_eq!(STATE::CONNECTED, STATE::from(0));
        assert_eq!(STATE::ALIVE, STATE::from(1));
        assert_eq!(STATE::CONNECTED, STATE::from(2));
        assert_eq!(STATE::CONNECTED, STATE::from(255));
    }
}
