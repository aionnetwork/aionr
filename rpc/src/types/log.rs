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

use acore::log_entry::{LocalizedLogEntry, LogEntry};
use types::{Bytes, H256, U256};
use serde::ser::{Serialize, Serializer, SerializeStruct};

/// Log
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Log {
    /// H256
    pub address: H256,
    /// Topics
    pub topics: Vec<H256>,
    /// Data
    pub data: Bytes,
    /// Block Hash
    pub block_hash: Option<H256>,
    /// Block Number
    pub block_number: Option<U256>,
    /// Transaction Hash
    pub transaction_hash: Option<H256>,
    /// Transaction Index
    pub transaction_index: Option<U256>,
    /// Log Index in Block
    pub log_index: Option<U256>,
    /// Log Index in Transaction
    pub transaction_log_index: Option<U256>,
    /// Log Type
    pub log_type: String,
}

impl Serialize for Log {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let mut log = serializer.serialize_struct("Log", 9)?;
        log.serialize_field("removed", &false)?;
        log.serialize_field("logIndex", &self.log_index)?;
        log.serialize_field("transactionIndex", &self.transaction_index)?;
        log.serialize_field("transactionHash", &self.transaction_hash)?;
        log.serialize_field("blockHash", &self.block_hash)?;
        log.serialize_field("blockNumber", &self.block_number)?;
        log.serialize_field("address", &self.address)?;
        log.serialize_field("data", &self.data)?;
        log.serialize_field("topics", &self.topics)?;
        log.end()
    }
}

impl From<LocalizedLogEntry> for Log {
    fn from(e: LocalizedLogEntry) -> Log {
        Log {
            address: e.entry.address.into(),
            topics: e.entry.topics.into_iter().map(Into::into).collect(),
            data: e.entry.data.into(),
            block_hash: Some(e.block_hash.into()),
            block_number: Some(e.block_number.into()),
            transaction_hash: Some(e.transaction_hash.into()),
            transaction_index: Some(e.transaction_index.into()),
            log_index: Some(e.log_index.into()),
            transaction_log_index: Some(e.transaction_log_index.into()),
            log_type: "mined".to_owned(),
        }
    }
}

impl From<LogEntry> for Log {
    fn from(e: LogEntry) -> Log {
        Log {
            address: e.address.into(),
            topics: e.topics.into_iter().map(Into::into).collect(),
            data: e.data.into(),
            block_hash: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            transaction_log_index: None,
            log_type: "pending".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json;
    use std::str::FromStr;
    use types::{Log, H256, U256};

    #[test]
    fn log_serialization() {
        //        let s = r#"{"address":"0x0000000000000000000000000000000000000000000000000000000000000001","topics":["0xa6697e974e6a320f454390be03f74955e8978f1a6971ea6730542e37b66179bc","0x4861736852656700000000000000000000000000000000000000000000000000"],"data":"0x","blockHash":"0xed76641c68a1c641aee09a94b3b471f4dc0316efe5ac19cf488e2674cf8d05b5","blockNumber":"0x4510c","transactionHash":"0x0000000000000000000000000000000000000000000000000000000000000000","transactionIndex":"0x0","logIndex":"0x1","transactionLogIndex":"0x1","type":"mined"}"#;
        let s = r#"{"removed":false,"logIndex":"0x1","transactionIndex":"0x0","transactionHash":"0x0000000000000000000000000000000000000000000000000000000000000000","blockHash":"0xed76641c68a1c641aee09a94b3b471f4dc0316efe5ac19cf488e2674cf8d05b5","blockNumber":"0x4510c","address":"0x0000000000000000000000000000000000000000000000000000000000000001","data":"0x","topics":["0xa6697e974e6a320f454390be03f74955e8978f1a6971ea6730542e37b66179bc","0x4861736852656700000000000000000000000000000000000000000000000000"]}"#;

        let log = Log {
            address: H256::from_str(
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            topics: vec![
                H256::from_str("a6697e974e6a320f454390be03f74955e8978f1a6971ea6730542e37b66179bc")
                    .unwrap(),
                H256::from_str("4861736852656700000000000000000000000000000000000000000000000000")
                    .unwrap(),
            ],
            data: vec![].into(),
            block_hash: Some(
                H256::from_str("ed76641c68a1c641aee09a94b3b471f4dc0316efe5ac19cf488e2674cf8d05b5")
                    .unwrap(),
            ),
            block_number: Some(U256::from(0x4510c)),
            transaction_hash: Some(H256::default()),
            transaction_index: Some(U256::default()),
            transaction_log_index: Some(1.into()),
            log_index: Some(U256::from(1)),
            log_type: "mined".to_owned(),
        };

        let serialized = serde_json::to_string(&log).unwrap();
        assert_eq!(serialized, s);
    }
}
