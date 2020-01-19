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

use aion_types::{H256, U256, Address};
use ajson;
use blake2b::BLAKE2B_NULL_RLP;
use crate::spec::seal::Seal;

pub struct Genesis {
    pub seal: Seal,
    pub difficulty: U256,
    pub author: Address,
    pub timestamp: u64,
    pub parent_hash: H256,
    pub extra_data: Vec<u8>,
    pub gas_limit: U256,

    pub gas_used: U256,
    pub transactions_root: H256,
    pub receipts_root: H256,
    pub state_root: Option<H256>,
}

impl From<ajson::spec::Genesis> for Genesis {
    fn from(g: ajson::spec::Genesis) -> Self {
        Genesis {
            seal: From::from(g.seal),
            difficulty: g.difficulty.into(),
            author: g.author.map_or_else(Address::zero, Into::into),
            timestamp: g.timestamp.map_or(0, Into::into),
            parent_hash: g.parent_hash.map_or_else(H256::zero, Into::into),
            extra_data: g.extra_data.map_or_else(Vec::new, Into::into),
            gas_limit: g.gas_limit.into(),

            gas_used: U256::from(0),
            state_root: None,
            transactions_root: BLAKE2B_NULL_RLP,
            receipts_root: BLAKE2B_NULL_RLP,
        }
    }
}
