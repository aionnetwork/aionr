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

//! Contract constructor call builder.
use {Param, Result, ErrorKind, Token, ParamType, encode, Bytes};

/// Contract constructor specification.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Constructor {
    /// Constructor input.
    pub inputs: Vec<Param>,
}

impl Constructor {
    /// Returns all input params of given constructor.
    fn param_types(&self) -> Vec<ParamType> { self.inputs.iter().map(|p| p.kind.clone()).collect() }

    /// Prepares ABI constructor call with given input params.
    pub fn encode_input(&self, code: Bytes, tokens: &[Token]) -> Result<Bytes> {
        let params = self.param_types();

        if Token::types_check(tokens, &params) {
            Ok(code.into_iter().chain(encode(tokens)).collect())
        } else {
            Err(ErrorKind::InvalidData.into())
        }
    }
}
