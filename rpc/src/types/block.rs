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

use acore::encoded::Header as EthHeader;
use aion_types::{H256, U256};
use ethbloom::Bloom;

use serde::{Serialize, Serializer};
use types::{Bytes, Transaction};

/// Block Transactions
#[derive(Debug)]
pub enum BlockTransactions {
    /// Only hashes
    Hashes(Vec<H256>),
    /// Full transactions
    Full(Vec<Transaction>),
}

impl Serialize for BlockTransactions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        match *self {
            BlockTransactions::Hashes(ref hashes) => hashes.serialize(serializer),
            BlockTransactions::Full(ref ts) => ts.serialize(serializer),
        }
    }
}

/// Block representation
#[derive(Debug, Serialize)]
pub struct Block {
    /// Hash of the block
    pub hash: Option<H256>,
    /// Hash of the parent
    #[serde(rename = "parentHash")]
    pub parent_hash: H256,
    // TODO: get rid of this one
    /// ?
    pub miner: H256,
    /// State root hash
    #[serde(rename = "stateRoot")]
    pub state_root: H256,
    /// Transactions root hash
    #[serde(rename = "transactionsRoot")]
    pub transactions_root: H256,
    /// Transactions receipts root hash
    #[serde(rename = "receiptsRoot")]
    pub receipts_root: H256,
    /// Block number
    pub number: Option<u64>,
    /// Gas Used
    #[serde(rename = "gasUsed")]
    pub gas_used: U256,
    /// Gas Limit
    #[serde(rename = "gasLimit")]
    pub gas_limit: U256,
    /// Extra data
    #[serde(rename = "extraData")]
    pub extra_data: Bytes,
    /// Logs bloom
    #[serde(rename = "logsBloom")]
    pub logs_bloom: Bloom,
    /// Timestamp
    pub timestamp: U256,
    /// Difficulty
    pub difficulty: U256,
    /// Total difficulty
    #[serde(rename = "totalDifficulty")]
    pub total_difficulty: Option<U256>,
    /// nonce
    pub nonce: Option<Bytes>,
    /// solution
    pub solution: Option<Bytes>,
    /// Transactions
    pub transactions: BlockTransactions,
    /// Size in bytes
    pub size: Option<U256>,
}

/// Block header representation.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Header {
    /// Hash of the block
    pub hash: Option<H256>,
    /// Hash of the parent
    #[serde(rename = "parentHash")]
    pub parent_hash: H256,
    // TODO: get rid of this one
    /// ?
    pub miner: H256,
    /// State root hash
    #[serde(rename = "stateRoot")]
    pub state_root: H256,
    /// Transactions root hash
    #[serde(rename = "transactionsRoot")]
    pub transactions_root: H256,
    /// Transactions receipts root hash
    #[serde(rename = "receiptsRoot")]
    pub receipts_root: H256,
    /// Block number
    pub number: Option<U256>,
    /// Gas Used
    #[serde(rename = "gasUsed")]
    pub gas_used: U256,
    /// Gas Limit
    #[serde(rename = "gasLimit")]
    pub gas_limit: U256,
    /// Extra data
    #[serde(rename = "extraData")]
    pub extra_data: Bytes,
    /// Logs bloom
    #[serde(rename = "logsBloom")]
    pub logs_bloom: Bloom,
    /// Timestamp
    pub timestamp: U256,
    /// Difficulty
    pub difficulty: U256,
    /// nonce
    pub nonce: Option<Bytes>,
    /// solution
    pub solution: Option<Bytes>,
    /// Size in bytes
    pub size: Option<U256>,
}

impl From<EthHeader> for Header {
    fn from(h: EthHeader) -> Self { (&h).into() }
}

