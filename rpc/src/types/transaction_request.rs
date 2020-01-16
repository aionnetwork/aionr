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

//! `TransactionRequest` type
use std::fmt;

use ansi_term::Colour;
use helpers;
use aion_types::{U256, Address, H256};

use types::{Bytes, TransactionCondition};

/// Transaction request coming from RPC
#[derive(
    Debug,
    Clone,
    Default,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize
)]
#[serde(deny_unknown_fields)]
pub struct TransactionRequest {
    /// Sender
    pub from: Option<Address>,
    /// Recipient
    pub to: Option<Address>,
    /// Gas Price
    #[serde(rename = "gasPrice")]
    pub gas_price: Option<U256>,
    /// Gas
    pub gas: Option<U256>,
    /// Value of transaction in wei
    pub value: Option<U256>,
    /// Additional data sent with transaction
    pub data: Option<Bytes>,
    /// Transaction's nonce
    pub nonce: Option<U256>,
    /// Transaction type
    #[serde(rename = "type")]
    pub tx_type: Option<U256>,
    /// Delay until this block condition.
    pub condition: Option<TransactionCondition>,
    /// Beacon hash
    pub beacon: Option<H256>,
}

pub fn format_ether(i: U256) -> String {
    let mut string = format!("{}", i);
    let idx = string.len() as isize - 18;
    if idx <= 0 {
        let mut prefix = String::from("0.");
        for _ in 0..idx.abs() {
            prefix.push('0');
        }
        string = prefix + &string;
    } else {
        string.insert(idx as usize, '.');
    }
    String::from(string.trim_end_matches('0').trim_end_matches('.'))
}

impl fmt::Display for TransactionRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let eth = self.value.unwrap_or(U256::from(0));
        match self.to {
            Some(ref to) => {
                write!(
                    f,
                    "{} ETH from {} to 0x{:?}",
                    Colour::White.bold().paint(format_ether(eth)),
                    Colour::White.bold().paint(format!("0x{:?}", self.from)),
                    to
                )
            }
            None => {
                write!(
                    f,
                    "{} ETH from {} for contract creation",
                    Colour::White.bold().paint(format_ether(eth)),
                    Colour::White.bold().paint(format!("0x{:?}", self.from)),
                )
            }
        }
    }
}

impl From<helpers::TransactionRequest> for TransactionRequest {
    fn from(r: helpers::TransactionRequest) -> Self {
        TransactionRequest {
            from: r.from,
            to: r.to,
            gas_price: r.gas_price,
            gas: r.gas,
            value: r.value,
            data: r.data.map(Into::into),
            nonce: r.nonce,
            tx_type: r.tx_type,
            condition: r.condition.map(Into::into),
            beacon: r.beacon,
        }
    }
}

impl From<helpers::FilledTransactionRequest> for TransactionRequest {
    fn from(r: helpers::FilledTransactionRequest) -> Self {
        TransactionRequest {
            from: Some(r.from),
            to: r.to,
            gas_price: Some(r.gas_price),
            gas: Some(r.gas),
            value: Some(r.value),
            data: Some(r.data.into()),
            nonce: r.nonce,
            tx_type: Some(r.tx_type),
            condition: r.condition.map(Into::into),
            beacon: r.beacon,
        }
    }
}

