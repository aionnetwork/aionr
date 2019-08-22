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

use std::time::SystemTime;
use header::Header;
use block::Block;

#[derive(Clone, PartialEq)]
pub struct HeaderWrapper {
    pub node_hash: u64,
    pub timestamp: SystemTime,
    pub headers: Vec<Header>,
}

impl HeaderWrapper {
    pub fn new() -> Self {
        HeaderWrapper {
            node_hash: 0,
            timestamp: SystemTime::now(),
            headers: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct BlockWrapper {
    pub node_hash: u64,
    pub timestamp: SystemTime,
    pub blocks: Vec<Block>,
}

// impl BlockWrapper {
//     pub fn new() -> Self {
//         BlockWrapper {
//             node_hash: 0,
//             timestamp: SystemTime::now(),
//             blocks: Vec::new(),
//         }
//     }
// }
