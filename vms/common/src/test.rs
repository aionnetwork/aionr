use std::{u16, u32, u64, u8};
use crate::avm::{NativeEncoder, NativeDecoder};

use super::EnvInfo;

#[test]
pub fn test_codec() {
    let mut encoder = NativeEncoder::new();
    encoder.encode_byte(u8::MIN);
    encoder.encode_byte(u8::MAX);
    encoder.encode_short(u16::MIN);
    encoder.encode_short(u16::MAX);
    encoder.encode_int(u32::MIN);
    encoder.encode_int(u32::MAX);
    encoder.encode_long(u64::MIN);
    encoder.encode_long(u64::MAX);
    encoder.encode_bytes(&"".as_bytes().to_vec());
    encoder.encode_bytes(&"test".as_bytes().to_vec());
    let bytes = encoder.to_bytes();

    let mut decoder = NativeDecoder::new(&bytes);
    assert_eq!(u8::MIN, decoder.decode_byte().unwrap());
    assert_eq!(u8::MAX, decoder.decode_byte().unwrap());
    assert_eq!(u16::MIN, decoder.decode_short().unwrap());
    assert_eq!(u16::MAX, decoder.decode_short().unwrap());
    assert_eq!(u32::MIN, decoder.decode_int().unwrap());
    assert_eq!(u32::MAX, decoder.decode_int().unwrap());
    assert_eq!(u64::MIN, decoder.decode_long().unwrap());
    assert_eq!(u64::MAX, decoder.decode_long().unwrap());
    assert_eq!("".as_bytes().to_vec(), decoder.decode_bytes().unwrap());
    assert_eq!("test".as_bytes().to_vec(), decoder.decode_bytes().unwrap());
}

#[test]
fn it_can_be_created_as_default() {
    let default_env_info = EnvInfo::default();
    assert_eq!(default_env_info.difficulty, 0.into());
}
