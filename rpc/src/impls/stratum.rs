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

//! Stratum rpc implementation.

use std::thread;
use std::time::{Instant, Duration};
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, LinkedList};
use rustc_hex::FromHex;
use rustc_hex::ToHex;

use aion_types::{H256, H512, U256};
use acore::block::IsBlock;
use acore::sync::SyncProvider;
use acore::client::{MiningBlockChainClient, BlockId};
use acore::miner::MinerService;
use acore::account_provider::AccountProvider;
use acore::header::SealType;
use jsonrpc_core::{Error, Result};

use crate::helpers::errors;
use crate::helpers::accounts::unwrap_provider;
use crate::traits::Stratum;
use crate::Metadata;
use crate::types::{
    Work, Info, AddressValidation, MiningInfo, MinerStats, TemplateParam, Bytes, StratumHeader,
    SimpleHeader, BlockNumber
};
use aion_types::clean_0x;

const MAX_QUEUE_SIZE_TO_MINE_ON: usize = 4;
// The maximum number of the latest pow blocks to count block time
const STRATUM_BLKTIME_INCLUDED_COUNT: usize = 32;
// The maximum latest blocks to cache
const STRATUM_RECENT_BLK_COUNT: usize = 256;
// The maximum latest PoW blocks to cache
const STRATUM_RECENT_POW_BLK_COUNT: usize = 128;

/// Stratum rpc implementation.
pub struct StratumClient<C, S: ?Sized, M>
where
    C: MiningBlockChainClient,
    S: SyncProvider,
    M: MinerService,
{
    client: Arc<C>,
    sync: Arc<S>,
    miner: Arc<M>,
    account_provider: Option<Arc<AccountProvider>>,
    recent_block_hash: Mutex<LinkedList<H256>>,
    recent_block_header: Mutex<HashMap<H256, (H256, u64, SealType)>>,
}

impl<C, S: ?Sized, M> StratumClient<C, S, M>
where
    C: MiningBlockChainClient,
    S: SyncProvider,
    M: MinerService,
{
    /// Creates new StratumClient.
    pub fn new(
        client: &Arc<C>,
        sync: &Arc<S>,
        miner: &Arc<M>,
        account_provider: &Option<Arc<AccountProvider>>,
    ) -> Self
    {
        StratumClient {
            client: client.clone(),
            sync: sync.clone(),
            miner: miner.clone(),
            account_provider: account_provider.clone(),
            recent_block_hash: Mutex::new(LinkedList::new()),
            recent_block_header: Mutex::new(HashMap::with_capacity(STRATUM_RECENT_BLK_COUNT)),
        }
    }

    fn account_provider(&self) -> Result<Arc<AccountProvider>> {
        unwrap_provider(&self.account_provider)
    }

    fn check_syncing(&self) -> Result<()> {
        //TODO: check if initial sync is complete here
        //let sync = self.sync;
        if
        /*sync.status().state != SyncState::Idle ||*/
        self.client.queue_info().total_queue_size() > MAX_QUEUE_SIZE_TO_MINE_ON {
            trace!(target: "miner", "Syncing. Cannot give any work.");
            return Err(errors::no_work());
        }

        // Otherwise spin until our submitted block has been included.
        let timeout = Instant::now() + Duration::from_millis(1000);
        while Instant::now() < timeout && self.client.queue_info().total_queue_size() > 0 {
            thread::sleep(Duration::from_millis(1));
        }

        Ok(())
    }
}

