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

//! Information about portions of the state and chain which the client may serve.
//!
//! Currently assumes that a client will store everything past a certain point
//! or everything. Will be extended in the future to support a definition
//! of which portions of the ancient chain and current state trie are stored as well.

/// Client pruning info. See module-level docs for more details.
#[derive(Debug, Clone)]
pub struct PruningInfo {
    /// The first block which everything can be served after.
    pub earliest_chain: u64,
    /// The first block where state requests may be served.
    pub earliest_state: u64,
}
