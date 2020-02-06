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

//! General test deserialization.

use std::io::Read;
use std::collections::BTreeMap;
use uint::Uint;
use bytes::Bytes;
use hash::{Address, H256};
use state::{Env, AccountState, Transaction};
use maybe::MaybeEmpty;
use serde_json::{self, Error};

/// State test deserializer.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Test(BTreeMap<String, State>);

impl IntoIterator for Test {
    type Item = <BTreeMap<String, State> as IntoIterator>::Item;
    type IntoIter = <BTreeMap<String, State> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}

impl Test {
    /// Loads test from json.
    pub fn load<R>(reader: R) -> Result<Self, Error>
    where R: Read {
        serde_json::from_reader(reader)
    }
}

/// State test deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct State {
    /// Environment.
    pub env: Env,
    /// Pre state.
    #[serde(rename = "pre")]
    pub pre_state: AccountState,
    /// Post state.
    #[serde(rename = "post")]
    pub post_states: BTreeMap<ForkSpec, Vec<PostStateResult>>,
    /// Transaction.
    pub transaction: MultiTransaction,
}

/// State test transaction deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct MultiTransaction {
    /// Transaction data set.
    pub data: Vec<Bytes>,
    /// Timestamp.
    // for simplicity, use same timestamp.
    pub timestamp: Bytes,
    /// Gas limit set.
    #[serde(rename = "gasLimit")]
    pub gas_limit: Vec<Uint>,
    /// Gas price.
    #[serde(rename = "gasPrice")]
    pub gas_price: Uint,
    /// Nonce.
    pub nonce: Uint,
    /// Secret key.
    #[serde(rename = "secretKey")]
    pub secret: Option<H256>,
    /// To.
    pub to: MaybeEmpty<Address>,
    /// Value set.
    pub value: Vec<Uint>,
}

impl MultiTransaction {
    /// Build transaction with given indexes.
    pub fn select(&self, indexes: &PostStateIndexes) -> Transaction {
        Transaction {
            data: self.data[indexes.data as usize].clone(),
            timestamp: self.timestamp.clone(),
            gas_limit: self.gas_limit[indexes.gas as usize].clone(),
            gas_price: self.gas_price.clone(),
            nonce: self.nonce.clone(),
            secret: self.secret.clone(),
            to: self.to.clone(),
            value: self.value[indexes.value as usize].clone(),
            transaction_type: 0x01,
        }
    }
}

/// State test transaction deserialization.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum ForkSpec {
    EIP150,
    EIP158,
    Constantinople,
    FrontierToHomesteadAt5,
    HomesteadToDaoAt5,
    HomesteadToEIP150At5,
}

/// State test indexes deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct PostStateIndexes {
    /// Index into transaction data set.
    pub data: u64,
    /// Index into transaction gas limit set.
    pub gas: u64,
    /// Index into transaction value set.
    pub value: u64,
}

/// State test indexed state result deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct PostStateResult {
    /// Post state hash
    pub hash: H256,
    /// Indexes
    pub indexes: PostStateIndexes,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use super::{MultiTransaction, State};

    #[test]
    fn multi_transaction_deserialization() {
        let s = r#"{
            "data" : [ "" ],
            "gasLimit" : [ "0x2dc6c0", "0x222222" ],
            "gasPrice" : "0x01",
            "nonce" : "0x00",
            "secretKey" : "45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8",
            "to" : "0000000000000000000000000000000000000000000000000000000000000002",
            "value" : [ "0x00", "0x01", "0x02" ],
            "timestamp" : "0x54c98c81"
        }"#;
        let _deserialized: MultiTransaction = serde_json::from_str(s).unwrap();
    }

    #[test]
    fn state_deserialization() {
        let s = r#"{
            "env" : {
                "currentCoinbase" : "0000000000000000000000000000000000000000000000000000000000000001",
                "currentDifficulty" : "0x0100",
                "currentGasLimit" : "0x01c9c380",
                "currentNumber" : "0x00",
                "currentTimestamp" : "0x01",
                "previousHash" : "5e20a0453cecd065ea59c37ac63e079ee08998b6045136a8ce6635c7912ec0b6"
            },
            "post" : {
                "EIP150" : [
                    {
                        "hash" : "3e6dacc1575c6a8c76422255eca03529bbf4c0dda75dfc110b22d6dc4152396f",
                        "indexes" : { "data" : 0, "gas" : 0,  "value" : 0 }
                    },
                    {
                        "hash" : "99a450d8ce5b987a71346d8a0a1203711f770745c7ef326912e46761f14cd764",
                        "indexes" : { "data" : 0, "gas" : 0,  "value" : 1 }
                    }
                ],
                "EIP158" : [
                    {
                        "hash" : "3e6dacc1575c6a8c76422255eca03529bbf4c0dda75dfc110b22d6dc4152396f",
                        "indexes" : { "data" : 0,   "gas" : 0,  "value" : 0 }
                    },
                    {
                        "hash" : "99a450d8ce5b987a71346d8a0a1203711f770745c7ef326912e46761f14cd764",
                        "indexes" : { "data" : 0,   "gas" : 0,  "value" : 1  }
                    }
                ]
            },
            "pre" : {
                "0000000000000000000000000000000000000000000000000000000000000001" : {
                    "balance" : "0x0de0b6b3a7640000",
                    "code" : "0x6040600060406000600173100000000000000000000000000000000000000162055730f1600055",
                    "nonce" : "0x00",
                    "storage" : {
                    }
                },
                "0000000000000000000000000000000000000000000000000000000000000002" : {
                    "balance" : "0x0de0b6b3a7640000",
                    "code" : "0x604060006040600060027310000000000000000000000000000000000000026203d090f1600155",
                    "nonce" : "0x00",
                    "storage" : {
                    }
                },
                "0000000000000000000000000000000000000000000000000000000000000003" : {
                    "balance" : "0x00",
                    "code" : "0x600160025533600455346007553060e6553260e8553660ec553860ee553a60f055",
                    "nonce" : "0x00",
                    "storage" : {
                    }
                },
                "0000000000000000000000000000000000000000000000000000000000000004" : {
                    "balance" : "0x0de0b6b3a7640000",
                    "code" : "0x",
                    "nonce" : "0x00",
                    "storage" : {
                    }
                }
            },
            "transaction" : {
                "data" : [ "" ],
                "gasLimit" : [ "285000",   "100000",  "6000" ],
                "gasPrice" : "0x01",
                "nonce" : "0x00",
                "secretKey" : "45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8",
                "to" : "0000000000000000000000000000000000000000000000000000000000000002",
                "value" : [   "10",   "0" ],
                "timestamp" : "0x56850c2c"
            }
        }"#;
        let _deserialized: State = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
