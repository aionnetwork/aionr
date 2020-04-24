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

extern crate rustc_hex;

use std::fmt;
use std::cmp::min;
use std::ops::{Deref, DerefMut};

/// Slice pretty print helper
pub struct PrettySlice<'a>(&'a [u8]);

impl<'a> fmt::Debug for PrettySlice<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in 0..self.0.len() {
            match i > 0 {
                true => {
                    write!(f, "·{:02x}", self.0[i])?;
                }
                false => {
                    write!(f, "{:02x}", self.0[i])?;
                }
            }
        }
        Ok(())
    }
}

impl<'a> fmt::Display for PrettySlice<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in 0..self.0.len() {
            write!(f, "{:02x}", self.0[i])?;
        }
        Ok(())
    }
}

/// Trait to allow a type to be pretty-printed in `format!`, where unoverridable
/// defaults cannot otherwise be avoided.
pub trait ToPretty {
    /// Convert a type into a derivative form in order to make `format!` print it prettily.
    fn pretty(&self) -> PrettySlice;
    /// Express the object as a hex string.
    fn to_hex(&self) -> String { format!("{}", self.pretty()) }
}

impl<T: AsRef<[u8]>> ToPretty for T {
    fn pretty(&self) -> PrettySlice { PrettySlice(self.as_ref()) }
}

/// A byte collection reference that can either be a slice or a vector
pub enum BytesRef<'a> {
    /// This is a reference to a vector
    Flexible(&'a mut Bytes),
    /// This is a reference to a slice
    Fixed(&'a mut [u8]),
}

impl<'a> BytesRef<'a> {
    /// Writes given `input` to this `BytesRef` starting at `offset`.
    /// Returns number of bytes written to the ref.
    /// NOTE can return number greater then `input.len()` in case flexible vector had to be extended.
    pub fn write(&mut self, offset: usize, input: &[u8]) -> usize {
        match *self {
            BytesRef::Flexible(ref mut data) => {
                let data_len = data.len();
                let wrote = input.len() + if data_len > offset {
                    0
                } else {
                    offset - data_len
                };

                data.resize(offset, 0);
                data.extend_from_slice(input);
                wrote
            }
            BytesRef::Fixed(ref mut data) if offset < data.len() => {
                let max = min(data.len() - offset, input.len());
                for i in 0..max {
                    data[offset + i] = input[i];
                }
                max
            }
            _ => 0,
        }
    }
}

impl<'a> Deref for BytesRef<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match *self {
            BytesRef::Flexible(ref bytes) => bytes,
            BytesRef::Fixed(ref bytes) => bytes,
        }
    }
}

impl<'a> DerefMut for BytesRef<'a> {
    fn deref_mut(&mut self) -> &mut [u8] {
        match *self {
            BytesRef::Flexible(ref mut bytes) => bytes,
            BytesRef::Fixed(ref mut bytes) => bytes,
        }
    }
}

/// Vector of bytes.
pub type Bytes = Vec<u8>;

pub fn i64_to_bytes(i: i64) -> Bytes {
    use std::mem::transmute;
    let bytes: [u8; 8] = unsafe { transmute(i.to_be()) };
    bytes.to_vec()
}

pub fn u64_to_bytes(i: u64) -> Bytes {
    use std::mem::transmute;
    let bytes: [u8; 8] = unsafe { transmute(i.to_be()) };
    bytes.to_vec()
}

pub fn i32_to_bytes(i: i32) -> [u8; 4] {
    use std::mem::transmute;
    let bytes: [u8; 4] = unsafe { transmute(i) };
    bytes
}

pub fn i32_to_bytes_le(i: i32) -> [u8; 4] {
    use std::mem::transmute;
    let bytes: [u8; 4] = unsafe { transmute(i.to_le()) };
    bytes
}

static CHARS: &'static [u8] = b"0123456789abcdef";
pub fn to_hex(bytes: &[u8]) -> String {
    let mut v = Vec::with_capacity(bytes.len() * 2);
    for &byte in bytes.iter() {
        v.push(CHARS[(byte >> 4) as usize]);
        v.push(CHARS[(byte & 0xf) as usize]);
    }

    unsafe { String::from_utf8_unchecked(v) }
}

pub fn bytes_to_i32s(input: &[u8], output: &mut [i32], big_endian: bool) {
    let mut off: usize = 0;
    if !big_endian {
        for i in 0..output.len() {
            let mut ii = (input[off] as i32) & 0x000000FF;
            off += 1;
            ii |= 0x0000FF00 & ((input[off] as i32) << 8);
            off += 1;
            ii |= 0x00FF0000 & ((input[off] as i32) << 16);
            off += 1;
            ii |= (input[off] as i32) << 24;
            off += 1;
            output[i] = ii;
        }
    } else {
        for i in 0..output.len() {
            let mut ii = (input[off] as i32) << 24;
            off += 1;
            ii |= ((input[off] as i32) << 16) & 0x00FF0000;
            off += 1;
            ii |= ((input[off] as i32) << 8) & 0x0000FF00;
            off += 1;
            ii |= (input[off] as i32) & 0x000000FF;
            off += 1;
            output[i] = ii;
        }
    }
}