impl<'a> From<&'a EthHeader> for Header {
    fn from(h: &'a EthHeader) -> Self {
        let seal_fields: Vec<Bytes> = h.view().seal().into_iter().map(Into::into).collect();
        // Pending block do not yet has nonce and solution. Return empty value in this case.
        let (nonce, solution) = match seal_fields.len() {
            length if length >= 2 => (Some(seal_fields[0].clone()), Some(seal_fields[1].clone())),
            _ => (None, None),
        };
        Header {
            hash: Some(h.hash()),
            size: Some(h.rlp().as_raw().len().into()),
            parent_hash: h.parent_hash(),
            miner: h.author(),
            state_root: h.state_root(),
            transactions_root: h.transactions_root(),
            receipts_root: h.receipts_root(),
            number: Some(h.number().into()),
            gas_used: h.gas_used(),
            gas_limit: h.gas_limit(),
            logs_bloom: h.log_bloom(),
            timestamp: h.timestamp().into(),
            difficulty: h.difficulty(),
            extra_data: h.extra_data().into(),
            nonce: nonce,
            solution: solution,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json;
    use ethbloom::Bloom;
    use aion_types::{H256, U256};
    use types::{Transaction, Bytes};
    use super::{Block, BlockTransactions, Header};

    #[test]
    fn test_serialize_block_transactions() {
        let t = BlockTransactions::Full(vec![Transaction::default()]);
        let serialized = serde_json::to_string(&t).unwrap();
        assert_eq!(serialized, r#"[{"hash":"0x0000000000000000000000000000000000000000000000000000000000000000","nonce":0,"blockHash":null,"blockNumber":null,"transactionIndex":null,"to":null,"from":"0x0000000000000000000000000000000000000000000000000000000000000000","value":"0x0","gasPrice":"0x0","gas":0,"nrgPrice":"0x0","nrg":0,"input":"0x","contractAddress":null,"timestamp":0}]"#);

        let t = BlockTransactions::Hashes(vec![H256::default().into()]);
        let serialized = serde_json::to_string(&t).unwrap();
        assert_eq!(
            serialized,
            r#"["0x0000000000000000000000000000000000000000000000000000000000000000"]"#
        );
    }

    #[test]
    fn test_serialize_block() {
        let block = Block {
            hash: Some(H256::default()),
            parent_hash: H256::default(),
            miner: H256::default(),
            state_root: H256::default(),
            transactions_root: H256::default(),
            receipts_root: H256::default(),
            number: Some(0u64),
            gas_used: U256::default(),
            gas_limit: U256::default(),
            extra_data: Bytes::default(),
            logs_bloom: Bloom::default(),
            timestamp: U256::default(),
            difficulty: U256::default(),
            total_difficulty: Some(U256::default()),
            nonce: Some(Bytes::default()),
            solution: Some(Bytes::default()),
            transactions: BlockTransactions::Hashes(vec![].into()),
            size: Some(69.into()),
        };
        let serialized_block = serde_json::to_string(&block).unwrap();

        assert_eq!(serialized_block, r#"{"hash":"0x0000000000000000000000000000000000000000000000000000000000000000","parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000","miner":"0x0000000000000000000000000000000000000000000000000000000000000000","stateRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","transactionsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","receiptsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","number":0,"gasUsed":"0x0","gasLimit":"0x0","extraData":"0x","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","timestamp":"0x0","difficulty":"0x0","totalDifficulty":"0x0","nonce":"0x","solution":"0x","transactions":[],"size":"0x45"}"#);
    }

    #[test]
    fn none_size_null() {
        let block = Block {
            hash: Some(H256::default()),
            parent_hash: H256::default(),
            miner: H256::default(),
            state_root: H256::default(),
            transactions_root: H256::default(),
            receipts_root: H256::default(),
            number: Some(0u64),
            gas_used: U256::default(),
            gas_limit: U256::default(),
            extra_data: Bytes::default(),
            logs_bloom: Bloom::default(),
            timestamp: U256::default(),
            difficulty: U256::default(),
            total_difficulty: Some(U256::default()),
            nonce: Some(Bytes::default()),
            solution: Some(Bytes::default()),
            transactions: BlockTransactions::Hashes(vec![].into()),
            size: None,
        };
        let serialized_block = serde_json::to_string(&block).unwrap();

        assert_eq!(serialized_block, r#"{"hash":"0x0000000000000000000000000000000000000000000000000000000000000000","parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000","miner":"0x0000000000000000000000000000000000000000000000000000000000000000","stateRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","transactionsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","receiptsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","number":0,"gasUsed":"0x0","gasLimit":"0x0","extraData":"0x","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","timestamp":"0x0","difficulty":"0x0","totalDifficulty":"0x0","nonce":"0x","solution":"0x","transactions":[],"size":null}"#);
    }

    #[test]
    fn test_serialize_header() {
        let header = Header {
            hash: Some(H256::default()),
            parent_hash: H256::default(),
            miner: H256::default(),
            state_root: H256::default(),
            transactions_root: H256::default(),
            receipts_root: H256::default(),
            number: Some(U256::default()),
            gas_used: U256::default(),
            gas_limit: U256::default(),
            extra_data: Bytes::default(),
            logs_bloom: Bloom::default(),
            timestamp: U256::default(),
            difficulty: U256::default(),
            nonce: Some(Bytes::default()),
            solution: Some(Bytes::default()),
            size: Some(69.into()),
        };
        let serialized_header = serde_json::to_string(&header).unwrap();

        assert_eq!(serialized_header, r#"{"hash":"0x0000000000000000000000000000000000000000000000000000000000000000","parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000","miner":"0x0000000000000000000000000000000000000000000000000000000000000000","stateRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","transactionsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","receiptsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","number":"0x0","gasUsed":"0x0","gasLimit":"0x0","extraData":"0x","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","timestamp":"0x0","difficulty":"0x0","nonce":"0x","solution":"0x","size":"0x45"}"#);
    }
}
