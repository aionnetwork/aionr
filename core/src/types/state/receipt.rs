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

//! Receipt

use aion_types::{H256, U256, Address};
use ethbloom::Bloom;
use heapsize::HeapSizeOf;
use rlp::*;
use acore_bytes::Bytes;

use types::BlockNumber;
use log_entry::{LogEntry, LocalizedLogEntry};

/// Simple information describing execution of a transaction for syncing
/// use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleReceipt {
    /// The OR-wide combination of all logs' blooms for this transaction.
    pub log_bloom: Bloom,
    /// The logs stemming from this transaction.
    pub logs: Vec<LogEntry>,
    /// State Root
    pub state_root: H256,
}

impl SimpleReceipt {
    /// Create a new receipt.
    pub fn new(state_root: H256, logs: Vec<LogEntry>) -> SimpleReceipt {
        SimpleReceipt {
            log_bloom: logs.iter().fold(Bloom::default(), |mut b, l| {
                b = &b | &l.bloom();
                b
            }), //TODO: use |= operator
            logs: logs,
            state_root: state_root,
        }
    }
}

impl Encodable for SimpleReceipt {
    fn rlp_append(&self, s: &mut RlpStream) {
        // [FZH] Java receipt also includes error message. To discuss if we do this or not.
        s.begin_list(3);
        s.append(&self.state_root);
        s.append(&self.log_bloom);
        s.append_list(&self.logs);
    }
}

impl Decodable for SimpleReceipt {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        Ok(SimpleReceipt {
            state_root: rlp.val_at(0)?,
            log_bloom: rlp.val_at(1)?,
            logs: rlp.list_at(2)?,
        })
    }
}

impl HeapSizeOf for SimpleReceipt {
    fn heap_size_of_children(&self) -> usize { self.logs.heap_size_of_children() }
}

/// Information describing execution of a transaction, including more information
/// for storage use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    /// Simple receipt
    pub simple_receipt: SimpleReceipt,
    /// The output of the applied transaction.
    pub output: Bytes,
    /// The total gas used in the block following execution of the transaction.
    pub gas_used: U256,
    /// error message
    pub error_message: String,
    /// transaction fee
    pub transaction_fee: U256,
}

impl Receipt {
    /// Create a new receipt.
    pub fn new(
        state_root: H256,
        gas_used: U256,
        transaction_fee: U256,
        logs: Vec<LogEntry>,
        output: Bytes,
        error_message: String,
    ) -> Receipt
    {
        Receipt {
            simple_receipt: SimpleReceipt {
                log_bloom: logs.iter().fold(Bloom::default(), |mut b, l| {
                    b = &b | &l.bloom();
                    b
                }), //TODO: use |= operator
                logs: logs,
                state_root: state_root,
            },
            output: output,
            gas_used: gas_used,
            error_message: error_message,
            transaction_fee: transaction_fee,
        }
    }

    /// Gzet simple receipt
    pub fn simple_receipt(&self) -> &SimpleReceipt { &self.simple_receipt }

    /// Get log bloom
    pub fn log_bloom(&self) -> &Bloom { &self.simple_receipt.log_bloom }

    /// Get logs
    pub fn logs(&self) -> &Vec<LogEntry> { &self.simple_receipt.logs }

    /// Get state root
    pub fn state_root(&self) -> &H256 { &self.simple_receipt.state_root }
}

impl Encodable for Receipt {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(6);
        s.append(&self.simple_receipt.state_root);
        s.append(&self.simple_receipt.log_bloom);
        s.append_list(&self.simple_receipt.logs);
        s.append(&self.output);
        s.append(&self.gas_used);
        s.append(&self.error_message);
    }
}

impl Decodable for Receipt {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        Ok(Receipt {
            simple_receipt: SimpleReceipt {
                state_root: rlp.val_at(0)?,
                log_bloom: rlp.val_at(1)?,
                logs: rlp.list_at(2)?,
            },
            output: rlp.val_at(3)?,
            gas_used: rlp.val_at(4)?,
            error_message: rlp.val_at(5)?,
            transaction_fee: U256::default(),
        })
    }
}

impl HeapSizeOf for Receipt {
    fn heap_size_of_children(&self) -> usize { self.simple_receipt.logs.heap_size_of_children() }
}

/// Receipt with additional info.
// RichReceipt is only used in pending receipts.
#[derive(Debug, Clone, PartialEq)]
pub struct RichReceipt {
    /// Transaction hash.
    pub transaction_hash: H256,
    /// Transaction index.
    pub transaction_index: usize,
    /// The total gas used in the block following execution of the transaction.
    pub cumulative_gas_used: U256,
    /// The gas used in the execution of the transaction. Note the difference of meaning to `Receipt::gas_used`.
    pub gas_used: U256,
    /// Contract address.
    pub contract_address: Option<Address>,
    /// Logs
    pub logs: Vec<LogEntry>,
    /// Logs bloom
    pub log_bloom: Bloom,
    /// State Root
    pub state_root: H256,
}

/// Receipt with additional info.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalizedReceipt {
    /// Transaction hash.
    pub transaction_hash: H256,
    /// Transaction index.
    pub transaction_index: usize,
    /// Block hash.
    pub block_hash: H256,
    /// Block number.
    pub block_number: BlockNumber,
    /// The total gas used in the block following execution of the transaction.
    pub cumulative_gas_used: U256,
    /// The gas used in the execution of the transaction. Note the difference of meaning to `Receipt::gas_used`.
    pub gas_used: U256,
    /// Contract address.
    pub contract_address: Option<Address>,
    /// Logs
    pub logs: Vec<LocalizedLogEntry>,
    /// Logs bloom
    pub log_bloom: Bloom,
    /// state root
    pub state_root: H256,
    /// gas price
    pub gas_price: U256,
    /// gas limit
    pub gas_limit: U256,
    /// from address
    pub from: Option<Address>,
    /// to address
    pub to: Option<Address>,
    /// output
    pub output: Bytes,
    /// error message
    pub error_message: String,
}
