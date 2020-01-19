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

//! Eth rpc interface.
use std::collections::HashMap;

use jsonrpc_core::{Result, BoxFuture};
use jsonrpc_derive::rpc;
use aion_types::{H64, H256, U256, U128, H128, Address};

use crate::types::{Block, BlockNumber, Bytes, CallRequest, Filter, FilterChanges, Index};
use crate::types::{Log, Receipt, SyncStatus, Transaction, Work, Contract};

/// Eth rpc interface.
#[rpc(server)]
pub trait Eth {
    type Metadata;

    /// Returns protocol version encoded as a string (quotes are necessary).
    #[rpc(name = "eth_protocolVersion")]
    fn protocol_version(&self) -> Result<String>;

    /// Returns an object with data about the sync status or false. (wtf?)
    #[rpc(name = "eth_syncing")]
    fn syncing(&self) -> Result<SyncStatus>;

    /// Returns the number of hashes per second that the node is mining with.
    #[rpc(name = "eth_hashrate")]
    fn hashrate(&self) -> Result<String>;

    /// Returns block author.
    #[rpc(name = "eth_coinbase")]
    fn author(&self) -> Result<H256>;

    /// Returns true if client is actively mining new blocks.
    #[rpc(name = "eth_mining")]
    fn is_mining(&self) -> Result<bool>;

    /// Returns current gas_price.
    #[rpc(name = "eth_gasPrice")]
    fn gas_price(&self) -> Result<U256>;

    /// Returns accounts list.
    #[rpc(name = "eth_accounts")]
    fn accounts(&self) -> Result<Vec<H256>>;

    /// Returns highest block number.
    #[rpc(name = "eth_blockNumber")]
    fn block_number(&self) -> Result<u64>;

    /// Returns balance of the given account.
    #[rpc(name = "eth_getBalance")]
    fn balance(&self, address: H256, num: Option<BlockNumber>) -> BoxFuture<U256>;

    /// Returns content of the storage at given address.
    #[rpc(name = "eth_getStorageAt")]
    fn storage_at(&self, address: Address, pos: U128, num: Option<BlockNumber>) -> BoxFuture<H128>;

    /// Returns block with given hash.
    #[rpc(name = "eth_getBlockByHash")]
    fn block_by_hash(&self, hash: H256, include_txs: bool) -> BoxFuture<Option<Block>>;

    /// Returns block with given number.
    #[rpc(name = "eth_getBlockByNumber")]
    fn block_by_number(&self, num: BlockNumber, include_txs: bool) -> BoxFuture<Option<Block>>;

    /// Returns the number of transactions sent from given address at given time (block number).
    #[rpc(name = "eth_getTransactionCount")]
    fn transaction_count(&self, address: Address, num: Option<BlockNumber>) -> BoxFuture<U256>;

    /// Returns the number of transactions in a block with given hash.
    #[rpc(name = "eth_getBlockTransactionCountByHash")]
    fn block_transaction_count_by_hash(&self, hash: H256) -> BoxFuture<Option<U256>>;

    /// Returns the number of transactions in a block with given block number.
    #[rpc(name = "eth_getBlockTransactionCountByNumber")]
    fn block_transaction_count_by_number(&self, num: BlockNumber) -> BoxFuture<Option<U256>>;

    /// Returns the code at given address at given time (block number).
    #[rpc(name = "eth_getCode")]
    fn code_at(&self, address: Address, num: Option<BlockNumber>) -> BoxFuture<Bytes>;

    /// Sends signed transaction, returning its hash.
    #[rpc(name = "eth_sendRawTransaction")]
    fn send_raw_transaction(&self, raw: Bytes) -> Result<H256>;

    /// @alias of `eth_sendRawTransaction`.
    #[rpc(name = "eth_submitTransaction")]
    fn submit_transaction(&self, raw: Bytes) -> Result<H256>;

    /// Call contract, returning the output data.
    #[rpc(name = "eth_call")]
    fn call(&self, request: CallRequest, num: Option<BlockNumber>) -> BoxFuture<Bytes>;

