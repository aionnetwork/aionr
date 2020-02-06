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

//! Blockchain test header deserializer.

use hash::{H64, Address, H256, Bloom};
use uint::Uint;
use bytes::Bytes;

/// Blockchain test header deserializer.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Header {
    /// Blocks bloom.
    pub bloom: Bloom,
    /// Blocks author.
    #[serde(rename = "coinbase")]
    pub author: Address,
    /// Difficulty.
    pub difficulty: Uint,
    #[serde(rename = "extraData")]
    /// Extra data.
    pub extra_data: Bytes,
    /// Gas limit.
    #[serde(rename = "gasLimit")]
    pub gas_limit: Uint,
    /// Gas used.
    #[serde(rename = "gasUsed")]
    pub gas_used: Uint,
    /// Hash.
    pub hash: H256,
    #[serde(rename = "mixHash")]
    /// Mix hash.
    pub mix_hash: H256,
    //    pub solution: Bytes,
    /// Seal nonce.
    pub nonce: H64,
    /// Block number.
    pub number: Uint,
    /// Parent hash.
    #[serde(rename = "parentHash")]
    pub parent_hash: H256,
    /// Receipt root.
    #[serde(rename = "receiptTrie")]
    pub receipts_root: H256,
    /// State root.
    #[serde(rename = "stateRoot")]
    pub state_root: H256,
    /// Timestamp.
    pub timestamp: Uint,
    /// Transactions root.
    #[serde(rename = "transactionsTrie")]
    pub transactions_root: H256,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use blockchain::header::Header;

    #[test]
    fn header_deserialization() {
        let s = r#"{
            "bloom" : "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "coinbase" : "0000000000000000000000000000000000000000000000000000000000000001",
            "difficulty" : "0x020000",
            "extraData" : "0x",
            "gasLimit" : "0x2fefba",
            "gasUsed" : "0x00",
            "hash" : "65ebf1b97fb89b14680267e0723d69267ec4bf9a96d4a60ffcb356ae0e81c18f",
            "mixHash" : "13735ab4156c9b36327224d92e1692fab8fc362f8e0f868c94d421848ef7cd06",
            "nonce" : "931dcc53e5edc514",
            "number" : "0x01",
            "parentHash" : "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae",
            "receiptTrie" : "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "stateRoot" : "c5c83ff43741f573a0c9b31d0e56fdd745f4e37d193c4e78544f302777aafcf3",
            "timestamp" : "0x56850b7b",
            "transactionsTrie" : "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "uncleHash" : "1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347"
        }"#;
        let _deserialized: Header = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
