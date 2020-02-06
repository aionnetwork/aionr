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

use std::cmp::Ordering;
use num::ToPrimitive;
use num_bigint::{BigInt, ToBigInt, Sign};
use acore_bytes::Bytes;
use aion_types::H256;
use super::bundle_request_call::BundleRequestCall;
use super::bridge_transfer::{BridgeTransfer, get_instance};
const FUNCTION_SIGNATURE: u64 = 4;
const DOUBLE_WORD_SIZE: u64 = 32;
const ADDRESS_SIZE: u64 = 32;

/// length of meta: 16 bytes
/// types of meta: 1) position pointer 2) length
/// we note here that despite being 16 bytes, we only use enough bytes such that
/// the range stays within a positive integer (31 bits).
const LIST_META_LEN: u64 = 16;
const LIST_SIZE_MAX: u64 = 512;
const ADDRESS_HEADER: u8 = 0xa0;

const MAX_SIGNED_4_BYTES_AS_U64: u64 = i32::max_value() as u64;
lazy_static! {
    static ref MAX_SIGNED_4_BYTES_AS_BIGINT: BigInt = i32::max_value().to_bigint().unwrap();
}

/// length && prefix 0xa0 check
fn is_invalid_address(address: &[u8]) -> bool {
    if address.len() as u64 != ADDRESS_SIZE || address[0] != ADDRESS_HEADER {
        return true;
    } else {
        return false;
    }
}

/// dynamic array meta: position of array length && length
/// meta length 16 bytes
/// [b,b,b,b,b,b, .... ,b,b,b,b,b,b,b,b,b,b,b,b,b,b,b,b ... ,b,b,b,b,b,b,b,b,b,b,b,b,b,b,b,b,b,b,b,b,b,b,b ... ,b,b,b,b,b,b]
/// |<-     |          |                           call len |                              |                            ->|
/// |<-fn ->|          |                              |     |                              |
///         |<-offset->|                              |     |                              |
///                    |<-        list pos          ->|     |                              |
///                                                         |<-         list len         ->|<-          list data       ->|
/// fn: 4 bytes function signature
/// offset: index which is relative from end of function signature
/// list pos & list len: 16 bytes
fn parse_meta(call: &[u8], offset: u64) -> Option<u64> {
    let call_len: u64 = call.len() as u64;
    if offset > i32::max_value() as u64 || offset > call_len || (offset + LIST_META_LEN) > call_len
    {
        return None;
    }
    let mut meta = vec![0x00u8; LIST_META_LEN as usize];
    meta[0..LIST_META_LEN as usize]
        .copy_from_slice(&call[offset as usize..(offset + LIST_META_LEN) as usize]);
    let pt_bigint = BigInt::from_bytes_be(Sign::Plus, &meta);
    if pt_bigint.cmp(&MAX_SIGNED_4_BYTES_AS_BIGINT) == Ordering::Greater {
        None
    } else {
        Some(pt_bigint.to_u64().unwrap())
    }
}

/// Parses a list given an offset, where the offset indicates the index that
/// contains the list metadata (offset) value.
/// Recall that list is encoded in the following pattern:
/// [...][list-pos][...][list-len][ele-0][ele-1][...]
/// we assume that the maximum size of each input will be limited to 2^32 - 1
/// param call: input call (with function signature)
/// param offset: the offset in the call at which to start
/// param element_length: the length of each element in the array
/// return Vec<Vec<u8>>, or none if list is improperly formatted
fn parse_list(call: &[u8], offset: u64, element_length: u64) -> Option<Vec<Vec<u8>>> {
    let call_len = call.len() as u64;
    if call_len < offset + LIST_META_LEN * 2 {
        return None;
    }

    match parse_meta(call, offset) {
        /*
         * Correct case S#1, found that we previously incremented the listOffset before
         * checking for ERR_INT, would have led to a situation where this check
         * (whether the first listOffset was invalid or not) would not trigger.
         * Correct by checking before incrementing with CALL_OFFSET.
         */
        Some(mut list_pos) => {
            list_pos = list_pos + FUNCTION_SIGNATURE;
            /*
             * parse_meta() performs checks on list_offset
             * Cover case S#4, assign an upper bound to the size of array we create.
             * Under current gas estimations, our token takes approximately 30k-50k gas for a
             * transfer.
             * 30_000 * 512 = 15_360_000, which is above the current Ethereum block limit.
             * Otherwise, if the limit does increase, we can simply cut bundles at this length.
             */
            match parse_meta(call, list_pos) {
                Some(list_len) => {
                    if list_len > LIST_SIZE_MAX {
                        return None;
                    }
                    /*
                     * Covers case Y#2, if attacker tries to construct and overflow to OOM output array,
                     * it will be caught here.
                     */
                    let consumed_length = list_len * element_length;
                    /*
                     * Recall that we confirmed listOffset <= call.length. To check that offset, we then
                     * further consumed the offset position.
                     */
                    if consumed_length > (call_len - list_pos - LIST_META_LEN) {
                        return None;
                    }

                    // max signed 4 bytes number as u64 overflow check
                    let start = list_pos + LIST_META_LEN;
                    if start > MAX_SIGNED_4_BYTES_AS_U64 {
                        return None;
                    }
                    let end = start + consumed_length;
                    if end > MAX_SIGNED_4_BYTES_AS_U64 {
                        return None;
                    }

                    let mut output: Vec<Vec<u8>> = Vec::new();
                    for i in 0..list_len {
                        let mut element = vec![0x00u8; element_length as usize];
                        element[0..element_length as usize].copy_from_slice(
                            &call[(i * element_length + start) as usize
                                ..((i + 1) * element_length + start) as usize],
                        );
                        output.push(element);
                    }
                    Some(output)
                }
                None => None,
            }
        }
        None => None,
    }
}

