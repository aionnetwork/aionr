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

pub trait SyncProvider: Send + Sync {
    /// Get sync status
    fn status(&self) -> SyncStatus;
}

#[derive(Clone, Copy)]
pub struct SyncStatus {
    /// Syncing protocol version. That's the maximum protocol version we connect to.
    pub protocol_version: u8,
    /// The underlying p2p network version.
    pub network_id: u32,
    /// `BlockChain` height for the moment the sync started.
    pub start_block_number: u64,
    /// Highest block number in the download queue (if any).
    pub highest_block_number: Option<u64>,
    /// Total number of connected peers
    pub num_peers: usize,
}
