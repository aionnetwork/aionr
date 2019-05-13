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
use tiny_keccak::keccak256;
use rustc_hex::FromHex;
use num_bigint::ToBigInt;
use bytebuffer::ByteBuffer;
use blake2b::blake2b;
use aion_types::H256;

use precompiled::atb::bridge_transfer::{BridgeTransfer, TRANSFER_SIZE};
use precompiled::precompiled_utilities::{WORD_LENGTH, HALF_WORD_LENGTH, pad};

pub fn to_signature(func_signature: &str) -> [u8; 4] {
    let mut sig_chopped: [u8; 4] = [0u8; 4];
    let full: Bytes = keccak256(func_signature.as_bytes()).to_vec();
    sig_chopped.copy_from_slice(&full[..4]);
    sig_chopped
}

pub fn to_event_signature(event_signature: &str) -> [u8; 32] {
    keccak256(event_signature.as_bytes())
}

pub fn get_signature(input: Bytes) -> Option<Bytes> {
    if input.len() < 4 {
        None
    } else {
        let mut sig: Bytes = vec![0; 4];
        sig.copy_from_slice(&input[..4]);
        Some(sig)
    }
}

#[allow(dead_code)]
pub fn or_default_word(input: Option<Bytes>) -> Bytes {
    match input {
        Some(value) => value,
        None => vec![0; HALF_WORD_LENGTH],
    }
}

pub fn or_default_d_word(input: Option<Bytes>) -> Bytes {
    match input {
        Some(value) => value,
        None => vec![0; WORD_LENGTH],
    }
}

pub fn boolean_to_result_bytes(input: bool) -> Bytes {
    match input {
        true => FromHex::from_hex("00000000000000000000000000000001").unwrap(),
        false => vec![0; HALF_WORD_LENGTH],
    }
}

pub fn int_to_result_bytes(input: i32) -> Option<Bytes> {
    match input.to_bigint() {
        Some(input_bigint) => pad(input_bigint.to_signed_bytes_be(), 16),
        None => None,
    }
}

