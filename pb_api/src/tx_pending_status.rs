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

#![allow(dead_code)]
use aion_types::H256;
use bytes::Bytes;

static TX_RET_CODE_OFFSET: i32 = 102;

/// pending transaction status information
#[derive(Default)]
pub struct TxPendingStatus {
    pub tx_hash: H256,
    pub socket_id: Bytes,
    pub msg_hash: Vec<u8>,
    pub tx_result: Bytes,
    pub error: String,
    pub state: i32,
}

impl TxPendingStatus {
    /// return transaction status code
    pub fn to_tx_return_code(&self) -> i32 { self.state + TX_RET_CODE_OFFSET }

    pub fn is_empty(&self) -> bool { self.tx_hash.len() == 0 && self.socket_id.len() == 0 }

    pub fn tx_hash(&self) -> &H256 { &self.tx_hash }

    pub fn socket_id(&self) -> &Bytes { &self.socket_id }

    pub fn msg_hash(&self) -> &Vec<u8> { &self.msg_hash }

    pub fn tx_result(&self) -> &Bytes { &self.tx_result }

    pub fn error(&self) -> &String { &self.error }

    pub fn state(&self) -> i32 { self.state }
}
