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
#[derive(Debug, PartialEq)]
pub enum State {
    HANDSHAKE_SUCCESS,
    HANDSHAKE_FAIL
}

impl State {
    pub fn value(&self) -> u8 {
        match *self {
            State::HANDSHAKE_SUCCESS => 0u8,
            State::HANDSHAKE_FAIL => 1u8,
        }
    }
    pub fn from(value: u8) -> State {
        match value {
            0 => State::HANDSHAKE_SUCCESS,
            _ => State::HANDSHAKE_FAIL,
        }
    }
}

#[cfg(test)]
mod tests {

    use state::STATE;

    #[test]
    fn equal() {
        assert_eq!(State::HANDSHAKE_SUCCESS, State::HANDSHAKE_SUCCESS);
        assert_eq!(State::HANDSHAKE_FALSE, State::HANDSHAKE_FALSE);
    }

    #[test]
    fn value() {
        assert_eq!(State::HANDSHAKE_SUCCESS.value(), 0);
        assert_eq!(State::HANDSHAKE_FAIL.value(), 1);
    }

    #[test]
    fn from() {
        assert_eq!(State::HANDSHAKE_SUCCESS, State::from(0));
        assert_eq!(State::HANDSHAKE_FAIL, State::from(1));
        assert_eq!(State::HANDSHAKE_FAIL, State::from(2));
        assert_eq!(State::HANDSHAKE_FAIL, State::from(255));
    }
}