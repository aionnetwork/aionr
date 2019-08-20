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
use aion_types::H256;
use p2p::Node;
//use p2p::Mode;

pub struct NodeInfo {
    /// node total difficulty
    total_difficulty: H256,
    /// node best block number
    block_number: u64,
    /// node best block hash
    block_hash: H256,
    // node mode
    //mode: Mode
}

impl NodeInfo {
    pub fn new(td: H256, bn: u64, bh: H256) -> Self {
        NodeInfo {
            total_difficulty: td,
            block_number: bn,
            block_hash: bh,
            // mode :
        }
    }
    pub fn update(&mut self, td: H256, bn: u64, bh: H256) {
        self.total_difficulty = td;
        self.block_number = bn;
        self.block_hash = bh;
        //self.mode = ;
    }
}
