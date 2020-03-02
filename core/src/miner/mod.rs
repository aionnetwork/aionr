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

mod miner;
pub mod external;

pub use self::miner::{Miner, MinerOptions, Banning, PendingSet};
pub use transaction::local_transactions::Status as LocalTransactionStatus;

use std::collections::BTreeMap;

use aion_types::{H256, U256, Address};
use acore_bytes::Bytes;
use block::ClosedBlock;
use client::{MiningBlockChainClient};
use types::error::{Error};
use header::{BlockNumber, SealType};
use receipt::Receipt;
use transaction::{UnverifiedTransaction, PendingTransaction};
use key::Ed25519KeyPair;

/// Miner client API, this trait is somewhat related to multiple kinds of miner
/// however, only one kind of miner now
pub trait MinerService: Send + Sync {
    /// Returns miner's status.
    fn status(&self) -> MinerStatus;

    /// Get the author that we will seal blocks as.
    fn author(&self) -> Address;

    /// Get the PoS staker that will seal PoS blocks.
    fn staker(&self) -> &Option<Ed25519KeyPair>;

    /// Set the author that we will seal blocks as.
    fn set_author(&self, author: Address);

    /// Set the PoS author that will seal PoS blocks.
    fn set_staker(&mut self, staker: Ed25519KeyPair);

    /// Get the extra_data that we will seal blocks with.
    fn extra_data(&self) -> Bytes;

    /// Set the extra_data that we will seal blocks with.
    fn set_extra_data(&self, extra_data: Bytes);

    /// Get current minimal gas price for transactions accepted to queue.
    fn minimal_gas_price(&self) -> U256;

    /// Set minimal gas price of transaction to be accepted for mining.
    fn set_minimal_gas_price(&mut self, min_gas_price: U256);

    /// Get current maximal gas price for transactions accepted to queue.
    fn maximal_gas_price(&self) -> U256;

    /// Set maximal gas price of transaction to be accepted for mining.
    fn set_maximal_gas_price(&mut self, min_gas_price: U256);

    /// Get current maximum gas price for new local transactions accepted to queue when using dynamic gas price.
    fn local_maximal_gas_price(&self) -> U256;

    /// Set maximum gas price of new local transaction to be accepted for mining when using dynamic gas price.
    //    fn set_local_maximal_gas_price(&mut self, default_max_gas_price: U256);

    /// Get the lower bound of the gas limit we wish to target when sealing a new block.
    fn gas_floor_target(&self) -> U256;

    /// Get the upper bound of the gas limit we wish to target when sealing a new block.
    fn gas_ceil_target(&self) -> U256;

    // TODO: coalesce into single set_range function.
    /// Set the lower bound of gas limit we wish to target when sealing a new block.
    fn set_gas_floor_target(&self, target: U256);

    /// Set the upper bound of gas limit we wish to target when sealing a new block.
    fn set_gas_ceil_target(&self, target: U256);

    /// Set maximum amount of gas allowed for any single transaction to mine.
    //    fn set_tx_gas_limit(&mut self, limit: U256);

    /// Get maximum amount of gas allowed for any single transaction to mine.
    fn tx_gas_limit(&self) -> U256;

    /// Imports transactions to transaction queue.
    fn import_external_transactions(
        &self,
        chain: &MiningBlockChainClient,
        transactions: Vec<UnverifiedTransaction>,
    ) -> Vec<Result<(), Error>>;

    /// Imports own (node owner) transaction to queue.
    fn import_own_transaction(
        &self,
        chain: &MiningBlockChainClient,
        transaction: PendingTransaction,
    ) -> Result<(), Error>;

    /// Returns hashes of transactions currently in pending
    fn pending_transactions_hashes(&self, best_block: BlockNumber) -> Vec<H256>;

    /// Removes all transactions from the queue and restart mining operation.
    fn clear_and_reset(&self, chain: &MiningBlockChainClient);

