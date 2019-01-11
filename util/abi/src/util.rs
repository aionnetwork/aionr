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

//! Utils used by different modules.

use {Error, ErrorKind};

/// Convers vector of bytes with len equal n * 32, to a vector of slices.
pub fn slice_data(data: &[u8]) -> Result<Vec<[u8; 32]>, Error> {
    if data.len() % 32 != 0 {
        return Err(ErrorKind::InvalidData.into());
    }

    let times = data.len() / 32;
    let mut result = Vec::with_capacity(times);
    for i in 0..times {
        let mut slice = [0u8; 32];
        let offset = 32 * i;
        slice.copy_from_slice(&data[offset..offset + 32]);
        result.push(slice);
    }
    Ok(result)
}

/// Converts u32 to right aligned array of 32 bytes.
pub fn pad_u32(value: u32) -> [u8; 32] {
    let mut padded = [0u8; 32];
    padded[28] = (value >> 24) as u8;
    padded[29] = (value >> 16) as u8;
    padded[30] = (value >> 8) as u8;
    padded[31] = value as u8;
    padded
}

/// Converts i32 to right aligned array of 32 bytes.
pub fn pad_i32(value: i32) -> [u8; 32] {
    if value >= 0 {
        return pad_u32(value as u32);
    }

    let mut padded = [0xffu8; 32];
    padded[28] = (value >> 24) as u8;
    padded[29] = (value >> 16) as u8;
    padded[30] = (value >> 8) as u8;
    padded[31] = value as u8;
    padded
}

#[cfg(test)]
mod tests {
    use hex::FromHex;
    use super::pad_i32;

    #[test]
    fn test_i32() {
        assert_eq!(
            "0000000000000000000000000000000000000000000000000000000000000000"
                .from_hex()
                .unwrap(),
            pad_i32(0).to_vec()
        );
        assert_eq!(
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                .from_hex()
                .unwrap(),
            pad_i32(-1).to_vec()
        );
        assert_eq!(
            "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe"
                .from_hex()
                .unwrap(),
            pad_i32(-2).to_vec()
        );
        assert_eq!(
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00"
                .from_hex()
                .unwrap(),
            pad_i32(-256).to_vec()
        );
    }
}