pub fn parse_double_word_from_call(call: &[u8]) -> Option<Vec<u8>> {
    let call_len: u64 = call.len() as u64;
    if call_len < FUNCTION_SIGNATURE + DOUBLE_WORD_SIZE {
        None
    } else {
        let mut double_word = vec![0x00u8; DOUBLE_WORD_SIZE as usize];
        double_word[0..ADDRESS_SIZE as usize].clone_from_slice(
            &call[FUNCTION_SIGNATURE as usize..(FUNCTION_SIGNATURE + ADDRESS_SIZE) as usize],
        );
        Some(double_word)
    }
}

/// parses a call with one owner address, externally this is known as change owner
/// assume that input contains function signature
/// param call input call with function signature
/// return address of new owner, null if anything is invalid
pub fn parse_address_from_call(call: &[u8]) -> Option<Vec<u8>> {
    match parse_double_word_from_call(call) {
        Some(address) => {
            if is_invalid_address(address.as_slice()) {
                None
            } else {
                Some(address)
            }
        }
        None => None,
    }
}

/// parses a list of addresses from input, currently only used by
/// ring initialization. This method enforces some checks on the class
/// of addresses before parsed, in that they <b>must</b> be user addresses
/// (start with 0xa0).
///
/// the implication being that you may not set a non-user, address to be
/// a ring member.
///
/// param call input data
/// return Vec<Vec<u8>> containing list of addresses. none otherwise.
pub fn parse_address_list(call: &[u8]) -> Option<Vec<Vec<u8>>> {
    let call_len: u64 = call.len() as u64;
    // check minimum length
    if call_len < (FUNCTION_SIGNATURE + DOUBLE_WORD_SIZE) {
        return None;
    }
    let address_list = parse_list(call, FUNCTION_SIGNATURE, 32);
    if address_list == None {
        return None;
    }
    let address_list_parsed = address_list.unwrap();
    for l in &address_list_parsed {
        if is_invalid_address(l.as_ref()) {
            return None;
        }
    }
    Some(address_list_parsed)
}

