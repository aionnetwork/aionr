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

//! Blockchain test account deserializer.

use std::collections::BTreeMap;
use uint::Uint;
use bytes::Bytes;

/// Blockchain test account deserializer.
#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct Account {
    /// Balance.
    pub balance: Uint,
    /// Code.
    pub code: Bytes,
    /// Nonce.
    pub nonce: Uint,
    /// Storage.
    pub storage: BTreeMap<Uint, Uint>,
    /// Double word Storage.
    pub storage_dword: Option<BTreeMap<Uint, Uint>>,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use blockchain::account::Account;

    #[test]
    fn account_deserialization() {
        let s = r#"{
            "balance" : "0x09184e72a078",
            "code" : "0x600140600155",
            "nonce" : "0x00",
            "storage" : {
                "0x01" : "0x9a10c2b5bb8f3c602e674006d9b21f09167df57c87a78a5ce96d4159ecb76520"
            }
        }"#;
        let _deserialized: Account = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
