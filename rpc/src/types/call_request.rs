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

use helpers::CallRequest as Request;
use types::{Bytes, H256, U256};

/// Call request
#[derive(Debug, Default, PartialEq, Deserialize)]
pub struct CallRequest {
    /// From
    pub from: Option<H256>,
    /// To
    pub to: Option<H256>,
    /// Gas Price
    #[serde(rename = "gasPrice")]
    pub gas_price: Option<U256>,
    /// Gas
    pub gas: Option<U256>,
    /// Value
    pub value: Option<U256>,
    /// Data
    pub data: Option<Bytes>,
    /// Nonce
    pub nonce: Option<U256>,
}

impl Into<Request> for CallRequest {
    fn into(self) -> Request {
        Request {
            from: self.from.map(Into::into),
            to: self.to.map(Into::into),
            gas_price: self.gas_price.map(Into::into),
            gas: self.gas.map(Into::into),
            value: self.value.map(Into::into),
            data: self.data.map(Into::into),
            nonce: self.nonce.map(Into::into),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use rustc_hex::FromHex;
    use serde_json;
    use types::{U256, H256};
    use super::CallRequest;

    #[test]
    fn call_request_deserialize() {
        let s = r#"{
            "from":"0x0000000000000000000000000000000000000000000000000000000000000001",
            "to":"0x0000000000000000000000000000000000000000000000000000000000000002",
            "gasPrice":"0x1",
            "gas":"0x2",
            "value":"0x3",
            "data":"0x123456",
            "nonce":"0x4"
        }"#;
        let deserialized: CallRequest = serde_json::from_str(s).unwrap();

        assert_eq!(
            deserialized,
            CallRequest {
                from: Some(H256::from(1)),
                to: Some(H256::from(2)),
                gas_price: Some(U256::from(1)),
                gas: Some(U256::from(2)),
                value: Some(U256::from(3)),
                data: Some(vec![0x12, 0x34, 0x56].into()),
                nonce: Some(U256::from(4)),
            }
        );
    }

    #[test]
    fn call_request_deserialize2() {
        let s = r#"{
            "from": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "to": "0x0000000000000000000000000000000000000000000000000000000000000002",
            "gas": "0x76c0",
            "gasPrice": "0x9184e72a000",
            "value": "0x9184e72a",
            "data": "0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675"
        }"#;
        let deserialized: CallRequest = serde_json::from_str(s).unwrap();

        assert_eq!(deserialized, CallRequest {
            from: Some(H256::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap()),
            to: Some(H256::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap()),
            gas_price: Some(U256::from_str("9184e72a000").unwrap()),
            gas: Some(U256::from_str("76c0").unwrap()),
            value: Some(U256::from_str("9184e72a").unwrap()),
            data: Some("d46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675".from_hex().unwrap().into()),
            nonce: None
        });
    }

    #[test]
    fn call_request_deserialize_empty() {
        let s = r#"{"from":"0x0000000000000000000000000000000000000000000000000000000000000001"}"#;
        let deserialized: CallRequest = serde_json::from_str(s).unwrap();

        assert_eq!(
            deserialized,
            CallRequest {
                from: Some(H256::from(1)),
                to: None,
                gas_price: None,
                gas: None,
                value: None,
                data: None,
                nonce: None,
            }
        );
    }
}
