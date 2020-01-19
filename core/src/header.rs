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

//! Block header.

use blake2b::{blake2b, BLAKE2B_NULL_RLP};
use acore_bytes::{u64_to_bytes, Bytes};
use ethbloom::Bloom;
use aion_types::{Address, H256, U128, U256, to_u256};
use heapsize::HeapSizeOf;
use rlp::*;
use std::cell::RefCell;
use std::cmp;
use time::get_time;
use serde::{Serialize, Serializer};

pub use crate::types::BlockNumber;
use crate::encoded;

// TODO: better location?
pub fn u256_to_u128(value: U256) -> U128 {
    let U256(ref arr) = value;
    // ignore overflow condition.
    let mut ret = [0; 2];
    ret[0] = arr[0];
    ret[1] = arr[1];
    U128(ret)
}

pub fn u256_to_u16(value: U256) -> [u8; 2] {
    let mut val = [0u8; 32];
    let mut ret = [0u8; 2];
    value.to_big_endian(&mut val);
    ret[0] = val[30];
    ret[1] = val[31];
    ret
}

/// Semantic boolean for when a seal/signature is included.
pub enum Seal {
    /// The seal is included.
    With,
    /// The seal is not included.
    Without,
}

/// Seal type of a block header
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SealType {
    /// Proof of Work header type
    PoW,
    /// Proof of Stake header type
    PoS,
}

/// Implement default for SealType
impl Default for SealType {
    /// Default SealType is PoW
    fn default() -> Self { SealType::PoW }
}

/// Implement display for SealType
impl ::std::fmt::Display for SealType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.write_str(match *self {
            SealType::PoW => "PoW",
            SealType::PoS => "PoS",
        })
    }
}

/// Implement from u8. 1 for PoW and 2 for PoS.
impl From<u8> for SealType {
    fn from(v: u8) -> Self {
        match v {
            1u8 => SealType::PoW,
            2u8 => SealType::PoS,
            _ => SealType::PoW, // Question: consider other numbers as PoW too?
        }
    }
}

/// Implement into u8. 1 for PoW and 2 for PoS.
impl Into<u8> for SealType {
    fn into(self) -> u8 {
        match self {
            SealType::PoW => 1u8,
            SealType::PoS => 2u8,
        }
    }
}

/// Implement Encodable for SealType to get included in header's rlp
impl Encodable for SealType {
    fn rlp_append(&self, s: &mut RlpStream) {
        let v: u8 = self.to_owned().into();
        Encodable::rlp_append(&v, s);
    }
}

/// Implement Dencodable for SealType to get parsed from header's rlp
impl Decodable for SealType {
    fn decode(rlp: &UntrustedRlp) -> ::std::result::Result<Self, DecoderError> {
        rlp.as_val().and_then(|v: u8| Ok(SealType::from(v)))
    }
}

/// Implement Serialize for SealType
impl Serialize for SealType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        match *self {
            SealType::PoW => serializer.serialize_str("0x1"),
            SealType::PoS => serializer.serialize_str("0x2"),
        }
    }
}

/// A block header.
///
/// Reflects the specific RLP fields of a block in the chain with additional room for the seal
/// which is non-specific.
///
/// Doesn't do all that much on its own.
#[derive(Debug, Clone, Eq)]
pub struct Header {
    /// Seal Type
    seal_type: Option<SealType>,
    /// Block number
    number: BlockNumber,
    /// Parent hash
    parent_hash: H256,
    /// Block author
    author: Address,
    /// State root
    state_root: H256,
    /// Transactions root
    transactions_root: H256,
    /// Block receipts root
    receipts_root: H256,
    /// Block bloom
    log_bloom: Bloom,
    /// Block difficulty
    difficulty: U256,
    /// Block extra data
    extra_data: Bytes,
    /// Gas used for contracts execution
    gas_used: U256,
    /// Block gas limit
    gas_limit: U256,
    /// Block timestamp
    timestamp: u64,
    /// Vector of post-RLP-encoded fields. It includes (nonce, solution) for a PoW seal, or (seed, signature, public key) for a PoS seal
    seal: Vec<Bytes>,
    /// The memoized hash of the RLP representation *including* the seal fields.
    hash: RefCell<Option<H256>>,
    /// The memoized hash of the RLP representation *without* the seal fields.
    bare_hash: RefCell<Option<H256>>,
    /// transaction fee
    transaction_fee: U256,
    /// reward
    reward: U256,
}

