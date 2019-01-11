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

//! Transaction test transaction deserialization.

use uint::Uint;
use bytes::Bytes;
use hash::Address;
use maybe::MaybeEmpty;

/// Transaction test transaction deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Transaction {
    /// Transaction data.
    pub data: Bytes,
    /// Gas limit.
    #[serde(rename = "gasLimit")]
    pub gas_limit: Uint,
    /// Gas price.
    #[serde(rename = "gasPrice")]
    pub gas_price: Uint,
    /// Nonce.
    pub nonce: Uint,
    /// To.
    pub to: MaybeEmpty<Address>,
    /// Value.
    pub value: Uint,
    /// R.
    pub r: Uint,
    /// S.
    pub s: Uint,
    /// V.
    pub v: Uint,
    /// public key.
    pub pk: Uint,
    /// Timestamp.
    pub timestamp: Bytes,
    /// Transaction Type.
    pub transaction_type: u8,
    /// signature
    pub sig: Bytes,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use transaction::Transaction;

    #[test]
    fn transaction_deserialization() {
        let s = r#"{
            "data" : "0x",
            "gasLimit" : "0xf388",
            "gasPrice" : "0x09184e72a000",
            "nonce" : "0x00",
            "r" : "0x2c",
            "s" : "0x04",
            "to" : "",
            "v" : "0x1b",
            "pk" : "0x0",
            "sig" : "0x0",
            "value" : "0x00",
            "timestamp" : "0x56850c2c",
            "transaction_type" : 0
        }"#;
        let _deserialized: Transaction = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
