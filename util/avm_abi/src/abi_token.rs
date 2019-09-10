#![allow(unused)]

use std::mem;
use std::fmt::{Display, Formatter, Error as FmtError};

pub trait ToBytes {
    fn to_vm_bytes(&self) -> Vec<u8>;
}

pub trait FromBytes {
    fn to_u32(&self) -> u32;
}

pub trait ToBe<T> {
    fn to_be(&self) -> T;
}

impl ToBe<u32> for f32 {
    fn to_be(&self) -> u32 {
        let data = unsafe { mem::transmute::<f32, u32>(*self) };
        data.to_be()
    }
}

impl ToBe<u64> for f64 {
    fn to_be(&self) -> u64 {
        let data = unsafe { mem::transmute::<f64, u64>(*self) };
        data.to_be()
    }
}

impl FromBytes for [u8; 4] {
    fn to_u32(&self) -> u32 {
        let ret: &u32 = unsafe { mem::transmute(self) };
        return ret.to_be();
    }
}

impl FromBytes for [u8] {
    fn to_u32(&self) -> u32 {
        assert!(self.len() >= 4);
        let ret: &u32 = unsafe { mem::transmute(&self[0]) };
        return ret.to_be();
    }
}

macro_rules! format_as_bytes {
    ($type_name:ident, $len:expr) => {
        impl ToBytes for $type_name {
            fn to_vm_bytes(&self) -> Vec<u8> {
                let bytes: [u8; $len] = unsafe { mem::transmute(self.to_be()) };

                bytes.to_vec()
            }
        }
    };
}

format_as_bytes!(u16, 2);
format_as_bytes!(i16, 2);
format_as_bytes!(u32, 4);
format_as_bytes!(i32, 4);
format_as_bytes!(u64, 8);
format_as_bytes!(i64, 8);
format_as_bytes!(f32, 4);
format_as_bytes!(f64, 8);

