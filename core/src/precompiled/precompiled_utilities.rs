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

use bytes::Bytes;

pub const WORD_LENGTH: usize = 32;
pub const HALF_WORD_LENGTH: usize = 16;

pub fn pad(input: Bytes, length: usize) -> Option<Bytes> {
    match input.len() {
        input_length if input_length > length => None,
        input_length if input_length == length => Some(input),
        input_length => {
            let mut output: Bytes = vec![0; length];
            let delta_length: usize = length - input_length;
            output[delta_length..].copy_from_slice(&input[0..]);
            Some(output)
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use super::pad;

    #[test]
    fn bytes_pad_0() {
        let test_expected: Bytes = vec![0, 0, 1, 2, 3];
        let test_input: Bytes = vec![1, 2, 3];
        let test_output: Bytes = pad(test_input, 5).unwrap();
        assert_eq!(test_output, test_expected);
    }
}
