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

use aion_types::{H256, U256};

#[derive(Clone)]
pub struct NodeInfo {
    /// node total difficulty
    pub total_difficulty: U256,
    /// node best block number
    pub best_block_number: u64,
    /// node best block hash
    pub best_block_hash: H256,
    /// last headers request time
    pub last_headers_request_time: SystemTime,
    // node mode
    //mode: Mode
}

impl NodeInfo {
    pub fn new() -> Self {
        NodeInfo {
            total_difficulty: U256::from(0u64),
            best_block_number: 0u64,
            best_block_hash: H256::default(),
            last_headers_request_time: UNIX_EPOCH,
        }
    }
}
