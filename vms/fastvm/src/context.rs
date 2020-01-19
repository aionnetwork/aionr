/*******************************************************************************
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

use std::collections::HashSet;

use crate::basetypes::{DataWord, EvmAddress};
use num_bigint::{BigInt};
use aion_types::{Address};

/**
 * A log is emitted by the LOGX vm instruction. It's composed of address, topics
 * and data.
 */
#[derive(Debug, Clone)]
pub struct Log {
    pub addr: EvmAddress,
    pub topics: Vec<Vec<u8>>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct AionInternalTx {
    parent_hash: Vec<u8>,
    deep: usize,
    index: usize,
    from: Address,
    to: Address,
    rejected: bool,
    nonce: BigInt,
    value: Vec<u8>,
    data: Vec<u8>,
    note: String,
}

impl AionInternalTx {
    pub fn new(
        parent_hash: &Vec<u8>,
        deep: usize,
        index: usize,
        from: &Address,
        to: &Address,
        nonce: BigInt,
        value: &Vec<u8>,
        data: &Vec<u8>,
        note: &str,
    ) -> AionInternalTx
    {
        AionInternalTx {
            parent_hash: parent_hash.clone(),
            deep: deep.clone(),
            index: index.clone(),
            rejected: false,
            from: from.clone(),
            to: to.clone(),
            nonce: nonce.clone(),
            value: value.clone(),
            data: data.clone(),
            note: note.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Call {
    data: Vec<u8>,
    destination: Vec<u8>,
    value: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TransactionResult {
    delete_accounts: HashSet<Address>,
    pub internal_txs: Vec<AionInternalTx>,
    pub logs: Vec<Log>,
    calls: Vec<Call>,
}

impl TransactionResult {
    pub fn new() -> Self {
        TransactionResult {
            delete_accounts: HashSet::new(),
            internal_txs: Vec::<AionInternalTx>::new(),
            logs: Vec::new(),
            calls: Vec::new(),
        }
    }

    pub fn add_delete_account(&mut self, _addr: &Address) {
        let evm_addr = _addr.clone();
        self.delete_accounts.insert(evm_addr);
    }

    pub fn add_log(&mut self, log: Log) { self.logs.push(log); }

    pub fn add_internal_transaction(&mut self, tx: AionInternalTx) { self.internal_txs.push(tx); }
}

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub tx_hash: Vec<u8>,

    pub address: Address,
    pub origin: Address,
    pub caller: Address,

    pub nrg_price: DataWord,
    pub nrg_limit: u64,
    pub call_value: DataWord,
    pub call_data: Vec<u8>,

    pub depth: i32,
    pub kind: i32,
    pub flags: i32,

    pub block_coinbase: Address,
    pub block_number: u64,
    pub block_timestamp: i64,
    pub block_nrglimit: u64,
    pub block_difficulty: DataWord,

    pub result: TransactionResult,
}

impl ExecutionContext {
    // correspond to constructor func for ExecutionContext
    pub fn new(
        tx_hash: Vec<u8>,
        address: Address,
        origin: Address,
        caller: Address,
        nrg_price: DataWord,
        nrg_limit: u64,
        call_value: DataWord,
        call_data: Vec<u8>,
        depth: i32,
        kind: i32,
        flags: i32,
        block_coinbase: Address,
        block_number: u64,
        block_timestamp: i64,
        block_nrglimit: u64,
        block_difficulty: DataWord,
        result: TransactionResult,
    ) -> ExecutionContext
    {
        ExecutionContext {
            address: address,
            origin: origin,
            caller: caller,

            nrg_price: nrg_price,
            nrg_limit: nrg_limit,
            call_value: call_value,
            call_data: call_data,

            depth: depth,
            kind: kind,
            flags: flags,

            block_coinbase: block_coinbase,
            block_number: block_number,
            block_timestamp: block_timestamp,
            block_nrglimit: block_nrglimit,
            block_difficulty: block_difficulty,

            tx_hash: tx_hash,
            result: result,
        }
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        ExecutionContext::new(
            vec![
                1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4,
                5, 6, 7, 8,
            ],
            Address::default(),           // address
            Address::default(),           // origin
            Address::default(),           // caller
            DataWord::new_with_int(1i32), // nrg_price
            1000000,                      // nrg_limit
            DataWord::new_with_int(0),    // call_value
            Vec::new(),                   // call_data
            0,                            // depth
            0,                            // kind
            0,                            // flags
            "000".as_bytes().into(),      // block coinbase
            0,                            // block number
            1,                            // block timestamp
            2000000,                      // block nrglimit
            DataWord::new_with_int(100),  // block difficulty
            TransactionResult::new(),     // tx result
        )
    }
}

// execution kind, this definition from FVM
pub mod execution_kind {
    pub const CREATE: i32 = 3;
    pub const CALLCODE: i32 = 2;
    pub const DELEGATECALL: i32 = 1;
    pub const CALL: i32 = 0;
}
