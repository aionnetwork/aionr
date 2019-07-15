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

use aion_types::{H256, U256, Address};

use types::{Transaction, Block, AcitvePeerInfo, PbSyncInfo, Receipt, Bytes, SimpleReceipt};

pub trait Pb: Sync + Send {
    fn balance(&self, address: Address) -> U256;

    fn transaction_by_hash(&self, txhash: H256) -> Option<Transaction>;

    fn nonce(&self, address: Address) -> U256;

    fn blocknumber(&self) -> U256;

    fn block_by_number(&self, number: i64, include_txs: bool) -> Option<Block>;

    fn block_receipt(&self, number: i64) -> Vec<SimpleReceipt>;

    fn get_active_nodes(&self) -> Vec<AcitvePeerInfo>;

    fn get_sync(&self) -> PbSyncInfo;

    fn transaction_receipt(&self, txhash: H256) -> Option<Receipt>;

    fn pb_send_transaction(&self, raw: Bytes) -> Option<H256>;
}
