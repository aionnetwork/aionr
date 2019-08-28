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
use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt::{Display, Formatter, Error as FmtError};

use aion_types::{H256, U256};

#[derive(Clone, PartialEq)]
pub enum Mode {
    Normal,
    Backward,
    Forward,
    // Lightning,
    Thunder,
}

impl Display for Mode {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match *self {
            Mode::Normal => write!(f, "NORMAL"),
            Mode::Backward => write!(f, "BACKWARD"),
            Mode::Forward => write!(f, "FORWARD"),
            // Mode::Lightning => write!(f, "LIGHTNING"),
            Mode::Thunder => write!(f, "THUNDER"),
        }
    }
}

#[derive(Clone)]
pub struct NodeInfo {
    /// node total difficulty
    pub total_difficulty: U256,
    /// best block number
    pub best_block_number: u64,
    /// best block hash
    pub best_block_hash: H256,
    /// last headers request time
    pub last_headers_request_time: SystemTime,
    /// syncing mode
    pub mode: Mode,
    /// base number for backward and forward syncing
    pub branch_sync_base: u64,
}

impl NodeInfo {
    pub fn new() -> Self {
        NodeInfo {
            total_difficulty: U256::from(0u64),
            best_block_number: 0u64,
            best_block_hash: H256::default(),
            last_headers_request_time: UNIX_EPOCH,
            mode: Mode::Normal,
            branch_sync_base: 0u64,
        }
    }

    pub fn switch_mode(&mut self, mode: Mode, local_best: &u64, node_hash: &u64) {
        if self.mode != mode {
            debug!(target:"sync", "Node {}: switch to {} mode. local_best: {}, node best: {}", node_hash, &mode, local_best, &self.best_block_number);
            self.mode = mode;
        }
    }
}