/// perhaps a length check is wrongly added here too, we do not want to
/// check later as other deserialization would be a waste.
/// param call input data
/// return BundleRequestCall containing deserialized data
/// none if anything regarding deserialization is wrong
pub fn parse_bundle_request(call: &[u8]) -> Option<BundleRequestCall> {
    /*
        source_transaction_list
        address_list
        value_list
        signature_chunks_1
        signature_chunks_2
        signature_chunks_3
    */

    if call.len() < (FUNCTION_SIGNATURE + DOUBLE_WORD_SIZE + (LIST_META_LEN * 2) * 5) as usize {
        return None;
    }

    let block_hash = parse_double_word_from_call(call);
    if block_hash == None {
        return None;
    }

    let parse_list_source_transaction_list =
        parse_list(call, FUNCTION_SIGNATURE + DOUBLE_WORD_SIZE, 32);
    if parse_list_source_transaction_list == None {
        return None;
    }

    let parse_list_address_list = parse_list(
        call,
        FUNCTION_SIGNATURE + DOUBLE_WORD_SIZE + LIST_META_LEN,
        32,
    );
    if parse_list_address_list == None {
        return None;
    }

    let parse_list_uint_list = parse_list(
        call,
        FUNCTION_SIGNATURE + DOUBLE_WORD_SIZE + (LIST_META_LEN * 2),
        16,
    );
    if parse_list_uint_list == None {
        return None;
    }

    let source_transaction_list = parse_list_source_transaction_list.unwrap();
    let address_list = parse_list_address_list.unwrap();
    let uint_list = parse_list_uint_list.unwrap();
    if address_list.len() != uint_list.len() || address_list.len() != source_transaction_list.len()
    {
        return None;
    }

    let parse_list_signature_chunk_0 = parse_list(
        call,
        FUNCTION_SIGNATURE + (LIST_META_LEN * 3) + DOUBLE_WORD_SIZE,
        32,
    );
    if parse_list_signature_chunk_0 == None {
        return None;
    }

    let parse_list_signature_chunk_1 = parse_list(
        call,
        FUNCTION_SIGNATURE + (LIST_META_LEN * 4) + DOUBLE_WORD_SIZE,
        32,
    );
    if parse_list_signature_chunk_1 == None {
        return None;
    }

    let parse_list_signature_chunk_2 = parse_list(
        call,
        FUNCTION_SIGNATURE + (LIST_META_LEN * 5) + DOUBLE_WORD_SIZE,
        32,
    );
    if parse_list_signature_chunk_2 == None {
        return None;
    }

    let signature_chunk_0: Vec<Vec<u8>> = parse_list_signature_chunk_0.unwrap();
    let signature_chunk_1: Vec<Vec<u8>> = parse_list_signature_chunk_1.unwrap();
    let signature_chunk_2: Vec<Vec<u8>> = parse_list_signature_chunk_2.unwrap();

    if signature_chunk_0.len() != signature_chunk_1.len()
        || signature_chunk_0.len() != signature_chunk_2.len()
    {
        return None;
    }

    let m = signature_chunk_0.len();
    let mut merged_signature_list: Vec<Bytes> = Vec::new();
    for i in 0..m {
        let m_0 = signature_chunk_0[i].len();
        let m_1 = signature_chunk_1[i].len();
        let m_2 = signature_chunk_2[i].len();
        let mut merged_signature = vec![0x00u8; m_0 + m_1 + m_2];

        merged_signature[0..m_0].copy_from_slice(&signature_chunk_0[i].as_slice()[0..m_0]);
        merged_signature[m_0..m_0 + m_1].copy_from_slice(&signature_chunk_1[i].as_slice()[0..m_1]);
        merged_signature[m_0 + m_1..m_0 + m_1 + m_2]
            .copy_from_slice(&signature_chunk_2[i].as_slice()[0..m_2]);
        merged_signature_list.push(merged_signature);
    }

    // package bundles
    let mut bundles: Vec<BridgeTransfer> = Vec::new();
    for i in 0..uint_list.len() {
        match get_instance(
            BigInt::from_bytes_be(Sign::Plus, uint_list[i].as_slice()),
            H256::from(address_list[i].as_slice()),
            H256::from(source_transaction_list[i].as_slice()),
        ) {
            Some(transfer) => {
                bundles.push(transfer);
            }
            None => {
                // any invalid transfer breaks bundle
                return None;
            }
        }
    }

    Some(BundleRequestCall {
        block_hash: Bytes::from(block_hash.unwrap()),
        bundles,
        signatures: merged_signature_list,
    })
}

/// RUST_BACKTRACE=1 cargo test --package ethcore --lib precompiled::atb::bridge_deserializer -- --nocapture
#[cfg(test)]
mod tests {

    use num_bigint::ToBigInt;
    use super::FUNCTION_SIGNATURE;
    use super::is_invalid_address;
    use super::parse_meta;
    use super::parse_list;
    use super::parse_address_list;

    // private function to simulate helper function on java kernel test case
    fn to_bytes(num: i32, output_len: usize) -> Option<Vec<u8>> {
        let unaligned_bytes = num.to_bigint().unwrap().to_bytes_be();
        let len = unaligned_bytes.1.len();
        let mut aligned_bytes = vec![0x00u8; output_len];

        // truncate data if needed
        if output_len > len {
            aligned_bytes[output_len - len..output_len].copy_from_slice(&unaligned_bytes.1[0..len]);
        } else {
            aligned_bytes[0..output_len].copy_from_slice(&unaligned_bytes.1[len - output_len..len]);
        }
        Some(aligned_bytes)
    }

