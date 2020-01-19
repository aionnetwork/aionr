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

//! State test transaction deserialization.

use crate::uint::Uint;
use crate::bytes::Bytes;
use crate::hash::{Address, H256};
use crate::maybe::MaybeEmpty;

/// State test transaction deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Transaction {
    /// Transaction data.
    pub data: Bytes,
    /// Timestamp.
    pub timestamp: Bytes,
    /// Type.
    pub transaction_type: u8,
    /// Gas limit.
    #[serde(rename = "gasLimit")]
    pub gas_limit: Uint,
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
    /// Value.
    pub value: Uint,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use crate::state::Transaction;

    #[test]
    fn transaction_deserialization() {
        let s = r#"{
            "data" : "",
            "gasLimit" : "0x2dc6c0",
            "gasPrice" : "0x01",
            "nonce" : "0x00",
            "secretKey" : "45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8",
            "to" : "0000000000000000000000000000000000000000000000000000000000000002",
            "value" : "0x00",
            "timestamp" : "0x56850c2c",
            "transaction_type": 1
        }"#;
        let _deserialized: Transaction = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