impl PartialEq for Header {
    fn eq(&self, c: &Header) -> bool {
        self.seal_type == c.seal_type
            && self.parent_hash == c.parent_hash
            && self.timestamp == c.timestamp
            && self.number == c.number
            && self.author == c.author
            && self.transactions_root == c.transactions_root
            && self.extra_data == c.extra_data
            && self.state_root == c.state_root
            && self.receipts_root == c.receipts_root
            && self.log_bloom == c.log_bloom
            && self.gas_used == c.gas_used
            && self.gas_limit == c.gas_limit
            && self.difficulty == c.difficulty
            && self.seal == c.seal
            && self.transaction_fee == c.transaction_fee
            && self.reward == c.reward
    }
}

impl Default for Header {
    fn default() -> Self {
        Header {
            seal_type: Some(SealType::PoW),
            parent_hash: H256::default(),
            timestamp: 0,
            number: 0,
            author: Address::default(),
            transactions_root: BLAKE2B_NULL_RLP,
            extra_data: vec![],
            state_root: BLAKE2B_NULL_RLP,
            receipts_root: BLAKE2B_NULL_RLP,
            log_bloom: Bloom::default(),
            gas_used: U256::default(),
            gas_limit: U256::default(),
            difficulty: U256::default(),
            seal: vec![],
            hash: RefCell::new(None),
            bare_hash: RefCell::new(None),
            transaction_fee: U256::default(),
            reward: U256::default(),
        }
    }
}

impl Header {
    /// Create a new, default-valued, header.
    pub fn new() -> Self { Self::default() }

    /// Get seal type of the header
    pub fn seal_type(&self) -> &Option<SealType> { &self.seal_type }

    /// Get the parent_hash field of the header.
    pub fn parent_hash(&self) -> &H256 { &self.parent_hash }

    /// Get the timestamp field of the header.
    pub fn timestamp(&self) -> u64 { self.timestamp }

    /// Get the number field of the header.
    pub fn number(&self) -> BlockNumber { self.number }

    /// Get the author field of the header.
    pub fn author(&self) -> &Address { &self.author }

    /// Get the extra data field of the header.
    pub fn extra_data(&self) -> &Bytes { &self.extra_data }

    /// Get a mutable reference to extra_data
    pub fn extra_data_mut(&mut self) -> &mut Bytes {
        self.note_dirty();
        &mut self.extra_data
    }

    /// Get the state root field of the header.
    pub fn state_root(&self) -> &H256 { &self.state_root }

    /// Get the receipts root field of the header.
    pub fn receipts_root(&self) -> &H256 { &self.receipts_root }

    /// Get the log bloom field of the header.
    pub fn log_bloom(&self) -> &Bloom { &self.log_bloom }

    /// Get the transactions root field of the header.
    pub fn transactions_root(&self) -> &H256 { &self.transactions_root }

    /// Get the gas used field of the header.
    pub fn gas_used(&self) -> &U256 { &self.gas_used }

    /// Get the gas limit field of the header.
    pub fn gas_limit(&self) -> &U256 { &self.gas_limit }

    /// Get the difficulty field of the header.
    pub fn difficulty(&self) -> &U256 { &self.difficulty }

    /// Get the boundary of the header.
    pub fn boundary(&self) -> H256 {
        if self.difficulty <= U256::one() {
            U256::max_value().into()
        } else {
            (((U256::one() << 255) / self.difficulty) << 1).into()
        }
    }
    /// Get the seal field of the header.
    pub fn seal(&self) -> &[Bytes] { &self.seal }

    /// Get the public key of seal in PoS header
    pub fn get_pk_of_pos(&self) -> Option<H256> {
        if self
            .seal_type()
            .to_owned()
            .map_or(true, |t| t == SealType::PoW)
            || self.seal.len() != 3
        {
            return None;
        }

        Some(self.seal[2].as_slice().into())
    }

    /// Get the cumulative transaction fee
    pub fn transaction_fee(&self) -> &U256 { &self.transaction_fee }

    /// Get the reward
    pub fn reward(&self) -> &U256 { &self.reward }

    /// Set the seal type of the header.
    pub fn set_seal_type(&mut self, seal_type: SealType) {
        self.seal_type = Some(seal_type);
        self.note_dirty();
    }

    /// Set the number field of the header.
    pub fn set_parent_hash(&mut self, a: H256) {
        self.parent_hash = a;
        self.note_dirty();
    }

    /// Set the state root field of the header.
    pub fn set_state_root(&mut self, a: H256) {
        self.state_root = a;
        self.note_dirty();
    }

