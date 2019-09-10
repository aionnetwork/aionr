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

//use std::collections::BTreeMap;
use serde::{Serialize, Serializer};
//use aion_types::{U256, H512};

/// Sync info
#[derive(Default, Debug, Serialize, PartialEq)]
pub struct SyncInfo {
    /// Starting block, hex representation
    #[serde(rename = "startingBlock")]
    pub starting_block: String,
    /// Current block, hex representation
    #[serde(rename = "currentBlock")]
    pub current_block: String,
    /// Highest block seen so far, hex representation
    #[serde(rename = "highestBlock")]
    pub highest_block: String,
}
//
///// Peers info
//#[derive(Default, Debug, Serialize)]
//pub struct Peers {
//    /// Number of active peers
//    pub active: usize,
//    /// Number of connected peers
//    pub connected: usize,
//    /// Max number of peers
//    pub max: u32,
//    /// Detailed information on peers
//    pub peers: Vec<PeerInfo>,
//}
//
///// Peer connection information
//#[derive(Default, Debug, Serialize)]
//pub struct PeerInfo {
//    /// Public node id
//    pub id: Option<String>,
//    /// Node client ID
//    pub name: String,
//    /// Capabilities
//    pub caps: Vec<String>,
//    /// Network information
//    pub network: PeerNetworkInfo,
//}
//
///// Peer network information
//#[derive(Default, Debug, Serialize)]
//pub struct PeerNetworkInfo {
//    /// Remote endpoint address
//    #[serde(rename = "remoteAddress")]
//    pub remote_address: String,
//    /// Local endpoint address
//    #[serde(rename = "localAddress")]
//    pub local_address: String,
//}

/// Sync status
#[derive(Debug, PartialEq)]
pub enum SyncStatus {
    /// Info when syncing
    Info(SyncInfo),
    /// Not syncing
    None,
}

///// Active peer infomation
//#[derive(Default, Debug, Serialize)]
//pub struct AcitvePeerInfo {
//    /// Best block number
//    pub highest_block_number: u64,
//    /// node id
//    pub id: String,
//    /// remote ip
//    pub ip: String,
//}
//
/////sync info use by pb
//pub struct PbSyncInfo {
//    /// is syncing
//    pub syncing: bool,
//    /// chain best block number
//    pub chain_best_number: u64,
//    /// network best block number
//    pub network_best_number: u64,
//    /// starting block
//    pub starting_block: u64,
//    /// max import block
//    pub max_import_block: u32,
//}

impl Serialize for SyncStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        match *self {
            SyncStatus::Info(ref info) => info.serialize(serializer),
            SyncStatus::None => false.serialize(serializer),
        }
    }
}

///// Propagation statistics for pending transaction.
//#[derive(Default, Debug, Serialize)]
//pub struct TransactionStats {
//    /// Block no this transaction was first seen.
//    #[serde(rename = "firstSeen")]
//    pub first_seen: u64,
//    /// Peers this transaction was propagated to with count.
//    #[serde(rename = "propagatedTo")]
//    pub propagated_to: BTreeMap<H512, usize>,
//}
//
///// Chain status.
//#[derive(Default, Debug, Serialize)]
//pub struct ChainStatus {
//    /// Describes the gap in the blockchain, if there is one: (first, last)
//    #[serde(rename = "blockGap")]
//    pub block_gap: Option<(U256, U256)>,
//}

#[cfg(test)]
mod tests {
    use serde_json;
    //    use std::collections::BTreeMap;
    use super::{SyncInfo, SyncStatus, /*Peers, TransactionStats, ChainStatus*/
};

    #[test]
    fn test_serialize_sync_info() {
        let t = SyncInfo::default();
        let serialized = serde_json::to_string(&t).unwrap();
        assert_eq!(
            serialized,
            r#"{"startingBlock":"","currentBlock":"","highestBlock":""}"#
        );
    }

    //    #[test]
    //    fn test_serialize_peers() {
    //        let t = Peers::default();
    //        let serialized = serde_json::to_string(&t).unwrap();
    //        assert_eq!(
    //            serialized,
    //            r#"{"active":0,"connected":0,"max":0,"peers":[]}"#
    //        );
    //    }

    #[test]
    fn test_serialize_sync_status() {
        let t = SyncStatus::None;
        let serialized = serde_json::to_string(&t).unwrap();
        assert_eq!(serialized, "false");

        let t = SyncStatus::Info(SyncInfo::default());
        let serialized = serde_json::to_string(&t).unwrap();
        assert_eq!(
            serialized,
            r#"{"startingBlock":"","currentBlock":"","highestBlock":""}"#
        );
    }
    //
    //    #[test]
    //    fn test_serialize_block_gap() {
    //        let mut t = ChainStatus::default();
    //        let serialized = serde_json::to_string(&t).unwrap();
    //        assert_eq!(serialized, r#"{"blockGap":null}"#);
    //
    //        t.block_gap = Some((1.into(), 5.into()));
    //
    //        let serialized = serde_json::to_string(&t).unwrap();
    //        assert_eq!(serialized, r#"{"blockGap":["0x1","0x5"]}"#);
    //    }
    //
    //    #[test]
    //    fn test_serialize_transaction_stats() {
    //        let stats = TransactionStats {
    //            first_seen: 100,
    //            propagated_to: map![
    //                10.into() => 50
    //            ],
    //        };
    //
    //        let serialized = serde_json::to_string(&stats).unwrap();
    //        assert_eq!(serialized, r#"{"firstSeen":100,"propagatedTo":{"0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a":50}}"#)
    //    }
}