    #[test]
    fn test_is_valid_address() {
        // len check
        let invalid_address_0: [u8; 0] = [];
        assert!(is_invalid_address(&invalid_address_0));
        // header check
        let invalid_address_1: [u8; 32] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];
        assert!(is_invalid_address(&invalid_address_1));
        let valid_address_0: [u8; 32] = [
            0xa0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];
        assert!(!is_invalid_address(&valid_address_0));
    }

    #[test]
    fn test_parse_meta() {
        // test max int overflow
        let max_int = vec![
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            0x00u8, 0x7fu8, 0xffu8, 0xffu8, 0xffu8,
        ];
        assert!(parse_meta(&max_int, 0).is_some());
        let max_int_overflow = vec![
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            0x00u8, 0x8fu8, 0xffu8, 0xffu8, 0xffu8,
        ];
        assert!(parse_meta(&max_int_overflow, 0).is_none());

        // test offset over
        assert!(parse_meta(&[0x00u8; 16], 0).is_some());
        assert!(parse_meta(&[0x00u8; 17], 1).is_some());
        assert!(parse_meta(&[0x00u8; 16], 1).is_none());
        assert!(parse_meta(&[0x00u8; 17], 2).is_none());
    }

    #[test]
    fn test_parse_list() {
        let invalid_len = [0x00u8; 32];
        assert!(parse_list(&invalid_len, 1, 0).is_none());

        // check over max list elements size
        let max_list_len = [
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x10u8, // pos 16
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            0x00u8, 0x00u8, 0x00u8, 0x02u8, 0x00u8, // len 512
        ];
        assert!(parse_list(&max_list_len, FUNCTION_SIGNATURE, 0).is_some());
        let over_max_list_len = [
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x10u8, // pos 16
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            0x00u8, 0x00u8, 0x00u8, 0x02u8, 0x01u8, // len 513
        ];
        assert!(parse_list(&over_max_list_len, FUNCTION_SIGNATURE, 0).is_none());

        // expected list data len out of bound
        let invalid_list_data_len = [
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x10u8, // pos 16
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x08u8, // len 8
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, // list data 7 bytes
        ];
        assert!(parse_list(&invalid_list_data_len, 4, 1).is_none());

        let list_data_len = [
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x10u8, // pos 16
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x08u8, // len 8
            0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            0x08u8, // list data 8 bytes
        ];
        assert!(parse_list(&list_data_len, 4, 1).is_some());

        // ignore test for (2g test data required)
        //  - start of list data over max signed int
        //  - end of list data over max signed int
    }

    /**
     * Tries to trigger an out of bounds exception on the first occurrence of parseMeta using some
     * integer overflow.
     * No assertions -- we are testing whether or not an exception gets thrown.
     */
    #[test]
    fn test_parse_address_list_integer_overflow_0() {
        let mut address_list: [u8; 36] = [0x00u8; 36];
        let max_int_bytes = to_bytes(i32::max_value(), 16).unwrap();
        address_list[4..20].copy_from_slice(&max_int_bytes[0..16]);
        parse_address_list(&address_list);
    }

    /**
     * Tries to trigger an out of bounds exception on the second occurrence of parseMeta using some
     * trickier integer overflowing.
     * No assertions -- we are testing whether or not an exception gets thrown.
     */
    #[test]
    fn test_parse_address_list_integer_overflow_1() {
        let mut address_list: [u8; 36] = [0x00u8; 36];
        let almost_max_int_bytes = to_bytes(i32::max_value() - 4, 16).unwrap();
        address_list[4..20].copy_from_slice(&almost_max_int_bytes[0..16]);
        parse_address_list(&address_list);
    }

    /**
     * Since the logic gives us the invariant: end <= call.length
     * and we access i + elementLength inside a loop that loops until end-1, this test gets some
     * numbers aligned so that end == call.length, the best place we can trigger an out of bounds.
     * No assertions -- we are testing whether or not an exception gets thrown.
     */
    #[test]
    fn test_parse_address_list_integer_overflow_2() {
        let mut address_list: [u8; 16425] = [0x00u8; 16425];
        let list_pos = to_bytes(21, 16).unwrap();
        println!(
            "test_parse_address_list_integer_overflow_2 -> list_pos\n{:?}",
            &list_pos
        );
        let list_len = to_bytes(512, 16).unwrap();
        println!(
            "test_parse_address_list_integer_overflow_2 -> list_len\n{:?}",
            &list_len
        );
        address_list[4..20].copy_from_slice(&list_pos[0..16]);
        address_list[4..20].copy_from_slice(&list_len[0..16]);
        parse_address_list(&address_list);
    }
}
