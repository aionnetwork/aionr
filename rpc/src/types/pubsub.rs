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

//! Pub-Sub types.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::Error;
use serde_json::{Value, from_value};
use types::{Header, Filter, Log, H256};

/// Subscription result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Result {
    /// New block header.
    Header(Header),
    /// Log
    Log(Log),
    /// Transaction hash
    TransactionHash(H256),
}

impl Serialize for Result {
    fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
    where S: Serializer {
        match *self {
            Result::Header(ref header) => header.serialize(serializer),
            Result::Log(ref log) => log.serialize(serializer),
            Result::TransactionHash(ref hash) => hash.serialize(serializer),
        }
    }
}

/// Subscription kind.
#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(deny_unknown_fields)]
pub enum Kind {
    /// New block headers subscription.
    #[serde(rename = "newHeads")]
    NewHeads,
    /// Logs subscription.
    #[serde(rename = "logs")]
    Logs,
    /// New Pending Transactions subscription.
    #[serde(rename = "newPendingTransactions")]
    NewPendingTransactions,
    /// Node syncing status subscription.
    #[serde(rename = "syncing")]
    Syncing,
}

/// Subscription kind.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Params {
    /// No parameters passed.
    None,
    /// Log parameters.
    Logs(Filter),
}

impl Default for Params {
    fn default() -> Self { Params::None }
}

impl<'a> Deserialize<'a> for Params {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Params, D::Error>
    where D: Deserializer<'a> {
        let v: Value = Deserialize::deserialize(deserializer)?;

        if v.is_null() {
            return Ok(Params::None);
        }

        from_value(v.clone())
            .map(Params::Logs)
            .map_err(|e| D::Error::custom(format!("Invalid Pub-Sub parameters: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use serde_json;
    use super::{Result, Kind, Params};
    use types::{Header, Filter};
    use types::filter::VariadicValue;

    #[test]
    fn should_deserialize_kind() {
        assert_eq!(
            serde_json::from_str::<Kind>(r#""newHeads""#).unwrap(),
            Kind::NewHeads
        );
        assert_eq!(
            serde_json::from_str::<Kind>(r#""logs""#).unwrap(),
            Kind::Logs
        );
        assert_eq!(
            serde_json::from_str::<Kind>(r#""newPendingTransactions""#).unwrap(),
            Kind::NewPendingTransactions
        );
        assert_eq!(
            serde_json::from_str::<Kind>(r#""syncing""#).unwrap(),
            Kind::Syncing
        );
    }

    #[test]
    fn should_deserialize_logs() {
        let none = serde_json::from_str::<Params>(r#"null"#).unwrap();
        assert_eq!(none, Params::None);

        let logs1 = serde_json::from_str::<Params>(r#"{}"#).unwrap();
        let logs2 = serde_json::from_str::<Params>(r#"{"limit":10}"#).unwrap();
        let logs3 = serde_json::from_str::<Params>(
            r#"{"topics":["0x000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b"]}"#,
        )
        .unwrap();
        assert_eq!(
            logs1,
            Params::Logs(Filter {
                from_block: None,
                to_block: None,
                address: None,
                topics: None,
                limit: None,
            })
        );
        assert_eq!(
            logs2,
            Params::Logs(Filter {
                from_block: None,
                to_block: None,
                address: None,
                topics: None,
                limit: Some(10),
            })
        );
        assert_eq!(
            logs3,
            Params::Logs(Filter {
                from_block: None,
                to_block: None,
                address: None,
                topics: Some(vec![VariadicValue::Single(
                    "000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b"
                        .parse()
                        .unwrap(),
                )]),
                limit: None,
            })
        );
    }

    #[test]
    fn should_serialize_header() {
        let header = Result::Header(Header {
            hash: Some(Default::default()),
            parent_hash: Default::default(),
            miner: Default::default(),
            state_root: Default::default(),
            transactions_root: Default::default(),
            receipts_root: Default::default(),
            number: Some(Default::default()),
            gas_used: Default::default(),
            gas_limit: Default::default(),
            extra_data: Default::default(),
            logs_bloom: Default::default(),
            timestamp: Default::default(),
            difficulty: Default::default(),
            nonce: Some(Default::default()),
            solution: Some(Default::default()),
            size: Some(69.into()),
        });
        let expected = r#"{"hash":"0x0000000000000000000000000000000000000000000000000000000000000000","parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000","miner":"0x0000000000000000000000000000000000000000000000000000000000000000","stateRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","transactionsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","receiptsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","number":"0x0","gasUsed":"0x0","gasLimit":"0x0","extraData":"0x","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","timestamp":"0x0","difficulty":"0x0","nonce":"0x","solution":"0x","size":"0x45"}"#;
        assert_eq!(serde_json::to_string(&header).unwrap(), expected);
    }
}
