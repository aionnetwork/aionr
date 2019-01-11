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

//! Function param.
use ParamType;

/// Function param.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Param {
    /// Param name.
    pub name: String,
    /// Param type.
    #[serde(rename = "type")]
    pub kind: ParamType,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use {Param, ParamType};

    #[test]
    fn param_deserialization() {
        let s = r#"{
            "name": "foo",
            "type": "address"
        }"#;

        let deserialized: Param = serde_json::from_str(s).unwrap();

        assert_eq!(
            deserialized,
            Param {
                name: "foo".to_owned(),
                kind: ParamType::Address,
            }
        );
    }
}
