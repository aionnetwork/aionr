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

use aion_types::{U256, H256};
use serde::ser::{Serialize, Serializer, SerializeStruct};
use rustc_hex::ToHex;

/// The result of an `eth_getWork` call: it differs based on an option
/// whether to send the block number.
#[derive(Debug, PartialEq, Eq)]
pub struct Work {
    /// The proof-of-work hash.
    pub pow_hash: H256,
    // The seed hash.
    // pub seed_hash: H256,
    /// parent hash.
    pub parent_hash: H256,
    /// The target.
    pub target: H256,
    /// The block number.
    pub number: u64,
    /// block minging reward.
    pub reward: U256,
    /// block total transaction fees.
    pub transaction_fee: U256,
}

impl Serialize for Work {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let mut work = serializer.serialize_struct("Work", 6)?;
        work.serialize_field("headerHash", &self.pow_hash.0.to_hex())?;
        work.serialize_field("previousblockhash", &self.parent_hash.0.to_hex())?;
        work.serialize_field("target", &self.target.0.to_hex())?;
        work.serialize_field("height", &self.number)?;
        work.serialize_field("blockBaseReward", &format!("{:x}", self.reward))?;
        work.serialize_field("blockTxFee", &format!("{:x}", self.transaction_fee))?;
        work.end()
    }
}

// Blockchain and node information to provide to miner
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Info {
    pub balance: u64,
    pub blocks: u64,
    pub connections: u64,
    pub proxy: String,
    pub generate: bool,
    pub genproclimit: u64,
    pub difficulty: u64,
}

// Validation info for address
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct AddressValidation {
    pub isvalid: bool,
    pub address: H256,
    pub ismine: bool,
}

// Mining information to provide to miner
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct MiningInfo {
    pub blocks: u64,
    pub currentblocksize: usize,
    pub currentblocktx: usize,
    pub difficulty: U256,
    pub testnet: bool,
}
