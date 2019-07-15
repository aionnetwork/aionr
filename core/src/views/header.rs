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

//! View onto block header rlp

use acore_bytes::Bytes;
use aion_types::{H256, U256, Address, to_u256};
use ethbloom::Bloom;
use blake2b::blake2b;
use header::BlockNumber;
use header::HeaderVersion;
use rlp::Rlp;

/// View onto block header rlp.
pub struct HeaderView<'a> {
    rlp: Rlp<'a>,
}

impl<'a> HeaderView<'a> {
    /// Creates new view onto header from raw bytes.
    pub fn new(bytes: &'a [u8]) -> HeaderView<'a> {
        HeaderView {
            rlp: Rlp::new(bytes),
        }
    }

    /// Creates new view onto header from rlp.
    pub fn new_from_rlp(rlp: Rlp<'a>) -> HeaderView<'a> {
        HeaderView {
            rlp: rlp,
        }
    }

    /// Returns header hash.
    pub fn hash(&self) -> H256 { blake2b(self.rlp.as_raw()) }

    /// Returns raw rlp.
    pub fn rlp(&self) -> &Rlp<'a> { &self.rlp }

    /// Returns version.
    pub fn version(&self) -> HeaderVersion { self.rlp.val_at(0) }

    /// Returns parent hash.
    pub fn parent_hash(&self) -> H256 { self.rlp.val_at(2) }

    /// Returns author.
    pub fn author(&self) -> Address { self.rlp.val_at(3) }

    /// Returns state root.
    pub fn state_root(&self) -> H256 { self.rlp.val_at(4) }

    /// Returns transactions root.
    pub fn transactions_root(&self) -> H256 { self.rlp.val_at(5) }

    /// Returns block receipts root.
    pub fn receipts_root(&self) -> H256 { self.rlp.val_at(6) }

    /// Returns block log bloom.
    pub fn log_bloom(&self) -> Bloom { self.rlp.val_at(7) }

    /// Returns block difficulty.
    pub fn difficulty(&self) -> U256 { to_u256(self.rlp.val_at::<Vec<u8>>(8), 16) }

    /// Returns block number.
    pub fn number(&self) -> BlockNumber { self.rlp.val_at(1) }

    /// Returns block gas limit.
    pub fn gas_limit(&self) -> U256 { self.rlp.val_at(11) }

    /// Returns block gas used.
    pub fn gas_used(&self) -> U256 { self.rlp.val_at(10) }

    /// Returns timestamp.
    pub fn timestamp(&self) -> u64 { self.rlp.val_at(12) }

    /// Returns block extra data.
    pub fn extra_data(&self) -> Bytes { self.rlp.val_at(9) }

    /// Returns a vector of post-RLP-encoded seal fields.
    pub fn seal(&self) -> Vec<Bytes> {
        let mut seal = vec![];
        for i in 13..self.rlp.item_count() {
            seal.push(self.rlp.val_at(i));
        }
        seal
    }
}