pub enum AbiToken<'a> {
    UCHAR(u8),
    BOOL(bool),
    INT8(i8),
    INT16(i16),
    INT32(i32),
    INT64(i64),
    FLOAT(f32),
    DOUBLE(f64),
    AUCHAR(&'a [u8]),
    ABOOL(&'a [bool]),
    AINT8(&'a [i8]),
    AINT16(&'a [i16]),
    AINT32(&'a [i32]),
    AINT64(&'a [i64]),
    AFLOAT(&'a [f32]),
    ADOUBLE(&'a [f64]),
    STRING(String),
    // METHOD(String),
    ADDRESS([u8; 32]),
}

pub trait AVMEncoder {
    fn encode(&self) -> Vec<u8>;
}

impl<'a> AVMEncoder for AbiToken<'a> {
    fn encode(&self) -> Vec<u8> {
        let mut res = Vec::new();
        match *self {
            AbiToken::UCHAR(v) => {
                res.push(0x01);
                res.push(v);
            }
            AbiToken::BOOL(v) => {
                res.push(0x02);
                if v {
                    res.push(0x01);
                } else {
                    res.push(0x0);
                }
            }
            AbiToken::INT8(v) => {
                res.push(0x03);
                res.push(v as u8);
            }
            AbiToken::INT16(v) => {
                res.push(0x04);
                res.append(&mut v.to_vm_bytes())
            }
            AbiToken::INT32(v) => {
                res.push(0x05);
                res.append(&mut v.to_vm_bytes())
            }
            AbiToken::INT64(v) => {
                res.push(0x06);
                res.append(&mut v.to_vm_bytes())
            }
            AbiToken::FLOAT(v) => {
                res.push(0x07);
                res.append(&mut v.to_vm_bytes())
            }
            AbiToken::DOUBLE(v) => {
                res.push(0x08);
                res.append(&mut v.to_vm_bytes())
            }
            AbiToken::AUCHAR(v) => {
                res.push(0x11);
                for item in v {
                    res.push(*item)
                }
            }
            AbiToken::ABOOL(v) => {
                res.push(0x12);
                for item in v {
                    if *item {
                        res.push(0x01)
                    } else {
                        res.push(0x02)
                    }
                }
            }
            AbiToken::AINT8(v) => {
                res.push(0x13);
                for item in v {
                    res.push(*item as u8)
                }
            }
            AbiToken::AINT16(v) => {
                res.push(0x14);
                for item in v {
                    res.append(&mut item.to_vm_bytes());
                }
            }
            AbiToken::AINT32(v) => {
                res.push(0x15);
                for item in v {
                    res.append(&mut item.to_vm_bytes());
                }
            }
            AbiToken::AINT64(v) => {
                res.push(0x16);
                for item in v {
                    res.append(&mut item.to_vm_bytes());
                }
            }
            AbiToken::AFLOAT(v) => {
                res.push(0x17);
                for item in v {
                    res.append(&mut item.to_vm_bytes());
                }
            }
            AbiToken::ADOUBLE(v) => {
                res.push(0x18);
                for item in v {
                    res.append(&mut item.to_vm_bytes())
                }
            }
            AbiToken::STRING(ref v) => {
                res.push(0x21);
                res.append(&mut (v.len() as i16).to_vm_bytes());
                res.append(&mut v.clone().into_bytes());
            }
            // AbiToken::METHOD(ref s) => {
            //     res.push(0x21);
            //     res.append(&mut (s.len() as u16).to_vm_bytes());
            //     res.append(&mut s.clone().into_bytes());
            // }
            AbiToken::ADDRESS(addr) => {
                res.push(0x22);
                res.extend(addr.iter());
            }
        }

        res
    }
}

pub struct AVMDecoder {
    bytes: Vec<u8>,
    offset: usize,
}

#[derive(PartialEq, Debug, Clone)]
pub enum DecodeError {
    NoEnoughBytes,
    UnknownFormat,
}

impl Display for DecodeError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match *self {
            DecodeError::NoEnoughBytes => write!(f, "{}", "NoEnoughBytes"),
            DecodeError::UnknownFormat => write!(f, "{}", "UnknownTargetFormat"),
        }
    }
}

impl AVMDecoder {
    pub fn new(input: Vec<u8>) -> Self {
        AVMDecoder {
            bytes: input,
            offset: 0,
        }
    }

    fn eat(&mut self, num: usize) -> Result<(), DecodeError> {
        if self.offset + num >= self.bytes.len() {
            return Err(DecodeError::NoEnoughBytes);
        }
        self.offset += num;
        Ok(())
    }

    fn require(&self, num: usize) -> Result<(), DecodeError> { Ok(()) }

    pub fn decode_ulong(&mut self) -> Result<u64, DecodeError> {
        self.eat(1)?;
        self.require(8)?;
        let mut ret = 0;
        for i in self.offset..self.offset + 8 {
            ret = ret | (self.bytes[i] as u64) << (7 + self.offset - i) * 8;
        }

        Ok(ret)
    }

    pub fn decode_one_address(&mut self) -> Result<[u8; 32], DecodeError> {
        self.eat(1)?;
        self.require(32)?;
        let mut ret = [0u8; 32];
        for i in self.offset..self.offset + 32 {
            ret[i - self.offset] = self.bytes[i];
        }

        // deep copy
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode() {
        let mut method = AbiToken::STRING("sayHello".to_string());
        let mut data_0 = AbiToken::UCHAR(0x01u8);

        assert_eq!(
            method.encode(),
            vec![0x21, 0x00, 0x08, 0x73, 0x61, 0x79, 0x48, 0x65, 0x6c, 0x6c, 0x6f,]
        );
        assert_eq!(data_0.encode(), vec![0x01, 0x01]);
        data_0 = AbiToken::UCHAR(0xff);
        assert_eq!(data_0.encode(), vec![0x01, 0xff]);
        data_0 = AbiToken::INT32(123);
        assert_eq!(data_0.encode(), vec![0x05, 0x00, 0x00, 0x00, 0x7b]);
        method = AbiToken::STRING("method".to_string());
        assert_eq!(
            method.encode(),
            vec![0x21, 0x00, 0x06, 0x6d, 0x65, 0x74, 0x68, 0x6f, 0x64]
        );
        data_0 = AbiToken::FLOAT(1.0);
        assert_eq!(data_0.encode(), vec![0x07, 0x3f, 0x80, 0x00, 0x00]);
        data_0 = AbiToken::AFLOAT(&[1.0, 2.0]);
        assert_eq!(data_0.encode(), vec![23, 63, 128, 0, 0, 64, 0, 0, 0]);
        data_0 = AbiToken::DOUBLE(1.0);
        assert_eq!(
            data_0.encode(),
            vec![0x08, 0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
        data_0 = AbiToken::ADDRESS([
            0xa0, 0xb8, 0xae, 0x18, 0xd8, 0x1f, 0xe0, 0x58, 0x06, 0x8e, 0xbe, 0x7c, 0x2e, 0x9c,
            0xcd, 0x3b, 0x77, 0x69, 0xd7, 0x14, 0x62, 0x98, 0x62, 0x91, 0x83, 0x89, 0x33, 0x09,
            0x57, 0x06, 0xe6, 0x4b,
        ]);
        println!("{:x?}", data_0.encode());
        data_0 = AbiToken::ADOUBLE(&[1.0, 2.0]);
        assert_eq!(
            data_0.encode(),
            vec![24, 63, 240, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0]
        );

        data_0 = AbiToken::STRING("vote".to_string());
        println!("vote = {:x?}", data_0.encode());
        println!(
            "register = {:x?}",
            AbiToken::STRING("register".to_string()).encode()
        );
        println!(
            "getVote = {:x?}",
            AbiToken::STRING("getVote".to_string()).encode()
        );
        assert_eq!(data_0.encode(), vec![33, 0, 4, 118, 111, 116, 101]);
    }

    #[test]
    fn decode() {
        let raw = [0x1u8, 0, 0, 0];
        assert_eq!(raw.to_u32(), 16777216);
        // decode u64
        let mut decoder = AVMDecoder::new(vec![0x11, 0, 0, 0, 0, 0, 0, 0, 99]);
        assert_eq!(decoder.decode_ulong(), Ok(99));
    }
}
