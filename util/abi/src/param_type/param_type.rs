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

//! Function and event param types.

use std::fmt;
use super::Writer;

/// Function and event param types.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamType {
    /// Address.
    Address,
    /// Bytes.
    Bytes,
    /// Signed integer.
    Int(usize),
    /// Unisgned integer.
    Uint(usize),
    /// Boolean.
    Bool,
    /// String.
    String,
    /// Array of unknown size.
    Array(Box<ParamType>),
    /// Vector of bytes with fixed size.
    FixedBytes(usize),
    /// Array with fixed size.
    FixedArray(Box<ParamType>, usize),
}

impl fmt::Display for ParamType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", Writer::write(self)) }
}

#[cfg(test)]
mod tests {
    use ParamType;

    #[test]
    fn test_param_type_display() {
        assert_eq!(format!("{}", ParamType::Address), "address".to_owned());
        assert_eq!(format!("{}", ParamType::Bytes), "bytes".to_owned());
        assert_eq!(
            format!("{}", ParamType::FixedBytes(32)),
            "bytes32".to_owned()
        );
        assert_eq!(format!("{}", ParamType::Uint(256)), "uint256".to_owned());
        assert_eq!(format!("{}", ParamType::Int(64)), "int64".to_owned());
        assert_eq!(format!("{}", ParamType::Bool), "bool".to_owned());
        assert_eq!(format!("{}", ParamType::String), "string".to_owned());
        assert_eq!(
            format!("{}", ParamType::Array(Box::new(ParamType::Bool))),
            "bool[]".to_owned()
        );
        assert_eq!(
            format!("{}", ParamType::FixedArray(Box::new(ParamType::String), 2)),
            "string[2]".to_owned()
        );
        assert_eq!(
            format!(
                "{}",
                ParamType::FixedArray(Box::new(ParamType::Array(Box::new(ParamType::Bool))), 2)
            ),
            "bool[][2]".to_owned()
        );
    }
}
