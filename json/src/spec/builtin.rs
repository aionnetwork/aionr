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

//! Spec builtin deserialization.

use hash::Address;
use uint::Uint;

/// Spec builtin.
#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct Builtin {
    /// Builtin name.
    pub name: String,
    /// Activation block.
    pub activate_at: Option<Uint>,
    /// Deactivation block.
    pub deactivate_at: Option<Uint>,
    /// Owner address.
    pub owner_address: Option<Address>,
    /// contract address. if not specified, it's the same with builtin's key.
    pub address: Option<Address>,
}

impl Builtin {
    pub fn set_address(&mut self, address: Address) { self.address = Some(address); }
}

#[cfg(test)]
mod tests {
    use serde_json;
    use spec::builtin::Builtin;
    use uint::Uint;

    #[test]
    fn builtin_deserialization() {
        let s = r#"{
            "name": "ecrecover"
        }"#;
        let deserialized: Builtin = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.name, "ecrecover");
        assert!(deserialized.activate_at.is_none());
    }

    #[test]
    fn activate_at() {
        let s = r#"{
            "name": "late_start",
            "activate_at": 100000
        }"#;

        let deserialized: Builtin = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.name, "late_start");
        assert_eq!(deserialized.activate_at, Some(Uint(100000.into())));
    }
}
