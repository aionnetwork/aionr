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

use acore::header::Header;
use rustc_hex::ToHex;
use aion_types::clean_0x;

#[derive(Default, Debug, Serialize)]
pub struct StratumHeader {
    /// result code
    pub code: i32,
    /// header nonce
    pub nonce: Option<String>,
    /// header solution
    pub solution: Option<String>,
    /// header mined hash
    #[serde(rename = "headerHash")]
    pub header_hash: Option<String>,
    /// header structure
    #[serde(rename = "blockHeader")]
    pub block_header: Option<SimpleHeader>,
    /// result message
    pub message: Option<String>,
}

#[derive(Default, Debug, Serialize)]
pub struct SimpleHeader {
    /// seal type
    #[serde(rename = "sealType")]
    pub seal_type: String,
    /// block number
    pub number: String,
    /// parent hash
    #[serde(rename = "parentHash")]
    pub parent_hash: String,
    /// author
    #[serde(rename = "coinBase")]
    pub coin_base: String,
    /// state root
    #[serde(rename = "stateRoot")]
    pub state_root: String,
    /// transaction trie root
    #[serde(rename = "txTrieRoot")]
    pub tx_trie_root: String,
    /// receipt trie root
    #[serde(rename = "receiptTrieRoot")]
    pub receipt_trie_root: String,
    /// logs bloom
    #[serde(rename = "logsBloom")]
    pub logs_bloom: String,
    /// difficulty
    pub difficulty: String,
    /// extra data
    #[serde(rename = "extraData")]
    pub extra_data: String,
    /// gas used
    #[serde(rename = "energyConsumed")]
    pub energy_consumed: String,
    /// gas limit
    #[serde(rename = "energyLimit")]
    pub energy_limit: String,
    /// timestamp
    pub timestamp: String,
}

impl From<Header> for SimpleHeader {
    fn from(h: Header) -> Self {
        SimpleHeader {
            seal_type: format!("{}", h.seal_type().to_owned().unwrap_or_default()),
            number: format!("{:x}", h.number()),
            parent_hash: clean_0x(&format!("{:?}", h.parent_hash())).to_owned(),
            coin_base: clean_0x(&format!("{:?}", h.author())).to_owned(),
            state_root: clean_0x(&format!("{:?}", h.state_root())).to_owned(),
            tx_trie_root: clean_0x(&format!("{:?}", h.transactions_root())).to_owned(),
            receipt_trie_root: clean_0x(&format!("{:?}", h.receipts_root())).to_owned(),
            logs_bloom: clean_0x(&format!("{:?}", h.log_bloom())).to_owned(),
            difficulty: clean_0x(&format!("{:x}", h.difficulty())).to_owned(),
            extra_data: h.extra_data().to_hex(),
            energy_consumed: clean_0x(&format!("{:x}", h.gas_used())).to_owned(),
            energy_limit: clean_0x(&format!("{:x}", h.gas_limit())).to_owned(),
            timestamp: format!("{:x}", h.timestamp()),
        }
    }
}