    /// Set the transactions root field of the header.
    pub fn set_transactions_root(&mut self, a: H256) {
        self.transactions_root = a;
        self.note_dirty()
    }

    /// Set the receipts root field of the header.
    pub fn set_receipts_root(&mut self, a: H256) {
        self.receipts_root = a;
        self.note_dirty()
    }

    /// Set the log bloom field of the header.
    pub fn set_log_bloom(&mut self, a: Bloom) {
        self.log_bloom = a;
        self.note_dirty()
    }

    /// Set the timestamp field of the header.
    pub fn set_timestamp(&mut self, a: u64) {
        self.timestamp = a;
        self.note_dirty();
    }

    /// Set the timestamp field of the header, but later than the specified time.
    pub fn set_timestamp_later_than(&mut self, a: u64, but_later_than: u64) {
        self.timestamp = cmp::max(a, but_later_than + 1);
        self.note_dirty();
    }

    /// Set the timestamp field of the header to the current time, but later than the specified time.
    pub fn set_timestamp_now_later_than(&mut self, but_later_than: u64) {
        self.timestamp = cmp::max(get_time().sec as u64, but_later_than + 1);
        self.note_dirty();
    }

    /// Set the number field of the header.
    pub fn set_number(&mut self, a: BlockNumber) {
        self.number = a;
        self.note_dirty();
    }

    /// Set the author field of the header.
    pub fn set_author(&mut self, a: Address) {
        if a != self.author {
            self.author = a;
            self.note_dirty();
        }
    }

    /// Set the extra data field of the header.
    pub fn set_extra_data(&mut self, a: Bytes) {
        if a != self.extra_data {
            self.extra_data = a;
            self.note_dirty();
        }
    }

    /// Set the gas used field of the header.
    pub fn set_gas_used(&mut self, a: U256) {
        self.gas_used = a;
        self.note_dirty();
    }

    /// Set the gas limit field of the header.
    pub fn set_gas_limit(&mut self, a: U256) {
        self.gas_limit = a;
        self.note_dirty();
    }

    /// Set the difficulty field of the header.
    pub fn set_difficulty(&mut self, a: U256) {
        self.difficulty = a;
        self.note_dirty();
    }
    /// Set the seal field of the header.
    pub fn set_seal(&mut self, a: Vec<Bytes>) {
        self.seal = a;
        self.note_dirty();
    }

    /// Cumulate transaction fee
    pub fn add_transaction_fee(&mut self, transaction_fee: &U256) {
        self.transaction_fee = self.transaction_fee + *transaction_fee;
    }

    /// Set block reward
    pub fn set_reward(&mut self, reward: U256) { self.reward = reward; }

    /// Get the hash of this header (blake2b of the RLP).
    pub fn hash(&self) -> H256 {
        let mut hash = self.hash.borrow_mut();
        match &mut *hash {
            &mut Some(ref h) => h.clone(),
            hash @ &mut None => {
                let h = self.rlp_blake2b(Seal::With);
                *hash = Some(h.clone());
                h
            }
        }
    }

    /// Get the hash of the header excluding the seal
    pub fn bare_hash(&self) -> H256 {
        let mut hash = self.bare_hash.borrow_mut();
        match &mut *hash {
            &mut Some(ref h) => h.clone(),
            hash @ &mut None => {
                let h = self.rlp_blake2b(Seal::Without);
                *hash = Some(h.clone());
                h
            }
        }
    }

    pub fn mine_hash(&self) -> H256 {
        let mut mine_hash_bytes: Vec<u8> = Vec::with_capacity(256);
        mine_hash_bytes.push(self.seal_type.to_owned().unwrap_or_default().into());
        mine_hash_bytes.extend(u64_to_bytes(self.number).iter());
        mine_hash_bytes.extend_from_slice(self.parent_hash.as_ref());
        mine_hash_bytes.extend_from_slice(self.author.as_ref());
        mine_hash_bytes.extend_from_slice(self.state_root.as_ref());
        mine_hash_bytes.extend_from_slice(self.transactions_root.as_ref());
        mine_hash_bytes.extend_from_slice(self.receipts_root.as_ref());
        mine_hash_bytes.extend_from_slice(self.log_bloom.data());
        let mut difficulty_buffer = [0u8; 16];
        u256_to_u128(self.difficulty).to_big_endian(&mut difficulty_buffer);
        mine_hash_bytes.extend_from_slice(&difficulty_buffer);
        mine_hash_bytes.extend(self.extra_data.iter());
        mine_hash_bytes.extend(u64_to_bytes(self.gas_used.low_u64()).iter());
        mine_hash_bytes.extend(u64_to_bytes(self.gas_limit.low_u64()).iter());
        mine_hash_bytes.extend(u64_to_bytes(self.timestamp).iter());
        if self.seal_type == Some(SealType::PoS) && self.seal.len() >= 1 {
            mine_hash_bytes.extend(
                self.seal
                    .get(0)
                    .expect("seal length greater than 0 tested before.")
                    .iter(),
            );
        }
        blake2b(mine_hash_bytes)
    }