pub fn compute_bundle_hash(source_block_hash: H256, bundles: &Vec<BridgeTransfer>) -> H256 {
    let size: usize = source_block_hash.len() + bundles.len() * TRANSFER_SIZE;
    let mut buffer: ByteBuffer = ByteBuffer::new();
    buffer.resize(size);
    buffer.write_bytes(&source_block_hash.to_vec());
    for b in bundles {
        buffer.write_bytes(&b.get_src_transaction_hash().to_vec());
        buffer.write_bytes(&b.get_recipient().to_vec());
        buffer.write_bytes(&match &b.get_transfer_value_bytearray() {
            Some(value) => value.to_vec(),
            None => vec![],
        });
    }
    blake2b(buffer.to_bytes())
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use aion_types::H256;
    use super::{
        to_signature, to_event_signature, get_signature, or_default_word, or_default_d_word,
        boolean_to_result_bytes, int_to_result_bytes, compute_bundle_hash,
};
    use precompiled::precompiled_utilities::{WORD_LENGTH, HALF_WORD_LENGTH};
    use num_bigint::ToBigInt;
    use precompiled::atb::bridge_transfer::{BridgeTransfer, get_instance};

    #[test]
    fn test_to_signature() {
        let test_expected: [u8; 4] = [24, 65, 214, 83];
        let test_input: &str = "12345";
        let test_output: [u8; 4] = to_signature(test_input);
        assert_eq!(test_output, test_expected);
    }

    #[test]
    fn test_to_event_signature() {
        let test_expected: [u8; 32] = [
            24, 65, 214, 83, 249, 196, 237, 218, 157, 102, 167, 231, 115, 123, 57, 118, 61, 107,
            212, 15, 86, 154, 62, 198, 133, 157, 51, 5, 183, 35, 16, 230,
        ];
        let test_input: &str = "12345";
        let test_output: [u8; 32] = to_event_signature(test_input);
        assert_eq!(test_output, test_expected);
    }

    #[test]
    fn test_get_signature_some() {
        let test_expected: Bytes = vec![0, 1, 2, 3];
        let test_input: Bytes = vec![0, 1, 2, 3, 4, 5];
        let test_output: Option<Bytes> = get_signature(test_input);
        assert_eq!(test_output.unwrap(), test_expected);
    }

    #[test]
    fn test_get_signature_none() {
        let test_input: Bytes = vec![0, 1, 2];
        let test_output: Option<Bytes> = get_signature(test_input);
        assert_eq!(test_output, None);
    }

    #[test]
    fn test_or_default_word_some() {
        let test_expected: Bytes = vec![0, 1, 2, 3];
        let test_input: Bytes = vec![0, 1, 2, 3];
        let test_output: Bytes = or_default_word(Some(test_input));
        assert_eq!(test_output, test_expected);
    }

    #[test]
    fn test_or_default_word_none() {
        let test_expected: Bytes = vec![0; HALF_WORD_LENGTH];
        let test_output: Bytes = or_default_word(None);
        assert_eq!(test_output, test_expected);
    }

    #[test]
    fn test_or_default_d_word_some() {
        let test_expected: Bytes = vec![0, 1, 2, 3];
        let test_input: Bytes = vec![0, 1, 2, 3];
        let test_output: Bytes = or_default_d_word(Some(test_input));
        assert_eq!(test_output, test_expected);
    }

    #[test]
    fn test_or_default_d_word_none() {
        let test_expected: Bytes = vec![0; WORD_LENGTH];
        let test_output: Bytes = or_default_d_word(None);
        assert_eq!(test_output, test_expected);
    }

    #[test]
    fn test_boolean_to_result_bytes_true() {
        let test_expected: Bytes = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let test_output: Bytes = boolean_to_result_bytes(true);
        assert_eq!(test_output, test_expected);
    }

    #[test]
    fn test_boolean_to_result_bytes_false() {
        let test_expected: Bytes = vec![0; HALF_WORD_LENGTH];
        let test_output: Bytes = boolean_to_result_bytes(false);
        assert_eq!(test_output, test_expected);
    }

    #[test]
    fn test_int_to_result_bytes_positive() {
        let test_expected: Bytes = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255];
        let test_input: i32 = 255;
        let test_output: Option<Bytes> = int_to_result_bytes(test_input);
        assert_eq!(test_output.unwrap(), test_expected);
    }

    #[test]
    fn test_int_to_result_bytes_negative() {
        let test_expected: Bytes = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255];
        let test_input: i32 = -1;
        let test_output: Option<Bytes> = int_to_result_bytes(test_input);
        assert_eq!(test_output.unwrap(), test_expected);
    }

    #[test]
    fn test_compute_bundle_hash() {
        let test_expected: Bytes = vec![
            242, 75, 113, 212, 182, 18, 101, 250, 27, 133, 108, 144, 227, 235, 109, 158, 94, 115,
            66, 81, 63, 133, 8, 229, 164, 213, 178, 67, 92, 182, 32, 249,
        ];
        let bridge_transfer_1: BridgeTransfer = get_instance(
            100i32.to_bigint().unwrap(),
            H256::from_slice(&[0xffu8; 32]),
            H256::from_slice(&[0xfdu8; 32]),
        )
        .unwrap();
        let bridge_transfer_2: BridgeTransfer = get_instance(
            100i32.to_bigint().unwrap(),
            H256::from_slice(&[0xffu8; 32]),
            H256::from_slice(&[0xfdu8; 32]),
        )
        .unwrap();
        let transfers: Vec<BridgeTransfer> = vec![bridge_transfer_1, bridge_transfer_2];
        let source_block_hash: H256 = H256::from_slice(vec![0, 1, 2, 3].as_slice());
        let test_output: H256 = compute_bundle_hash(source_block_hash, &transfers);
        assert_eq!(test_output.to_vec(), test_expected);
    }
}
