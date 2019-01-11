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

//! Transaction test deserialization.

use uint::Uint;
use bytes::Bytes;
use hash::Address;
use transaction::Transaction;

/// Transaction test deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct TransactionTest {
    /// Block number.
    #[serde(rename = "blocknumber")]
    pub block_number: Option<Uint>,
    /// Transaction rlp.
    pub rlp: Bytes,
    /// Transaction sender.
    pub sender: Option<Address>,
    /// Transaction
    pub transaction: Option<Transaction>,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use transaction::TransactionTest;

    #[test]
    fn transaction_deserialization() {
        let s = r#"{
            "blocknumber" : "0",
            "rlp" : "0xf83f800182520894095e7baea6a6c7c4c2dfeb977efac326af552d870b801ba048b55bfa915ac795c431978d8a6a992b628d557da5ff759b307d495a3664935301",
            "sender" : "0000000000000000000000000000000000000000000000000000000000000001",
            "transaction" : {
                "data" : "",
                "gasLimit" : "0x5208",
                "gasPrice" : "0x01",
                "nonce" : "0x00",
                "r" : "0x48b55bfa915ac795c431978d8a6a992b628d557da5ff759b307d495a36649353",
                "s" : "0x01",
                "to" : "0000000000000000000000000000000000000000000000000000000000000002",
                "v" : "0x1b",
                "value" : "0x0b",
                "timestamp" : "0x56850c2c",
                "pk" : "0x0",
                "sig" : "0x0",
                "transaction_type" : 0
            }
        }"#;
        let _deserialized: TransactionTest = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