impl Into<helpers::TransactionRequest> for TransactionRequest {
    fn into(self) -> helpers::TransactionRequest {
        helpers::TransactionRequest {
            from: self.from,
            to: self.to,
            gas_price: self.gas_price,
            gas: self.gas,
            value: self.value,
            data: self.data.map(Into::into),
            nonce: self.nonce,
            tx_type: self.tx_type,
            condition: self.condition.map(Into::into),
            beacon: self.beacon,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hex::FromHex;
    use serde_json;
    use std::str::FromStr;
    use aion_types::{H256, U256};
    use types::TransactionCondition;

    #[test]
    fn transaction_request_deserialize() {
        let s = r#"{
            "from":"0x0000000000000000000000000000000000000000000000000000000000000001",
            "to":"0x0000000000000000000000000000000000000000000000000000000000000002",
            "gasPrice":"0x1",
            "gas":"0x2",
            "value":"0x3",
            "data":"0x123456",
            "nonce":"0x4",
            "type":"0x01",
            "condition": { "block": 19 }
        }"#;
        let deserialized: TransactionRequest = serde_json::from_str(s).unwrap();

        assert_eq!(
            deserialized,
            TransactionRequest {
                from: Some(H256::from(1)),
                to: Some(H256::from(2)),
                gas_price: Some(U256::from(1)),
                gas: Some(U256::from(2)),
                value: Some(U256::from(3)),
                data: Some(vec![0x12, 0x34, 0x56].into()),
                nonce: Some(U256::from(4)),
                tx_type: Some(U256::from(0x01)),
                condition: Some(TransactionCondition::Number(0x13)),
                beacon: None
            }
        );
    }

    #[test]
    fn transaction_request_deserialize2() {
        let s = r#"{
            "from": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "to": "0x0000000000000000000000000000000000000000000000000000000000000002",
            "gas": "0x76c0",
            "gasPrice": "0x9184e72a000",
            "value": "0x9184e72a",
            "data": "0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675"
        }"#;
        let deserialized: TransactionRequest = serde_json::from_str(s).unwrap();

        assert_eq!(deserialized, TransactionRequest {
            from: Some(H256::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap()),
            to: Some(H256::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap()),
            gas_price: Some(U256::from_str("9184e72a000").unwrap()),
            gas: Some(U256::from_str("76c0").unwrap()),
            value: Some(U256::from_str("9184e72a").unwrap()),
            data: Some("d46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675".from_hex().unwrap().into()),
            nonce: None,
            condition: None,
            tx_type: None,
            beacon: None
        });
    }

    #[test]
    fn transaction_request_deserialize_empty() {
        let s = r#"{"from":"0x0000000000000000000000000000000000000000000000000000000000000001"}"#;
        let deserialized: TransactionRequest = serde_json::from_str(s).unwrap();

        assert_eq!(
            deserialized,
            TransactionRequest {
                from: Some(H256::from(1).into()),
                to: None,
                gas_price: None,
                gas: None,
                value: None,
                data: None,
                nonce: None,
                condition: None,
                tx_type: None,
                beacon: None
            }
        );
    }

    #[test]
    fn transaction_request_deserialize_test() {
        let s = r#"{
            "from":"0x0000000000000000000000000000000000000000000000000000000000000001",
            "to":"0x0000000000000000000000000000000000000000000000000000000000000002",
            "data":"0x8595bab1",
            "gas":"0x2fd618",
            "gasPrice":"0x0ba43b7400"
        }"#;

        let deserialized: TransactionRequest = serde_json::from_str(s).unwrap();

        assert_eq!(
            deserialized,
            TransactionRequest {
                from: Some(
                    H256::from_str(
                        "0000000000000000000000000000000000000000000000000000000000000001"
                    )
                    .unwrap()
                ),
                to: Some(
                    H256::from_str(
                        "0000000000000000000000000000000000000000000000000000000000000002"
                    )
                    .unwrap()
                ),
                gas_price: Some(U256::from_str("0ba43b7400").unwrap()),
                gas: Some(U256::from_str("2fd618").unwrap()),
                value: None,
                data: Some(vec![0x85, 0x95, 0xba, 0xb1].into()),
                nonce: None,
                condition: None,
                tx_type: None,
                beacon: None
            }
        );
    }

    #[test]
    fn transaction_request_deserialize_error() {
        let s = r#"{
            "from":"0x0000000000000000000000000000000000000000000000000000000000000001",
            "to":"",
            "data":"0x8595bab1",
            "gas":"0x2fd618",
            "gasPrice":"0x0ba43b7400"
        }"#;

        let deserialized = serde_json::from_str::<TransactionRequest>(s);

        assert!(deserialized.is_err(), "Should be error because to is empty");
    }

    #[test]
    fn test_format_ether() {
        assert_eq!(&format_ether(U256::from(1000000000000000000u64)), "1");
        assert_eq!(&format_ether(U256::from(500000000000000000u64)), "0.5");
        assert_eq!(&format_ether(U256::from(50000000000000000u64)), "0.05");
        assert_eq!(&format_ether(U256::from(5000000000000000u64)), "0.005");
        assert_eq!(&format_ether(U256::from(2000000000000000000u64)), "2");
        assert_eq!(&format_ether(U256::from(2500000000000000000u64)), "2.5");
        assert_eq!(&format_ether(U256::from(10000000000000000000u64)), "10");
    }
}
