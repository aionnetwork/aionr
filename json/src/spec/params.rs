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

//! Spec params deserialization.

use uint::{self, Uint};
use hash::Address;

/// Spec params.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Params {
    /// Maximum size of extra data.
    #[serde(rename = "maximumExtraDataSize")]
    pub maximum_extra_data_size: Uint,
    /// Minimum gas limit.
    #[serde(rename = "minGasLimit")]
    pub min_gas_limit: Uint,
    /// See `CommonParams` docs.
    #[serde(rename = "gasLimitBoundDivisor")]
    #[serde(deserialize_with = "uint::validate_non_zero")]
    pub gas_limit_bound_divisor: Uint,
    /// See `CommonParams` docs.
    pub registrar: Option<Address>,
    /// monetary policy update block number.
    #[serde(rename = "monetaryPolicyUpdate")]
    pub monetary_policy_update: Option<Uint>,
    /// Transaction permission contract address.
    #[serde(rename = "transactionPermissionContract")]
    pub transaction_permission_contract: Option<Address>,
    /// Unity update block number.
    #[serde(rename = "unityUpdate")]
    pub unity_update: Option<Uint>,
    /// Unity hybrid seed update block number.
    #[serde(rename = "unityHybridSeedUpdate")]
    pub unity_hybrid_seed_update: Option<Uint>,
    /// Unity ecvrf seed update block number.
    #[serde(rename = "unityECVRFSeedUpdate")]
    pub unity_ecvrf_seed_update: Option<Uint>,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use uint::Uint;
    use aion_types::U256;
    use spec::params::Params;

    #[test]
    fn params_deserialization() {
        let s = r#"{
            "maximumExtraDataSize": "0x20",
            "minGasLimit": "0x1388",
            "gasLimitBoundDivisor": "0x20"
        }"#;

        let deserialized: Params = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.maximum_extra_data_size, Uint(U256::from(0x20)));
        assert_eq!(deserialized.min_gas_limit, Uint(U256::from(0x1388)));
        assert_eq!(deserialized.gas_limit_bound_divisor, Uint(U256::from(0x20)));
    }

    #[test]
    #[should_panic(expected = "a non-zero value")]
    fn test_zero_value_divisor() {
        let s = r#"{
            "maximumExtraDataSize": "0x20",
            "minGasLimit": "0x1388",
            "gasLimitBoundDivisor": "0x0"
        }"#;

        let _deserialized: Params = serde_json::from_str(s).unwrap();
    }
}