    /// Note that some fields have changed. Resets the memoised hash.
    pub fn note_dirty(&self) {
        *self.hash.borrow_mut() = None;
        *self.bare_hash.borrow_mut() = None;
    }

    // TODO: make these functions traity
    /// Place this header into an RLP stream `s`, optionally `with_seal`.
    pub fn stream_rlp(&self, s: &mut RlpStream, with_seal: Seal) {
        s.begin_list(
            13 + match with_seal {
                Seal::With => self.seal.len(),
                _ => 0,
            },
        );

        s.append(&self.seal_type.clone().unwrap_or_default());
        s.append(&self.number);
        s.append(&self.parent_hash);
        s.append(&self.author);
        s.append(&self.state_root);
        s.append(&self.transactions_root);
        s.append(&self.receipts_root);
        s.append(&self.log_bloom);

        if self.number() == 0 {
            // for genesis
            let difficulty_buffer = u256_to_u16(self.difficulty);
            s.append(&difficulty_buffer.to_vec());
        } else {
            let mut difficulty_buffer = [0u8; 16];
            u256_to_u128(self.difficulty).to_big_endian(&mut difficulty_buffer);
            s.append(&difficulty_buffer.to_vec());
        }

        s.append(&self.extra_data);
        s.append(&self.gas_used);
        s.append(&self.gas_limit);
        s.append(&self.timestamp);
        if let Seal::With = with_seal {
            for b in &self.seal {
                s.append(b);
            }
        }
    }

    /// Get the RLP of this header, optionally `with_seal`.
    pub fn rlp(&self, with_seal: Seal) -> Bytes {
        let mut s = RlpStream::new();
        self.stream_rlp(&mut s, with_seal);
        s.out()
    }

    /// Get the SHA3 (blake2b) of this header, optionally `with_seal`.
    pub fn rlp_blake2b(&self, with_seal: Seal) -> H256 { blake2b(self.rlp(with_seal)) }

    /// Encode the header, getting a type-safe wrapper around the RLP.
    pub fn encoded(&self) -> encoded::Header { encoded::Header::new(self.rlp(Seal::With)) }
}

impl Decodable for Header {
    fn decode(r: &UntrustedRlp) -> Result<Self, DecoderError> {
        let mut blockheader = Header {
            seal_type: Some(r.val_at(0)?),
            number: r.val_at::<U256>(1)?.low_u64(),
            parent_hash: r.val_at(2)?,
            author: r.val_at(3)?,
            state_root: r.val_at(4)?,
            transactions_root: r.val_at(5)?,
            receipts_root: r.val_at(6)?,
            log_bloom: r.val_at(7)?,
            difficulty: to_u256(r.val_at::<Vec<u8>>(8)?, 16),
            extra_data: r.val_at(9)?,
            gas_used: to_u256(r.val_at::<Vec<u8>>(10)?, 8),
            gas_limit: to_u256(r.val_at::<Vec<u8>>(11)?, 8),
            timestamp: r.val_at::<U256>(12)?.low_u64(),
            seal: vec![],
            hash: RefCell::new(Some(blake2b(r.as_raw()))),
            bare_hash: RefCell::new(None),
            transaction_fee: U256::default(),
            reward: U256::default(),
        };

        for i in 13..r.item_count()? {
            blockheader.seal.push(r.val_at(i)?);
        }

        Ok(blockheader)
    }
}

impl Encodable for Header {
    fn rlp_append(&self, s: &mut RlpStream) { self.stream_rlp(s, Seal::With); }
}

impl HeapSizeOf for Header {
    fn heap_size_of_children(&self) -> usize {
        self.extra_data.heap_size_of_children() + self.seal.heap_size_of_children()
    }
}

impl ::aion_machine::Header for Header {
    fn bare_hash(&self) -> H256 { Header::bare_hash(self) }

    fn hash(&self) -> H256 { Header::hash(self) }

    fn seal(&self) -> &[Vec<u8>] { Header::seal(self) }

    fn author(&self) -> &Address { Header::author(self) }

    fn number(&self) -> BlockNumber { Header::number(self) }
}