    /// Called when blocks are imported to chain, updates transactions queue.
    fn chain_new_blocks(
        &self,
        chain: &MiningBlockChainClient,
        imported: &[H256],
        invalid: &[H256],
        enacted: &[H256],
        retracted: &[H256],
    );

    /// Submit `seal` as a valid solution for the header of `pow_hash`.
    /// Will check the seal, but not actually insert the block into the chain.
    fn submit_seal(
        &self,
        chain: &MiningBlockChainClient,
        pow_hash: H256,
        seal: Vec<Bytes>,
    ) -> Result<(), Error>;

    fn add_sealing_pos(
        &self,
        hash: &H256,
        b: ClosedBlock,
        seed: [u8; 64],
        timestamp: u64,
        client: &MiningBlockChainClient,
    ) -> Result<(), Error>;

    fn get_ready_pos(&self, h: &H256) -> Option<(ClosedBlock, Vec<Bytes>)>;

    fn clear_pos_pending(&self);

    fn get_pos_template(
        &self,
        client: &MiningBlockChainClient,
        seed: [u8; 64],
        public_key: H256,
        coinbase: H256,
    ) -> Option<H256>;

    fn try_seal_pos(
        &self,
        client: &MiningBlockChainClient,
        seal: Vec<Bytes>,
        block: ClosedBlock,
    ) -> Result<(), Error>;

    // AION 2.0
    // Check if next block is on the unity hard fork
    fn unity_update(&self, client: &MiningBlockChainClient) -> bool;

    // AION Unity hybrid seed update
    // Check if the next block is on the unity hybrid seed hard fork
    fn unity_hybrid_seed_update(&self, client: &MiningBlockChainClient) -> bool;

    // AION 2.0
    // Check if it's allowed to produce a new block with given seal type.
    // A block's seal type must be different than its parent's seal type.
    fn new_block_allowed_with_seal_type(
        &self,
        client: &MiningBlockChainClient,
        seal_type: &SealType,
    ) -> bool;

    /// Get the sealing work package and if `Some`, apply some transform.
    fn map_sealing_work<F, T>(&self, chain: &MiningBlockChainClient, f: F) -> Option<T>
    where
        F: FnOnce(&ClosedBlock) -> T,
        Self: Sized;

    /// Query pending transactions for hash.
    fn transaction(&self, best_block: BlockNumber, hash: &H256) -> Option<PendingTransaction>;

    /// Removes transaction from the queue.
    /// NOTE: The transaction is not removed from pending block if mining.
    //    fn remove_pending_transaction(&self, hash: H256);

    /// Get a list of all pending transactions in the queue.
    fn pending_transactions(&self) -> Vec<PendingTransaction>;

    /// Get a list of all transactions that can go into the given block.
    fn ready_transactions(
        &self,
        best_block: BlockNumber,
        best_block_timestamp: u64,
    ) -> Vec<PendingTransaction>;

    /// Get a list of all future transactions.
    fn future_transactions(&self) -> Vec<PendingTransaction>;

    /// Get a list of local transactions with statuses.
    //    fn local_transactions(&self) -> HashMap<H256, LocalTransactionStatus>;

    /// Get a list of all pending receipts.
    fn pending_receipts(&self, best_block: BlockNumber) -> BTreeMap<H256, Receipt>;

    /// Get a particular receipt.
    //    fn pending_receipt(&self, best_block: BlockNumber, hash: &H256) -> Option<RichReceipt>;

    /// Returns highest transaction nonce for given address.
    fn last_nonce(&self, address: &Address) -> Option<U256>;

    /// Is it currently sealing?
    fn is_currently_sealing(&self) -> bool;

    /// Default suggested gas limit.
    fn default_gas_limit(&self) -> U256;
}

/// Mining status
#[derive(Debug)]
pub struct MinerStatus {
    /// Number of transactions in queue with state `pending` (ready to be included in block)
    pub transactions_in_pending_queue: usize,
    /// Number of transactions in queue with state `future` (not yet ready to be included in block)
    pub transactions_in_future_queue: usize,
    /// Number of transactions included in currently mined block
    pub transactions_in_pending_block: usize,
}
