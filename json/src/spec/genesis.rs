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

//! Spec genesis deserialization.

use uint::Uint;
use hash::{Address, H256};
use bytes::Bytes;
use spec::Seal;

/// Spec genesis.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Genesis {
    /// Seal.
    pub seal: Seal,
    /// Difficulty.
    pub difficulty: Uint,
    /// Block author, defaults to 0.
    pub author: Option<Address>,
    /// Block timestamp, defaults to 0.
    pub timestamp: Option<Uint>,
    /// Parent hash, defaults to 0.
    #[serde(rename = "parentHash")]
    pub parent_hash: Option<H256>,
    /// Gas limit.
    #[serde(rename = "gasLimit")]
    pub gas_limit: Uint,
    /// Transactions root.
    #[serde(rename = "transactionsRoot")]
    pub transactions_root: Option<H256>,
    /// Receipts root.
    #[serde(rename = "receiptsRoot")]
    pub receipts_root: Option<H256>,
    /// State root.
    #[serde(rename = "stateRoot")]
    pub state_root: Option<H256>,
    /// Gas used.
    #[serde(rename = "gasUsed")]
    pub gas_used: Option<Uint>,
    /// Extra data.
    #[serde(rename = "extraData")]
    pub extra_data: Option<Bytes>,
}