pub fn slice_to_array_32<T>(slice: &[T]) -> Option<&[T; 32]> {
    if slice.len() == 32 {
        Some(unsafe { &*(slice as *const [T] as *const [T; 32]) })
    } else {
        None
    }
}

pub fn slice_to_array_64<T>(slice: &[T]) -> Option<&[T; 64]> {
    if slice.len() == 64 {
        Some(unsafe { &*(slice as *const [T] as *const [T; 64]) })
    } else {
        None
    }
}

pub fn slice_to_array_80<T>(slice: &[T]) -> Option<&[T; 80]> {
    if slice.len() == 80 {
        Some(unsafe { &*(slice as *const [T] as *const [T; 80]) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{BytesRef, slice_to_array_32, slice_to_array_64, slice_to_array_80};
    use rustc_hex::ToHex;

    #[test]
    fn should_write_bytes_to_fixed_bytesref() {
        // given
        let mut data1 = vec![0, 0, 0];
        let mut data2 = vec![0, 0, 0];
        let (res1, res2) = {
            let mut bytes1 = BytesRef::Fixed(&mut data1[..]);
            let mut bytes2 = BytesRef::Fixed(&mut data2[1..2]);

            // when
            let res1 = bytes1.write(1, &[1, 1, 1]);
            let res2 = bytes2.write(3, &[1, 1, 1]);
            (res1, res2)
        };

        // then
        assert_eq!(&data1, &[0, 1, 1]);
        assert_eq!(res1, 2);

        assert_eq!(&data2, &[0, 0, 0]);
        assert_eq!(res2, 0);
    }

    #[test]
    fn should_write_bytes_to_flexible_bytesref() {
        // given
        let mut data1 = vec![0, 0, 0];
        let mut data2 = vec![0, 0, 0];
        let mut data3 = vec![0, 0, 0];
        let (res1, res2, res3) = {
            let mut bytes1 = BytesRef::Flexible(&mut data1);
            let mut bytes2 = BytesRef::Flexible(&mut data2);
            let mut bytes3 = BytesRef::Flexible(&mut data3);

            // when
            let res1 = bytes1.write(1, &[1, 1, 1]);
            let res2 = bytes2.write(3, &[1, 1, 1]);
            let res3 = bytes3.write(5, &[1, 1, 1]);
            (res1, res2, res3)
        };

        // then
        assert_eq!(&data1, &[0, 1, 1, 1]);
        assert_eq!(res1, 3);

        assert_eq!(&data2, &[0, 0, 0, 1, 1, 1]);
        assert_eq!(res2, 3);

        assert_eq!(&data3, &[0, 0, 0, 0, 0, 1, 1, 1]);
        assert_eq!(res3, 5);
    }

    #[test]
    fn test_bytes_to_i32s_4bytes() {
        let input: [u8; 4] = [0x01u8, 0x00u8, 0x00u8, 0x00u8];
        let mut output: [i32; 1] = [0];
        super::bytes_to_i32s(&input, &mut output, true);
        assert_eq!(16777216, output[0]);
        super::bytes_to_i32s(&input, &mut output, false);
        assert_eq!(1, output[0]);
    }

    #[test]
    fn test_bytes_to_i32s_8bytes() {
        let input: [u8; 8] = [
            0x01u8, 0x02u8, 0x03u8, 0x04u8, 0x05u8, 0x06u8, 0x07u8, 0x08u8,
        ];
        let mut output: [i32; 2] = [0; 2];
        super::bytes_to_i32s(&input, &mut output, true);
        assert_eq!(16909060, output[0]);
        assert_eq!(84281096, output[1]);
        super::bytes_to_i32s(&input, &mut output, false);
        assert_eq!(67305985, output[0]);
        assert_eq!(134678021, output[1]);
    }

    #[test]
    fn test_slice_to_array_32() {
        let slice: &[u8] = &[0; 32];
        let array_32 = slice_to_array_32(slice);
        assert_eq!(array_32, Some(&[0; 32]));

        let slice: &[u8] = &[0; 31];
        let array_32 = slice_to_array_32(slice);
        assert_eq!(array_32, None);
    }

    #[test]
    fn test_slice_to_array_64() {
        let slice: &[u8] = &[1; 64];
        let array_64 = slice_to_array_64(slice);
        assert_eq!(array_64.unwrap().to_hex(), [1; 64].to_hex());

        let slice: &[u8] = &[1; 31];
        let array_64 = slice_to_array_64(slice);
        assert!(array_64.is_none());
    }

    #[test]
    fn test_slice_to_array_80() {
        let slice: &[u8] = &[2; 80];
        let array_80 = slice_to_array_80(slice);
        assert_eq!(array_80.unwrap().to_hex(), [2; 80].to_hex());

        let slice: &[u8] = &[2; 31];
        let array_80 = slice_to_array_80(slice);
        assert!(array_80.is_none());
    }
}
