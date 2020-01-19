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

use aion_types::{H256, U256};
use acore_bytes::Bytes;
use crate::header::BlockNumber;

/// Best block info.
#[derive(Default)]
pub struct BestBlock {
    /// Best block hash.
    pub hash: H256,
    /// Best block number.
    pub number: BlockNumber,
    /// Best block timestamp.
    pub timestamp: u64,
    /// Best block total difficulty.
    pub total_difficulty: U256,
    /// Best block uncompressed bytes
    pub block: Bytes,
}

/// Best ancient block info. If the blockchain has a gap this keeps track of where it starts.
#[derive(Default)]
pub struct BestAncientBlock {
    /// Best block hash.
    pub hash: H256,
    /// Best block number.
    pub number: BlockNumber,
}
