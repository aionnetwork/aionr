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

//! Blockchain DB extras.

use std::ops;
use types::blooms::{GroupPosition, BloomGroup};
use db::Key;
use header::BlockNumber;
use receipt::Receipt;

use heapsize::HeapSizeOf;
use aion_types::{H256, H264, U256};

/// Represents index of extra data in database
#[derive(Copy, Debug, Hash, Eq, PartialEq, Clone)]
pub enum ExtrasIndex {
    /// Block details index
    BlockDetails = 0,
    /// Block hash index
    BlockHash = 1,
    /// Transaction address index
    TransactionAddress = 2,
    /// Block blooms index
    BlocksBlooms = 3,
    /// Block receipts index
    BlockReceipts = 4,
}

fn with_index(hash: &H256, i: ExtrasIndex) -> H264 {
    let mut result = H264::default();
    result[0] = i as u8;
    (*result)[1..].clone_from_slice(hash);
    result
}

pub struct BlockNumberKey([u8; 5]);

impl ops::Deref for BlockNumberKey {
    type Target = [u8];

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Key<H256> for BlockNumber {
    type Target = BlockNumberKey;

    fn key(&self) -> Self::Target {
        let mut result = [0u8; 5];
        result[0] = ExtrasIndex::BlockHash as u8;
        result[1] = (self >> 24) as u8;
        result[2] = (self >> 16) as u8;
        result[3] = (self >> 8) as u8;
        result[4] = *self as u8;
        BlockNumberKey(result)
    }
}

impl Key<BlockDetails> for H256 {
    type Target = H264;

    fn key(&self) -> Self::Target { with_index(self, ExtrasIndex::BlockDetails) }
}

pub struct LogGroupKey([u8; 6]);

impl ops::Deref for LogGroupKey {
    type Target = [u8];

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Key<BloomGroup> for GroupPosition {
    type Target = LogGroupKey;

    fn key(&self) -> Self::Target {
        let mut result = [0u8; 6];
        result[0] = ExtrasIndex::BlocksBlooms as u8;
        result[1] = self.level;
        result[2] = (self.index >> 24) as u8;
        result[3] = (self.index >> 16) as u8;
        result[4] = (self.index >> 8) as u8;
        result[5] = self.index as u8;
        LogGroupKey(result)
    }
}

impl Key<TransactionAddress> for H256 {
    type Target = H264;

    fn key(&self) -> H264 { with_index(self, ExtrasIndex::TransactionAddress) }
}

impl Key<BlockReceipts> for H256 {
    type Target = H264;

    fn key(&self) -> H264 { with_index(self, ExtrasIndex::BlockReceipts) }
}

/// Familial details concerning a block
#[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
pub struct BlockDetails {
    /// Block number
    pub number: BlockNumber,
    /// Total difficulty of the block and all its parents
    pub total_difficulty: U256,
    /// PoW total difficulty of all the PoW block till this block
    pub pow_total_difficulty: U256,
    /// PoS total difficulty of all the PoS block till this block
    pub pos_total_difficulty: U256,
    /// Parent block hash
    pub parent: H256,
    /// List of children block hashes
    pub children: Vec<H256>,
    /// The anti seal parent hash
    pub anti_seal_parent: Option<H256>,
}

impl HeapSizeOf for BlockDetails {
    fn heap_size_of_children(&self) -> usize { self.children.heap_size_of_children() }
}

/// Represents address of certain transaction within block
#[derive(Debug, PartialEq, Clone, RlpEncodable, RlpDecodable)]
pub struct TransactionAddress {
    /// Block hash
    pub block_hash: H256,
    /// Transaction index within the block
    pub index: usize,
}

impl HeapSizeOf for TransactionAddress {
    fn heap_size_of_children(&self) -> usize { 0 }
}

/// Contains all block receipts.
#[derive(Clone, RlpEncodableWrapper, RlpDecodableWrapper)]
pub struct BlockReceipts {
    pub receipts: Vec<Receipt>,
}

impl BlockReceipts {
    pub fn new(receipts: Vec<Receipt>) -> Self {
        BlockReceipts {
            receipts: receipts,
        }
    }
}

impl HeapSizeOf for BlockReceipts {
    fn heap_size_of_children(&self) -> usize { self.receipts.heap_size_of_children() }
}
