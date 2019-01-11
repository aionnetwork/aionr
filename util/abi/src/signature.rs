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

use blake2b::Blake2b;
use param_type::{Writer, ParamType};
use Hash;

pub fn short_signature(name: &str, params: &[ParamType]) -> [u8; 4] {
    let mut result = [0u8; 4];
    fill_signature(name, params, &mut result);
    result
}

pub fn long_signature(name: &str, params: &[ParamType]) -> Hash {
    let mut result = [0u8; 32];
    fill_signature(name, params, &mut result);
    result.into()
}

fn fill_signature(name: &str, params: &[ParamType], result: &mut [u8]) {
    let types = params
        .iter()
        .map(Writer::write)
        .collect::<Vec<String>>()
        .join(",");

    let data: Vec<u8> = From::from(format!("{}({})", name, types).as_str());

    let mut sponge = Blake2b::new(32);
    sponge.update(&data);
    sponge.finalize(result);
}

#[cfg(test)]
mod tests {
    use hex::FromHex;
    use super::short_signature;
    use {ParamType};

    #[test]
    fn test_signature() {
        assert_eq!(
            "921bcc0e".from_hex().unwrap(),
            short_signature("baz", &[ParamType::Uint(32), ParamType::Bool])
        );
    }
}