impl<C, S: ?Sized, M> Stratum for StratumClient<C, S, M>
where
    C: MiningBlockChainClient + 'static,
    S: SyncProvider + 'static,
    M: MinerService + 'static,
{
    type Metadata = Metadata;

    /// Returns the work of current block
    fn work(&self, _tpl_param: Option<TemplateParam>) -> Result<Work> {
        // check if we're still syncing and return empty strings in that case
        self.check_syncing()?;

        if self.miner.author().is_zero() {
            warn!(target: "miner", "Cannot give work package - no author is configured. Use --author to configure!");
            return Err(errors::no_author());
        }
        self.miner
            .map_sealing_work(&*self.client, |b| {
                let pow_hash = b.block().header().mine_hash();
                let target = b.block().header().boundary();
                let parent_hash = b.header().parent_hash().clone();
                let transaction_fee = b.header().transaction_fee().clone();
                let block_number = b.block().header().number();
                let reward = b.header().reward().clone();

                Ok(Work {
                    pow_hash: pow_hash,
                    parent_hash: parent_hash,
                    target: target,
                    number: block_number,
                    reward: reward,
                    transaction_fee: transaction_fee,
                })
            })
            .unwrap_or(Err(errors::internal("No work found.", "")))
    }

    /// get block header by number
    fn get_block_by_number(&self, num: BlockNumber) -> Result<StratumHeader> {
        let client = &self.client;
        let mut stratum_header = StratumHeader::default();
        match client.block(num.clone().into()) {
            Some(b) => {
                let header = b.decode_header();
                let simple_header = SimpleHeader::from(header.clone());
                stratum_header.code = 0;
                let seal = header.seal();
                if seal.len() == 2 {
                    stratum_header.nonce = Some(seal[0].to_hex());
                    stratum_header.solution = Some(seal[1].to_hex());
                    stratum_header.header_hash =
                        Some(clean_0x(&format!("{:?}", header.mine_hash())).to_owned());
                    stratum_header.block_header = Some(simple_header);
                } else {
                    stratum_header.code = -4;
                    stratum_header.message = Some("No nonce or solution.".into());
                }
            }
            None => {
                stratum_header.code = -2;
                stratum_header.message = Some(format!("Fail - Unable to find block{:?}", num));
            }
        }
        Ok(stratum_header)
    }

    /// Submit a proof-of-work solution
    fn submit_work(
        &self,
        nonce_str: String,
        solution_str: String,
        pow_hash_str: String,
    ) -> Result<bool>
    {
        let nonce: H256 = clean_0x(nonce_str.as_str())
            .parse()
            .map_err(|_e| Error::invalid_params("invalid nonce"))?;

        let solution = Bytes(
            clean_0x(solution_str.as_str())
                .from_hex()
                .map_err(|_e| Error::invalid_params("invalid solution"))?,
        );

        let pow_hash: H256 = clean_0x(pow_hash_str.as_str())
            .parse()
            .map_err(|_e| Error::invalid_params("invalid pow_hash"))?;

        trace!(target: "miner", "submit_work: Decoded: nonce={}, pow_hash={}, solution={:?}", nonce, pow_hash, solution);

        let seal = vec![nonce.to_vec(), solution.0];
        Ok(self
            .miner
            .submit_seal(&*self.client, pow_hash, seal)
            .is_ok())
    }

    /// Get information
    fn get_info(&self) -> Result<Info> {
        // TODO
        Ok(Info {
            balance: 0,
            blocks: 0,
            connections: self.sync.status().num_peers as u64,
            proxy: String::default(),
            generate: true,
            genproclimit: 100,
            difficulty: 0,
        })
    }

    /// Check if address is valid
    fn validate_address(&self, address: H256) -> Result<AddressValidation> {
        let isvalid: bool = address.0[0] == 0xa0 as u8;
        let account_provider = self.account_provider()?;
        let ismine = match account_provider.has_account(&address) {
            Ok(true) => true,
            _ => false,
        };
        Ok(AddressValidation {
            isvalid,
            address,
            ismine,
        })
    }

    /// Get the highest known difficulty
    fn get_difficulty(&self) -> Result<U256> {
        let best_pow_block = self
            .client
            .best_pow_block()
            .expect("must have a best pow block");
        Ok(best_pow_block.difficulty())
    }

    /// Get mining information
    fn get_mining_info(&self) -> Result<MiningInfo> {
        let best_pow_block = self
            .client
            .best_pow_block()
            .expect("must have a best pow block");
        Ok(MiningInfo {
            blocks: best_pow_block.number(),
            currentblocksize: best_pow_block.0.len(),
            currentblocktx: best_pow_block.transactions_count(),
            difficulty: best_pow_block.difficulty(),
            testnet: true,
        })
    }

    /// Get miner stats
    fn get_miner_stats(&self, address: H256) -> Result<MinerStats> {
        let mut best_header = self
            .client
            .block_header(BlockId::Latest)
            .expect("must have a best block header");
        let latest_pow_difficulty = self
            .client
            .best_pow_block()
            .expect("must have a best pow block")
            .header()
            .difficulty();
        let mut index = 0;
        let mut new_blk_headers = Vec::new();
        let mut recent_block_hash = self.recent_block_hash.lock().unwrap();

        // Get latest blocks to make sure it has at least 128 PoW blocks
        if let Some(last_blk_hash) = recent_block_hash.front() {
            while *last_blk_hash != best_header.hash()
                && index < STRATUM_RECENT_POW_BLK_COUNT
                && best_header.number() > 2
            {
                let parent_hash = best_header.parent_hash();
                if best_header.seal_type().unwrap_or_default() == SealType::PoW {
                    index = index + 1;
                }
                new_blk_headers.push(best_header);
                match self.client.block_header(BlockId::Hash(parent_hash.into())) {
                    Some(h) => best_header = h,
                    None => break,
                }
            }
        } else {
            while index < STRATUM_RECENT_POW_BLK_COUNT && best_header.number() > 2 {
                let parent_hash = best_header.parent_hash();
                if best_header.seal_type().unwrap_or_default() == SealType::PoW {
                    index = index + 1;
                }
                new_blk_headers.push(best_header);
                match self.client.block_header(BlockId::Hash(parent_hash.into())) {
                    Some(h) => best_header = h,
                    None => break,
                }
            }
        }

        // Update latest 256 block headers cache
        let mut recent_block_header = self.recent_block_header.lock().unwrap();
        while let Some(top) = new_blk_headers.pop() {
            if recent_block_hash.len() == STRATUM_RECENT_BLK_COUNT {
                if let Some(hash) = recent_block_hash.pop_back() {
                    recent_block_header.remove(&hash);
                }
            }
            recent_block_hash.push_front(top.hash());
            recent_block_header.insert(
                top.hash(),
                (
                    top.author(),
                    top.timestamp(),
                    top.seal_type().unwrap_or_default(),
                ),
            );
        }

        // Calculate the average pow block time (only count for latest 32 pow blocks) and count the blocks mined by the miner
        let mut last_block_timestamp = 0;
        let mut block_time_accumulator = 0;
        let mut block_time_accumulated = 0;
        let mut mined_by_miner = 0;
        let mut pow_index = 0;
        let mut last_seal_type = Default::default();
        for hash in recent_block_hash.iter() {
            if let Some((author, timestamp, seal_type)) = recent_block_header.get(hash) {
                // Only count the latest 32 pow blocks' block time
                if pow_index <= STRATUM_BLKTIME_INCLUDED_COUNT {
                    // If last block is a pow block, calculate the delta of its timestamp and the recorded last timestamp to get pow block time
                    if last_seal_type == SealType::PoW && last_block_timestamp != 0 {
                        block_time_accumulator =
                            block_time_accumulator + (last_block_timestamp - timestamp);
                        block_time_accumulated = block_time_accumulated + 1;
                    }
                    // If it's a pow block, record its timestamp
                    if *seal_type == SealType::PoW {
                        last_block_timestamp = *timestamp;
                    }
                }
                // Only calculate recent 128 pow blocks' hash rate
                if pow_index > STRATUM_RECENT_POW_BLK_COUNT {
                    break;
                }

                if *seal_type == SealType::PoW {
                    if *author == address {
                        mined_by_miner = mined_by_miner + 1;
                    }
                    pow_index = pow_index + 1;
                }
                last_seal_type = seal_type.clone();
            }
        }
        let mut block_time: f64 = 0_f64;
        if block_time_accumulator > 0 {
            block_time = block_time_accumulator as f64 / block_time_accumulated as f64;
        }

        // Calculate the network hashrate and the miner's hash share and hash rate
        let mut network_hashrate = 0_f64;
        let mut miner_hashrate_share = 0_f64;
        let mut miner_hashrate = 0_f64;
        if block_time > 0_f64 {
            network_hashrate = latest_pow_difficulty.as_f64() / block_time;
        }
        // hashrate shared by miner: mined blocks / total blocks
        if pow_index > 0 && mined_by_miner > 0 {
            miner_hashrate_share = mined_by_miner as f64 / pow_index as f64;
            miner_hashrate = network_hashrate * miner_hashrate_share;
        }

        Ok(MinerStats {
            miner_hashrate_share,
            miner_hashrate,
            network_hashrate,
        })
    }

    /// PoS get seed
    fn pos_get_seed(&self) -> Result<H512> {
        // seal map:
        // 64 bytes signature + 64 bytes seed + 32 public key
        if !self
            .miner
            .new_block_allowed_with_seal_type(&*self.client, &SealType::PoS)
        {
            return Err(errors::pos_not_allowed());
        }
        let best_block = self.client.best_block_header();
        let grand_parent = self.client.block_header_data(&best_block.parent_hash());
        match grand_parent {
            Some(ref header) if header.seal_type() == Some(SealType::PoS) => {
                // seal length must be 3, since it is already validated
                let seal = header.seal();
                let mut s = [0u8; 64];
                debug!(target: "miner", "seal = {:?}, len = {}", seal, seal.len());
                s.copy_from_slice(seal[0].as_slice());
                Ok(s.into())
            }
            _ => Ok(H512::zero()),
        }
    }

    /// PoS submit seed
    /// seed: signed seed by staker
    /// psk: public key of staker
    /// coinbase: the account who receives block reward
    fn pos_submit_seed(&self, seed: H512, psk: H256, coinbase: H256) -> Result<H256> {
        // try to get block hash
        debug!(target: "miner", "submit seed: {:?} - {:?}", seed, psk);
        let template = self
            .miner
            .get_pos_template(&*self.client, seed.into(), psk, coinbase);
        if template.is_some() {
            return Ok(template.unwrap().into());
        } else {
            return Err(errors::pos_not_allowed());
        }
    }

    /// PoS submit work
    fn pos_submit_work(&self, sig: H512, hash: H256) -> Result<bool> {
        if let Some((block, mut seal)) = self.miner.get_ready_pos(&hash) {
            debug!(target: "miner", "got PoS block template");
            seal[1] = sig.to_vec();
            let result = self.miner.try_seal_pos(&*self.client, seal, block);
            Ok(result.is_ok())
        } else {
            Ok(false)
        }
    }
}