    /// Estimate gas needed for execution of given contract.
    #[rpc(name = "eth_estimateGas")]
    fn estimate_gas(&self, request: CallRequest, num: Option<BlockNumber>) -> BoxFuture<U256>;

    /// Get transaction by its hash.
    #[rpc(name = "eth_getTransactionByHash")]
    fn transaction_by_hash(&self, hash: H256) -> BoxFuture<Option<Transaction>>;

    /// Returns transaction at given block hash and index.
    #[rpc(name = "eth_getTransactionByBlockHashAndIndex")]
    fn transaction_by_block_hash_and_index(
        &self,
        hash: H256,
        index: Index,
    ) -> BoxFuture<Option<Transaction>>;

    /// Returns transaction by given block number and index.
    #[rpc(name = "eth_getTransactionByBlockNumberAndIndex")]
    fn transaction_by_block_number_and_index(
        &self,
        num: BlockNumber,
        index: Index,
    ) -> BoxFuture<Option<Transaction>>;

    /// Returns transaction receipt by transaction hash.
    #[rpc(name = "eth_getTransactionReceipt")]
    fn transaction_receipt(&self, hash: H256) -> BoxFuture<Option<Receipt>>;

    /// Returns available compilers.
    /// @deprecated
    #[rpc(name = "eth_getCompilers")]
    fn compilers(&self) -> Result<Vec<String>>;

    /// Compiles lll code.
    /// @deprecated
    #[rpc(name = "eth_compileLLL")]
    fn compile_lll(&self, _: String) -> Result<Bytes>;

    /// Compiles solidity.
    /// @deprecated
    #[rpc(name = "eth_compileSolidity")]
    fn compile_solidity(&self, contract_texts: String) -> Result<HashMap<String, Contract>>;

    /// Compiles serpent.
    /// @deprecated
    #[rpc(name = "eth_compileSerpent")]
    fn compile_serpent(&self, _: String) -> Result<Bytes>;

    /// Returns logs matching given filter object.
    #[rpc(name = "eth_getLogs")]
    fn logs(&self, filter: Filter) -> BoxFuture<Vec<Log>>;

    /// Returns the hash of the current block, the seedHash, and the boundary condition to be met.
    #[rpc(name = "eth_getWork")]
    fn work(&self, _no_new_work_timeout: Option<u64>) -> Result<Work>;

    /// Used for submitting a proof-of-work solution.
    #[rpc(name = "eth_submitWork")]
    fn submit_work(&self, _nonce: H64, _pow_hash: H256, _solution: Bytes) -> Result<bool>;

    /// Used for submitting mining hashrate.
    #[rpc(name = "eth_submitHashrate")]
    fn submit_hashrate(&self, rate: U256, id: H256) -> Result<bool>;
}

/// Eth filters rpc api (polling).
// TODO: do filters api properly
#[rpc(server)]
pub trait EthFilter {
    type Metadata;

    /// Returns id of new filter.
    #[rpc(name = "eth_newFilter")]
    fn new_filter(&self, filter: Filter) -> Result<U256>;

    /// Returns id of new block filter.
    #[rpc(name = "eth_newBlockFilter")]
    fn new_block_filter(&self) -> Result<U256>;

    /// Returns id of new block filter.
    #[rpc(name = "eth_newPendingTransactionFilter")]
    fn new_pending_transaction_filter(&self) -> Result<U256>;

    /// Returns filter changes since last poll.
    #[rpc(name = "eth_getFilterChanges")]
    fn filter_changes(&self, index: Index) -> BoxFuture<FilterChanges>;

    /// Returns all logs matching given filter (in a range 'from' - 'to').
    #[rpc(name = "eth_getFilterLogs")]
    fn filter_logs(&self, index: Index) -> BoxFuture<Vec<Log>>;

    /// Uninstalls filter.
    #[rpc(name = "eth_uninstallFilter")]
    fn uninstall_filter(&self, index: Index) -> Result<bool>;
}
