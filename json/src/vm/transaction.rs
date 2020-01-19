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

//! Executed transaction.
use crate::hash::Address;
use crate::uint::Uint;
use crate::bytes::Bytes;

/// Executed transaction.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Transaction {
    /// Contract address.
    pub address: Address,
    /// Transaction sender.
    #[serde(rename = "caller")]
    pub sender: Address,
    /// Contract code.
    pub code: Bytes,
    /// Input data.
    pub data: Bytes,
    /// Gas.
    pub gas: Uint,
    /// Gas price.
    #[serde(rename = "gasPrice")]
    pub gas_price: Uint,
    /// Transaction origin.
    pub origin: Address,
    /// Sent value.
    pub value: Uint,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use crate::vm::Transaction;

    #[test]
    fn transaction_deserialization() {
        let s = r#"{
            "address" : "0000000000000000000000000000000000000000000000000000000000000001",
            "caller" : "0000000000000000000000000000000000000000000000000000000000000001",
            "code" : "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff01600055",
            "data" : "0x",
            "gas" : "0x0186a0",
            "gasPrice" : "0x5af3107a4000",
            "origin" : "0000000000000000000000000000000000000000000000000000000000000001",
            "value" : "0x0de0b6b3a7640000"
        }"#;
        let _deserialized: Transaction = serde_json::from_str(s).unwrap();
    }
}
