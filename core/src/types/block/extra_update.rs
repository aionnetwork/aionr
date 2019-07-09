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
use std::collections::HashMap;
use aion_types::H256;
use header::BlockNumber;
use blooms::{BloomGroup, GroupPosition};

use types::block::info::BlockInfo;
use types::blockchain::extra::{BlockDetails, BlockReceipts, TransactionAddress};

/// Block extras update info.
pub struct ExtrasUpdate<'a> {
    /// Block info.
    pub info: BlockInfo,
    /// Block timestamp.
    pub timestamp: u64,
    /// Current block uncompressed rlp bytes
    pub block: &'a [u8],
    /// Modified block hashes.
    pub block_hashes: HashMap<BlockNumber, H256>,
    /// Modified block details.
    pub block_details: HashMap<H256, BlockDetails>,
    /// Modified block receipts.
    pub block_receipts: HashMap<H256, BlockReceipts>,
    /// Modified blocks blooms.
    pub blocks_blooms: HashMap<GroupPosition, BloomGroup>,
    /// Modified transaction addresses (None signifies removed transactions).
    pub transactions_addresses: HashMap<H256, Option<TransactionAddress>>,
}
