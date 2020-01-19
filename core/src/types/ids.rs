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

//! Unique identifiers.

use aion_types::H256;
use crate::{types::BlockNumber};

/// Uniquely identifies block.
#[derive(Debug, PartialEq, Copy, Clone, Hash, Eq)]
pub enum BlockId {
    /// Block's sha3.
    /// Querying by hash is always faster.
    Hash(H256),
    /// Block number within canon blockchain.
    Number(BlockNumber),
    /// Earliest block (genesis).
    Earliest,
    /// Latest mined block.
    Latest,
    /// Pending block.
    Pending,
}

/// Uniquely identifies transaction.
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub enum TransactionId {
    /// Transaction's sha3.
    Hash(H256),
    /// Block id and transaction index within this block.
    /// Querying by block position is always faster.
    Location(BlockId, usize),
}

/// Uniquely identifies Trace.
pub struct TraceId {
    /// Transaction
    pub transaction: TransactionId,
    /// Trace address within transaction.
    pub address: Vec<usize>,
}

/// Uniquely identifies Uncle.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct UncleId {
    /// Block id.
    pub block: BlockId,
    /// Position in block.
    pub position: usize,
}
