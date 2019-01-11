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

use std::fmt;
use serde::{Deserialize, Deserializer};
use serde::de::{Error as SerdeError, Visitor};
use super::{ParamType, Reader};

impl<'a> Deserialize<'a> for ParamType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'a> {
        deserializer.deserialize_identifier(ParamTypeVisitor)
    }
}

struct ParamTypeVisitor;

impl<'a> Visitor<'a> for ParamTypeVisitor {
    type Value = ParamType;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a correct name of abi-encodable parameter type")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where E: SerdeError {
        Reader::read(value).map_err(|e| SerdeError::custom(format!("{:?}", e).as_str()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where E: SerdeError {
        self.visit_str(value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use serde_json;
    use ParamType;

    #[test]
    fn param_type_deserialization() {
        let s = r#"["address", "bytes", "bytes32", "bool", "string", "int", "uint", "address[]", "uint[3]", "bool[][5]"]"#;
        let deserialized: Vec<ParamType> = serde_json::from_str(s).unwrap();
        assert_eq!(
            deserialized,
            vec![
                ParamType::Address,
                ParamType::Bytes,
                ParamType::FixedBytes(32),
                ParamType::Bool,
                ParamType::String,
                ParamType::Int(256),
                ParamType::Uint(256),
                ParamType::Array(Box::new(ParamType::Address)),
                ParamType::FixedArray(Box::new(ParamType::Uint(256)), 3),
                ParamType::FixedArray(Box::new(ParamType::Array(Box::new(ParamType::Bool))), 5)
            ]
        );
    }
}
