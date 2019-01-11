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

//! Log entry type definition.

use std::ops::Deref;
use heapsize::HeapSizeOf;
use bytes::Bytes;
use aion_types::{H256, Address};
use ethbloom::{Bloom, Input as BloomInput};

use {BlockNumber};
use ajson;

/// A record of execution for a `LOG` operation.
#[derive(
    Default,
    Debug,
    Clone,
    PartialEq,
    Eq,
    RlpEncodable,
    RlpDecodable
)]
pub struct LogEntry {
    /// The address of the contract executing at the point of the `LOG` operation.
    pub address: Address,
    /// The topics associated with the `LOG` operation.
    pub topics: Vec<H256>,
    /// The data associated with the `LOG` operation.
    pub data: Bytes,
}

impl HeapSizeOf for LogEntry {
    fn heap_size_of_children(&self) -> usize {
        self.topics.heap_size_of_children() + self.data.heap_size_of_children()
    }
}

impl LogEntry {
    /// Calculates the bloom of this log entry.
    pub fn bloom(&self) -> Bloom {
        self.topics
            .iter()
            .fold(Bloom::from(BloomInput::Raw(&self.address)), |mut b, t| {
                b.accrue(BloomInput::Raw(t));
                b
            })
    }
}

impl From<ajson::state::Log> for LogEntry {
    fn from(l: ajson::state::Log) -> Self {
        LogEntry {
            address: l.address.into(),
            topics: l.topics.into_iter().map(Into::into).collect(),
            data: l.data.into(),
        }
    }
}

/// Log localized in a blockchain.
#[derive(Default, Debug, PartialEq, Clone)]
pub struct LocalizedLogEntry {
    /// Plain log entry.
    pub entry: LogEntry,
    /// Block in which this log was created.
    pub block_hash: H256,
    /// Block number.
    pub block_number: BlockNumber,
    /// Hash of transaction in which this log was created.
    pub transaction_hash: H256,
    /// Index of transaction within block.
    pub transaction_index: usize,
    /// Log position in the block.
    pub log_index: usize,
    /// Log position in the transaction.
    pub transaction_log_index: usize,
}

impl Deref for LocalizedLogEntry {
    type Target = LogEntry;

    fn deref(&self) -> &Self::Target { &self.entry }
}

#[cfg(test)]
mod tests {
    use aion_types::Address;
    use ethbloom::Bloom;
    use super::LogEntry;

    #[test]
    fn test_empty_log_bloom() {
        let bloom = "00000000000000000000000008000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000040"
            .parse::<Bloom>()
            .unwrap();
        let address = "a00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2c"
            .parse::<Address>()
            .unwrap();
        let log = LogEntry {
            address: address,
            topics: vec!["ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9".into()],
            data: vec![],
        };
        assert_eq!(log.bloom(), bloom);
    }
}
