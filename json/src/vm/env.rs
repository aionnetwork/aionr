/*******************************************************************************
 * Copyright (c) 2015-2018 Parity Technologies (UK) Ltd.
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

//! Vm environment.
use hash::Address;
use uint::Uint;

/// Vm environment.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Env {
    /// Address.
    #[serde(rename = "currentCoinbase")]
    pub author: Address,
    /// Difficulty
    #[serde(rename = "currentDifficulty")]
    pub difficulty: Uint,
    /// Gas limit.
    #[serde(rename = "currentGasLimit")]
    pub gas_limit: Uint,
    /// Number.
    #[serde(rename = "currentNumber")]
    pub number: Uint,
    /// Timestamp.
    #[serde(rename = "currentTimestamp")]
    pub timestamp: Uint,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use vm::Env;

    #[test]
    fn env_deserialization() {
        let s = r#"{
            "currentCoinbase" : "0000000000000000000000000000000000000000000000000000000000000001",
            "currentDifficulty" : "0x0100",
            "currentGasLimit" : "0x0f4240",
            "currentNumber" : "0x00",
            "currentTimestamp" : "0x01"
        }"#;
        let _deserialized: Env = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
