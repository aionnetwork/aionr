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

//! Engine deserialization.

use super::{POWEquihashEngine, NullEngine};

/// Engine deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub enum Engine {
    /// POW Equihash engine
    POWEquihashEngine(POWEquihashEngine),
    /// Null engine.
    #[serde(rename = "null")]
    Null(NullEngine),
    /// Instantly sealing engine.
    #[serde(rename = "instantSeal")]
    InstantSeal,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use spec::Engine;

    #[test]
    fn engine_deserialization() {
        let s = r#"{
            "null": {
                "params": {
                    "blockReward": "0x0d"
                }
            }
        }"#;

        let deserialized: Engine = serde_json::from_str(s).unwrap();
        match deserialized {
            Engine::Null(_) => {} // unit test in its own file.
            _ => panic!(),
        }

        let s = r#"{
            "instantSeal": null
        }"#;

        let deserialized: Engine = serde_json::from_str(s).unwrap();
        match deserialized {
            Engine::InstantSeal => {} // instant seal is unit tested in its own file.
            _ => panic!(),
        };
    }
}
