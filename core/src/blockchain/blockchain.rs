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

//! Blockchain database.

use std::collections::{HashMap, hash_map};
use std::sync::Arc;
use std::mem;
use itertools::Itertools;
use bloomchain as bc;
use heapsize::HeapSizeOf;
use aion_types::{H256, U256};
use ethbloom::Bloom;
use parking_lot::{Mutex, RwLock};
use acore_bytes::Bytes;
use rlp::*;
use rlp_compress::{compress, decompress, blocks_swapper};
use header::*;
use transaction::*;
use views::*;
use log_entry::{LogEntry, LocalizedLogEntry};
use receipt::Receipt;
use types::blooms::{BloomGroup, GroupPosition};
use types::blockchain::best_block::{BestBlock, BestAncientBlock};
use types::block::info::{BlockInfo, BlockLocation, BranchBecomingCanonChainData};
use types::blockchain::extra::{
    BlockReceipts, BlockDetails, TransactionAddress,
};
use types::blockchain::info::BlockChainInfo;
use types::blockchain::tree_route::TreeRoute;
use types::block::extra_update::ExtrasUpdate;
use types::blockchain::config::Config;
use types::blockchain::cache::CacheSize;
use types::blockchain::import_route::ImportRoute;
use db::{self, Writable, Readable, CacheUpdatePolicy};
use cache_manager::CacheManager;
use encoded;
// use engine::epoch::{PendingTransition as PendingEpochTransition};
use rayon::prelude::*;
use ansi_term::Colour;
use kvdb::{DBTransaction, KeyValueDB};

const LOG_BLOOMS_LEVELS: usize = 3;
const LOG_BLOOMS_ELEMENTS_PER_INDEX: usize = 16;

/// Interface for querying blocks by hash and by number.
pub trait BlockProvider {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known(&self, hash: &H256) -> bool;

    /// Get the first block of the best part of the chain.
    /// Return `None` if there is no gap and the first block is the genesis.
    /// Any queries of blocks which precede this one are not guaranteed to
    /// succeed.
    fn first_block(&self) -> Option<H256>;

    /// Get the number of the first block.
    fn first_block_number(&self) -> Option<BlockNumber> {
        self.first_block().map(|b| {
            self.block_number(&b).expect(
                "First block is always set to an existing block or `None`. Existing block always \
                 has a number; qed",
            )
        })
    }

    /// Get the best block of an first block sequence if there is a gap.
    fn best_ancient_block(&self) -> Option<H256>;

    /// Get the number of the first block.
    fn best_ancient_number(&self) -> Option<BlockNumber> {
        self.best_ancient_block().map(|h| {
            self.block_number(&h).expect(
                "Ancient block is always set to an existing block or `None`. Existing block \
                 always has a number; qed",
            )
        })
    }
    /// Get raw block data
    fn block(&self, hash: &H256) -> Option<encoded::Block>;

    /// Get the familial details concerning a block.
    fn block_details(&self, hash: &H256) -> Option<BlockDetails>;

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256>;

    /// Get the address of transaction with given hash.
    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress>;

    /// Get receipts of block with given hash.
    fn block_receipts(&self, hash: &H256) -> Option<BlockReceipts>;

    /// Get the partial-header of a block.
    fn block_header(&self, hash: &H256) -> Option<Header> {
        self.block_header_data(hash).map(|header| header.decode())
    }

    /// Get the header RLP of a block.
    fn block_header_data(&self, hash: &H256) -> Option<encoded::Header>;

    /// Get the header RLP of the seal parent of the given block.
    /// Parameters:
    ///   parent_hash: parent hash of the given block
    ///   seal_type: seal type of the given block
    fn seal_parent_header(
        &self,
        parent_hash: &H256,
        seal_type: &Option<SealType>,
    ) -> Option<::encoded::Header>;

    /// Get the current best block with specified seal type
    fn best_block_header_with_seal_type(&self, seal_type: &SealType) -> Option<encoded::Header>;

    /// Get the block body (uncles and transactions).
    fn block_body(&self, hash: &H256) -> Option<encoded::Body>;

    /// Get the number of given block's hash.
    fn block_number(&self, hash: &H256) -> Option<BlockNumber> {
        self.block_details(hash).map(|details| details.number)
    }

    /// Get transaction with given transaction hash.
    fn transaction(&self, address: &TransactionAddress) -> Option<LocalizedTransaction> {
        self.block_body(&address.block_hash).and_then(|body| {
            self.block_number(&address.block_hash).and_then(|n| {
                body.view()
                    .localized_transaction_at(&address.block_hash, n, address.index)
            })
        })
    }

    /// Get transaction receipt.
    fn transaction_receipt(&self, address: &TransactionAddress) -> Option<Receipt> {
        self.block_receipts(&address.block_hash)
            .and_then(|br| br.receipts.into_iter().nth(address.index))
    }

    /// Get a list of transactions for a given block.
    /// Returns None if block does not exist.
    fn transactions(&self, hash: &H256) -> Option<Vec<LocalizedTransaction>> {
        self.block_body(hash).and_then(|body| {
            self.block_number(hash)
                .map(|n| body.view().localized_transactions(hash, n))
        })
    }

    /// Returns reference to genesis hash.
    fn genesis_hash(&self) -> H256 {
        self.block_hash(0)
            .expect("Genesis hash should always exist")
    }

    /// Returns the header of the genesis block.
    fn genesis_header(&self) -> Header {
        self.block_header(&self.genesis_hash())
            .expect("Genesis header always stored; qed")
    }

    /// Returns numbers of blocks containing given bloom.
    fn blocks_with_bloom(
        &self,
        bloom: &Bloom,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Vec<BlockNumber>;

    /// Returns logs matching given filter.
    fn logs<F>(
        &self,
        blocks: Vec<BlockNumber>,
        matches: F,
        limit: Option<usize>,
    ) -> Vec<LocalizedLogEntry>
    where
        F: Fn(&LogEntry) -> bool + Send + Sync,
        Self: Sized;
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
enum CacheId {
    BlockHeader(H256),
    BlockBody(H256),
    BlockDetails(H256),
    BlockHashes(BlockNumber),
    TransactionAddresses(H256),
    BlocksBlooms(GroupPosition),
    BlockReceipts(H256),
}

impl bc::group::BloomGroupDatabase for BlockChain {
    fn blooms_at(&self, position: &bc::group::GroupPosition) -> Option<bc::group::BloomGroup> {
        let position = GroupPosition::from(position.clone());
        let result = self
            .db
            .read_with_cache(db::COL_EXTRA, &self.blocks_blooms, &position)
            .map(Into::into);
        self.cache_man
            .lock()
            .note_used(CacheId::BlocksBlooms(position));
        result
    }
}

/// Structure providing fast access to blockchain data.
///
/// **Does not do input data verification.**
pub struct BlockChain {
    // All locks must be captured in the order declared here.
    blooms_config: bc::Config,

    best_block: RwLock<BestBlock>,
    // Stores best block of the first uninterrupted sequence of blocks. `None` if there are no gaps.
    // Only updated with `insert_unordered_block`.
    best_ancient_block: RwLock<Option<BestAncientBlock>>,
    // Stores the last block of the last sequence of blocks. `None` if there are no gaps.
    // This is calculated on start and does not get updated.
    first_block: Option<H256>,

    // block cache
    block_headers: RwLock<HashMap<H256, Bytes>>,
    block_bodies: RwLock<HashMap<H256, Bytes>>,

    // extra caches
    block_details: RwLock<HashMap<H256, BlockDetails>>,
    block_hashes: RwLock<HashMap<BlockNumber, H256>>,
    transaction_addresses: RwLock<HashMap<H256, TransactionAddress>>,
    blocks_blooms: RwLock<HashMap<GroupPosition, BloomGroup>>,
    block_receipts: RwLock<HashMap<H256, BlockReceipts>>,

    db: Arc<KeyValueDB>,

    cache_man: Mutex<CacheManager<CacheId>>,

    pending_best_block: RwLock<Option<BestBlock>>,
    pending_block_hashes: RwLock<HashMap<BlockNumber, H256>>,
    pending_block_details: RwLock<HashMap<H256, BlockDetails>>,
    pending_transaction_addresses: RwLock<HashMap<H256, Option<TransactionAddress>>>,
}

impl BlockProvider for BlockChain {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known(&self, hash: &H256) -> bool {
        self.db
            .exists_with_cache(db::COL_EXTRA, &self.block_details, hash)
    }

    fn first_block(&self) -> Option<H256> { self.first_block.clone() }

    fn best_ancient_block(&self) -> Option<H256> {
        self.best_ancient_block.read().as_ref().map(|b| b.hash)
    }

    fn best_ancient_number(&self) -> Option<BlockNumber> {
        self.best_ancient_block.read().as_ref().map(|b| b.number)
    }

    /// Get raw block data
    fn block(&self, hash: &H256) -> Option<encoded::Block> {
        match (self.block_header_data(hash), self.block_body(hash)) {
            (Some(header), Some(body)) => {
                let mut block = RlpStream::new_list(2);
                let body_rlp = body.rlp();
                block.append_raw(header.rlp().as_raw(), 1);
                block.append_raw(body_rlp.at(0).as_raw(), 1);
                Some(encoded::Block::new(block.out()))
            }
            _ => None,
        }
    }

    /// Get block header data
    fn block_header_data(&self, hash: &H256) -> Option<encoded::Header> {
        // Check cache first
        {
            let read = self.block_headers.read();
            if let Some(v) = read.get(hash) {
                return Some(encoded::Header::new(v.clone()));
            }
        }

        // Check if it's the best block
        {
            let best_block = self.best_block.read();
            if &best_block.hash == hash {
                return Some(encoded::Header::new(
                    Rlp::new(&best_block.block).at(0).as_raw().to_vec(),
                ));
            }
        }

        // Read from DB and populate cache
        let opt = self
            .db
            .get(db::COL_HEADERS, hash)
            .expect("Low level database error. Some issue with disk?");

        let result = match opt {
            Some(b) => {
                let bytes = decompress(&b, blocks_swapper()).into_vec();
                let mut write = self.block_headers.write();
                write.insert(*hash, bytes.clone());
                Some(encoded::Header::new(bytes))
            }
            None => None,
        };

        self.cache_man.lock().note_used(CacheId::BlockHeader(*hash));
        result
    }

    /// Get the header RLP of the seal parent of the given block.
    /// Parameters:
    ///   parent_hash: parent hash of the given block
    ///   seal_type: seal type of the given block
    fn seal_parent_header(
        &self,
        parent_hash: &H256,
        seal_type: &Option<SealType>,
    ) -> Option<::encoded::Header>
    {
        // Get parent header
        let parent_header: ::encoded::Header = match self.block_header_data(parent_hash) {
            Some(header) => header,
            None => return None,
        };
        let parent_seal_type: Option<SealType> = parent_header.seal_type();
        // If parent's seal type is the same as the current, return parent
        if seal_type.to_owned().unwrap_or_default() == parent_seal_type.unwrap_or_default() {
            Some(parent_header)
        }
        // Else return the anti seal parent of the parent
        else {
            let parent_details: BlockDetails = match self.block_details(parent_hash) {
                Some(details) => details,
                None => return None,
            };
            let anti_seal_parent: H256 = parent_details.anti_seal_parent;
            self.block_header_data(&anti_seal_parent)
        }
    }

    /// Get the current best block with specified seal type
    fn best_block_header_with_seal_type(&self, seal_type: &SealType) -> Option<encoded::Header> {
        let best_block_header = self.best_block_header();
        // If the best block's seal type corresponds to the given seal type, return the current best block
        if seal_type == &best_block_header.seal_type().unwrap_or_default() {
            Some(best_block_header)
        }
        // Else, return the anti seal parent of the current best block
        else {
            let block_details: BlockDetails = match self.block_details(&best_block_header.hash()) {
                Some(details) => details,
                None => return None,
            };
            let anti_seal_parent: H256 = block_details.anti_seal_parent;
            self.block_header_data(&anti_seal_parent)
        }
    }

    /// Get block body data
    fn block_body(&self, hash: &H256) -> Option<encoded::Body> {
        // Check cache first
        {
            let read = self.block_bodies.read();
            if let Some(v) = read.get(hash) {
                return Some(encoded::Body::new(v.clone()));
            }
        }

        // Check if it's the best block
        {
            let best_block = self.best_block.read();
            if &best_block.hash == hash {
                return Some(encoded::Body::new(Self::block_to_body(&best_block.block)));
            }
        }

        // Read from DB and populate cache
        let opt = self
            .db
            .get(db::COL_BODIES, hash)
            .expect("Low level database error. Some issue with disk?");

        let result = match opt {
            Some(b) => {
                let bytes = decompress(&b, blocks_swapper()).into_vec();
                let mut write = self.block_bodies.write();
                write.insert(*hash, bytes.clone());
                Some(encoded::Body::new(bytes))
            }
            None => None,
        };

        self.cache_man.lock().note_used(CacheId::BlockBody(*hash));

        result
    }

    /// Get the familial details concerning a block.
    fn block_details(&self, hash: &H256) -> Option<BlockDetails> {
        let result = self
            .db
            .read_with_cache(db::COL_EXTRA, &self.block_details, hash);
        self.cache_man
            .lock()
            .note_used(CacheId::BlockDetails(*hash));
        result
    }

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256> {
        let result = self
            .db
            .read_with_cache(db::COL_EXTRA, &self.block_hashes, &index);
        self.cache_man.lock().note_used(CacheId::BlockHashes(index));
        result
    }

    /// Get the address of transaction with given hash.
    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress> {
        let result = self
            .db
            .read_with_cache(db::COL_EXTRA, &self.transaction_addresses, hash);
        self.cache_man
            .lock()
            .note_used(CacheId::TransactionAddresses(*hash));
        result
    }

    /// Get receipts of block with given hash.
    fn block_receipts(&self, hash: &H256) -> Option<BlockReceipts> {
        let result = self
            .db
            .read_with_cache(db::COL_EXTRA, &self.block_receipts, hash);
        self.cache_man
            .lock()
            .note_used(CacheId::BlockReceipts(*hash));
        result
    }

    /// Returns numbers of blocks containing given bloom.
    fn blocks_with_bloom(
        &self,
        bloom: &Bloom,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Vec<BlockNumber>
    {
        let range = from_block as bc::Number..to_block as bc::Number;
        let chain = bc::group::BloomGroupChain::new(self.blooms_config, self);
        chain
            .with_bloom(&range, bloom)
            .into_iter()
            .map(|b| b as BlockNumber)
            .collect()
    }

    fn logs<F>(
        &self,
        mut blocks: Vec<BlockNumber>,
        matches: F,
        limit: Option<usize>,
    ) -> Vec<LocalizedLogEntry>
    where
        F: Fn(&LogEntry) -> bool + Send + Sync,
        Self: Sized,
    {
        // sort in reverse order
        blocks.sort_by(|a, b| b.cmp(a));

        let mut logs = blocks
            .chunks(128)
            .flat_map(move |blocks_chunk| {
                blocks_chunk
                    .into_par_iter()
                    .filter_map(|number| self.block_hash(*number).map(|hash| (*number, hash)))
                    .filter_map(|(number, hash)| {
                        self.block_receipts(&hash)
                            .map(|r| (number, hash, r.receipts))
                    })
                    .filter_map(|(number, hash, receipts)| {
                        self.block_body(&hash)
                            .map(|ref b| (number, hash, receipts, b.transaction_hashes()))
                    })
                    .flat_map(|(number, hash, mut receipts, mut hashes)| {
                        if receipts.len() != hashes.len() {
                            warn!(
                                target: "blockchain",
                                "Block {} ({}) has different number of receipts ({}) to \
                                 transactions ({}). Database corrupt?",
                                number,
                                hash,
                                receipts.len(),
                                hashes.len()
                            );
                            assert!(false);
                        }
                        let mut log_index = receipts
                            .iter()
                            .fold(0, |sum, receipt| sum + receipt.logs().len());

                        let receipts_len = receipts.len();
                        hashes.reverse();
                        receipts.reverse();
                        receipts
                            .into_iter()
                            .map(|receipt| receipt.logs().clone())
                            .zip(hashes)
                            .enumerate()
                            .flat_map(move |(index, (mut logs, tx_hash))| {
                                let current_log_index = log_index;
                                let no_of_logs = logs.len();
                                log_index -= no_of_logs;

                                logs.reverse();
                                logs.into_iter().enumerate().map(move |(i, log)| {
                                    LocalizedLogEntry {
                                        entry: log.clone(),
                                        block_hash: hash,
                                        block_number: number,
                                        transaction_hash: tx_hash,
                                        // iterating in reverse order
                                        transaction_index: receipts_len - index - 1,
                                        transaction_log_index: no_of_logs - i - 1,
                                        log_index: current_log_index - i - 1,
                                    }
                                })
                            })
                            .filter(|log_entry| matches(&log_entry.entry))
                            .take(limit.unwrap_or(::std::usize::MAX))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .take(limit.unwrap_or(::std::usize::MAX))
            .collect::<Vec<LocalizedLogEntry>>();
        logs.reverse();
        logs
    }
}

/// An iterator which walks the blockchain towards the genesis.
#[cfg(test)]
#[derive(Clone)]
pub struct AncestryIter<'a> {
    current: H256,
    chain: &'a BlockChain,
}

#[cfg(test)]
impl<'a> Iterator for AncestryIter<'a> {
    type Item = H256;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current.is_zero() {
            None
        } else {
            self.chain
                .block_details(&self.current)
                .map(|details| mem::replace(&mut self.current, details.parent))
        }
    }
}

impl BlockChain {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(config: Config, genesis: &[u8], db: Arc<KeyValueDB>) -> BlockChain {
        // 400 is the avarage size of the key
        let cache_man = CacheManager::new(config.pref_cache_size, config.max_cache_size, 400);

        let mut bc = BlockChain {
            blooms_config: bc::Config {
                levels: LOG_BLOOMS_LEVELS,
                elements_per_index: LOG_BLOOMS_ELEMENTS_PER_INDEX,
            },
            first_block: None,
            best_block: RwLock::new(BestBlock::default()),
            best_ancient_block: RwLock::new(None),
            block_headers: RwLock::new(HashMap::new()),
            block_bodies: RwLock::new(HashMap::new()),
            block_details: RwLock::new(HashMap::new()),
            block_hashes: RwLock::new(HashMap::new()),
            transaction_addresses: RwLock::new(HashMap::new()),
            blocks_blooms: RwLock::new(HashMap::new()),
            block_receipts: RwLock::new(HashMap::new()),
            db: db.clone(),
            cache_man: Mutex::new(cache_man),
            pending_best_block: RwLock::new(None),
            pending_block_hashes: RwLock::new(HashMap::new()),
            pending_block_details: RwLock::new(HashMap::new()),
            pending_transaction_addresses: RwLock::new(HashMap::new()),
        };

        // load best block
        let best_block_hash = match bc
            .db
            .get(db::COL_EXTRA, b"best")
            .expect("EXTRA db not be found")
        {
            Some(best) => H256::from_slice(&best),
            None => {
                // best block does not exist
                // we need to insert genesis into the cache
                let block = BlockView::new(genesis);
                let header = block.header_view();
                let hash = block.hash();

                let details = BlockDetails {
                    number: header.number(),
                    total_difficulty: header.difficulty(),
                    pow_total_difficulty: header.difficulty(),
                    pos_total_difficulty: Default::default(),
                    parent: header.parent_hash(),
                    children: vec![],
                    anti_seal_parent: H256::default(),
                };

                let mut batch = DBTransaction::new();
                batch.put(db::COL_HEADERS, &hash, block.header_rlp().as_raw());
                batch.put(db::COL_BODIES, &hash, &Self::block_to_body(genesis));

                batch.write(db::COL_EXTRA, &hash, &details);
                batch.write(db::COL_EXTRA, &header.number(), &hash);

                batch.put(db::COL_EXTRA, b"best", &hash);
                bc.db
                    .write(batch)
                    .expect("Low level database error. Some issue with disk?");
                hash
            }
        };

        {
            // Fetch best block details
            let best_block_number = bc
                .block_number(&best_block_hash)
                .expect("best block not found, db may crashed");
            let best_block_details = bc
                .block_details(&best_block_hash)
                .expect("best block not found, db may crashed");
            let best_block_total_difficulty = best_block_details.total_difficulty;
            let best_block_pow_total_difficulty = best_block_details.pow_total_difficulty;
            let best_block_pos_total_difficulty = best_block_details.pos_total_difficulty;
            let best_block_rlp = bc
                .block(&best_block_hash)
                .expect("best block not found, db may crashed")
                .into_inner();
            let best_block_timestamp = BlockView::new(&best_block_rlp).header().timestamp();

            let raw_first = bc
                .db
                .get(db::COL_EXTRA, b"first")
                .expect("EXTRA db not be found")
                .map(|v| v.into_vec());
            let mut best_ancient = bc
                .db
                .get(db::COL_EXTRA, b"ancient")
                .expect("EXTRA db not be found")
                .map(|h| H256::from_slice(&h));
            let best_ancient_number;
            if best_ancient.is_none() && best_block_number > 1 && bc.block_hash(1).is_none() {
                best_ancient = Some(bc.genesis_hash());
                best_ancient_number = Some(0);
            } else {
                best_ancient_number = best_ancient.as_ref().and_then(|h| bc.block_number(h));
            }

            // binary search for the first block.
            match raw_first {
                None => {
                    let (mut f, mut hash) = (best_block_number, best_block_hash);
                    let mut l = best_ancient_number.unwrap_or(0);

                    loop {
                        if l >= f {
                            break;
                        }

                        let step = (f - l) >> 1;
                        let m = l + step;

                        match bc.block_hash(m) {
                            Some(h) => {
                                f = m;
                                hash = h
                            }
                            None => l = m + 1,
                        }
                    }

                    if hash != bc.genesis_hash() {
                        trace!(target:"blockchain","First block calculated: {:?}", hash);
                        let mut batch = DBTransaction::new();
                        batch.put(db::COL_EXTRA, b"first", &hash);
                        db.write(batch).expect("Low level database error.");
                        bc.first_block = Some(hash);
                    }
                }
                Some(raw_first) => {
                    bc.first_block = Some(H256::from_slice(&raw_first));
                }
            }

            // and write them
            let mut best_block = bc.best_block.write();
            *best_block = BestBlock {
                number: best_block_number,
                total_difficulty: best_block_total_difficulty,
                pow_total_difficulty: best_block_pow_total_difficulty,
                pos_total_difficulty: best_block_pos_total_difficulty,
                hash: best_block_hash,
                timestamp: best_block_timestamp,
                block: best_block_rlp,
            };

            if let (Some(hash), Some(number)) = (best_ancient, best_ancient_number) {
                let mut best_ancient_block = bc.best_ancient_block.write();
                *best_ancient_block = Some(BestAncientBlock {
                    hash: hash,
                    number: number,
                });
            }
        }

        bc
    }

    /// Returns true if the given parent block has given child
    /// (though not necessarily a part of the canon chain).
    fn is_known_child(&self, parent: &H256, hash: &H256) -> bool {
        self.db
            .read_with_cache(db::COL_EXTRA, &self.block_details, parent)
            .map_or(false, |d| d.children.contains(hash))
    }

    /// Returns a tree route between `from` and `to`, which is a tuple of:
    ///
    /// - a vector of hashes of all blocks, ordered from `from` to `to`.
    ///
    /// - common ancestor of these blocks.
    ///
    /// - an index where best common ancestor would be
    ///
    /// 1.) from newer to older
    ///
    /// - bc: `A1 -> A2 -> A3 -> A4 -> A5`
    /// - from: A5, to: A4
    /// - route:
    ///
    ///   ```json
    ///   { blocks: [A5], ancestor: A4, index: 1 }
    ///   ```
    ///
    /// 2.) from older to newer
    ///
    /// - bc: `A1 -> A2 -> A3 -> A4 -> A5`
    /// - from: A3, to: A4
    /// - route:
    ///
    ///   ```json
    ///   { blocks: [A4], ancestor: A3, index: 0 }
    ///   ```
    ///
    /// 3.) fork:
    ///
    /// - bc:
    ///
    ///   ```text
    ///   A1 -> A2 -> A3 -> A4
    ///              -> B3 -> B4
    ///   ```
    /// - from: B4, to: A4
    /// - route:
    ///
    ///   ```json
    ///   { blocks: [B4, B3, A3, A4], ancestor: A2, index: 2 }
    ///   ```
    ///
    /// If the tree route verges into pruned or unknown blocks,
    /// `None` is returned.
    pub fn tree_route(&self, from: H256, to: H256) -> Option<TreeRoute> {
        let mut from_branch = vec![];
        let mut to_branch = vec![];

        let mut from_details = self.block_details(&from)?;
        let mut to_details = self.block_details(&to)?;
        let mut current_from = from;
        let mut current_to = to;

        // reset from && to to the same level
        while from_details.number > to_details.number {
            from_branch.push(current_from);
            current_from = from_details.parent.clone();
            from_details = self.block_details(&from_details.parent)?;
        }

        while to_details.number > from_details.number {
            to_branch.push(current_to);
            current_to = to_details.parent.clone();
            to_details = self.block_details(&to_details.parent)?;
        }

        assert_eq!(from_details.number, to_details.number);

        // move to shared parent
        while current_from != current_to {
            from_branch.push(current_from);
            current_from = from_details.parent.clone();
            from_details = self.block_details(&from_details.parent)?;

            to_branch.push(current_to);
            current_to = to_details.parent.clone();
            to_details = self.block_details(&to_details.parent)?;
        }

        let index = from_branch.len();

        from_branch.extend(to_branch.into_iter().rev());

        Some(TreeRoute {
            blocks: from_branch,
            ancestor: current_from,
            index: index,
        })
    }

    #[cfg(test)]
    /// Inserts a verified, known block from the canonical chain.
    ///
    /// Can be performed out-of-order, but care must be taken that the final chain is in a correct state.
    /// `is_best` forces the best block to be updated to this block.
    /// `is_ancient` forces the best block of the first block sequence to be updated to this block.
    /// `parent_td` is a parent total diffuculty
    /// Supply a dummy parent total difficulty when the parent block may not be in the chain.
    /// Returns true if the block is disconnected.
    pub fn insert_unordered_block(
        &self,
        batch: &mut DBTransaction,
        bytes: &[u8],
        receipts: Vec<Receipt>,
        parent_pow_td: Option<U256>,
        parent_pos_td: Option<U256>,
        is_best: bool,
        is_ancient: bool,
    ) -> bool
    {
        let block = BlockView::new(bytes);
        let header = block.header_view();
        let hash = header.hash();

        if self.is_known(&hash) {
            return false;
        }

        assert!(self.pending_best_block.read().is_none());

        let compressed_header = compress(block.header_rlp().as_raw(), blocks_swapper());
        let compressed_body = compress(&Self::block_to_body(bytes), blocks_swapper());

        // store block in db
        batch.put(db::COL_HEADERS, &hash, &compressed_header);
        batch.put(db::COL_BODIES, &hash, &compressed_body);

        let maybe_parent = self.block_details(&header.parent_hash());

        if let Some(parent_details) = maybe_parent {
            // parent known to be in chain.
            let (pow_td, pos_td) = match header.seal_type().unwrap_or_default() {
                SealType::PoW => {
                    (
                        parent_details.pow_total_difficulty + header.difficulty(),
                        parent_details.pos_total_difficulty,
                    )
                }
                SealType::PoS => {
                    (
                        parent_details.pow_total_difficulty,
                        parent_details.pos_total_difficulty + header.difficulty(),
                    )
                }
            };
            let info = BlockInfo {
                hash: hash,
                number: header.number(),
                // TODO-UNITY: add overflow check
                total_difficulty: pow_td * ::std::cmp::max(pos_td, U256::from(1u64)),
                pow_total_difficulty: pow_td,
                pos_total_difficulty: pos_td,
                location: BlockLocation::CanonChain,
            };

            self.prepare_update(
                batch,
                ExtrasUpdate {
                    block_hashes: self.prepare_block_hashes_update(bytes, &info),
                    block_details: self.prepare_block_details_update(bytes, &info),
                    block_receipts: self.prepare_block_receipts_update(receipts, &info),
                    blocks_blooms: self.prepare_block_blooms_update(bytes, &info),
                    transactions_addresses: self.prepare_transaction_addresses_update(bytes, &info),
                    info: info,
                    timestamp: header.timestamp(),
                    block: bytes,
                },
                is_best,
            );

            if is_ancient {
                let mut best_ancient_block = self.best_ancient_block.write();
                let ancient_number = best_ancient_block.as_ref().map_or(0, |b| b.number);
                if self.block_hash(header.number() + 1).is_some() {
                    batch.delete(db::COL_EXTRA, b"ancient");
                    *best_ancient_block = None;
                } else if header.number() > ancient_number {
                    batch.put(db::COL_EXTRA, b"ancient", &hash);
                    *best_ancient_block = Some(BestAncientBlock {
                        hash: hash,
                        number: header.number(),
                    });
                }
            }

            false
        } else {
            // parent not in the chain yet. we need the parent difficulty to proceed.
            let pow_d = parent_pow_td.expect(
                "parent PoW total difficulty always supplied for first block in chunk. only first \
                 block can have missing parent; qed",
            );
            let pos_d = parent_pos_td.expect(
                "parent PoS total difficulty always supplied for first block in chunk. only first \
                 block can have missing parent; qed",
            );

            let (pow_td, pos_td) = match header.seal_type().unwrap_or_default() {
                SealType::PoW => (pow_d + header.difficulty(), pos_d),
                SealType::PoS => (pow_d, pos_d + header.difficulty()),
            };

            let info = BlockInfo {
                hash: hash,
                number: header.number(),
                // TODO-UNITY: add overflow check
                total_difficulty: pow_td * ::std::cmp::max(pos_td, U256::from(1u64)),
                pow_total_difficulty: pow_td,
                pos_total_difficulty: pos_td,
                location: BlockLocation::CanonChain,
            };

            let block_details = BlockDetails {
                number: header.number(),
                total_difficulty: info.total_difficulty,
                pow_total_difficulty: info.pow_total_difficulty,
                pos_total_difficulty: info.pos_total_difficulty,
                parent: header.parent_hash(),
                children: Vec::new(),
                anti_seal_parent: H256::default(),
            };

            let mut update = HashMap::new();
            update.insert(hash, block_details);

            self.prepare_update(
                batch,
                ExtrasUpdate {
                    block_hashes: self.prepare_block_hashes_update(bytes, &info),
                    block_details: update,
                    block_receipts: self.prepare_block_receipts_update(receipts, &info),
                    blocks_blooms: self.prepare_block_blooms_update(bytes, &info),
                    transactions_addresses: self.prepare_transaction_addresses_update(bytes, &info),
                    info: info,
                    timestamp: header.timestamp(),
                    block: bytes,
                },
                is_best,
            );
            true
        }
    }

    /// Inserts the block into backing cache database.
    /// Expects the block to be valid and already verified.
    /// If the block is already known, does nothing.
    pub fn insert_block(
        &self,
        batch: &mut DBTransaction,
        bytes: &[u8],
        receipts: Vec<Receipt>,
    ) -> ImportRoute
    {
        // create views onto rlp
        let block = BlockView::new(bytes);
        let header = block.header_view();
        let hash = header.hash();

        if self.is_known_child(&header.parent_hash(), &hash) {
            return ImportRoute::none();
        }

        assert!(self.pending_best_block.read().is_none());

        let compressed_header = compress(block.header_rlp().as_raw(), blocks_swapper());
        let compressed_body = compress(&Self::block_to_body(bytes), blocks_swapper());

        // store block in db
        batch.put(db::COL_HEADERS, &hash, &compressed_header);
        batch.put(db::COL_BODIES, &hash, &compressed_body);

        let info = self.block_info(&header);

        if let BlockLocation::BranchBecomingCanonChain(ref d) = info.location {
            info!(target: "reorg", "Reorg to {} ({} {} {})",
                Colour::Yellow.bold().paint(format!("#{} {}", info.number, info.hash)),
                Colour::Red.paint(d.retracted.iter().join(" ")),
                Colour::White.paint(format!("#{} {}", self.block_details(&d.ancestor).expect("`ancestor` is in the route; qed").number, d.ancestor)),
                Colour::Green.paint(d.enacted.iter().join(" "))
            );
        }

        self.prepare_update(
            batch,
            ExtrasUpdate {
                block_hashes: self.prepare_block_hashes_update(bytes, &info),
                block_details: self.prepare_block_details_update(bytes, &info),
                block_receipts: self.prepare_block_receipts_update(receipts, &info),
                blocks_blooms: self.prepare_block_blooms_update(bytes, &info),
                transactions_addresses: self.prepare_transaction_addresses_update(bytes, &info),
                info: info.clone(),
                timestamp: header.timestamp(),
                block: bytes,
            },
            true,
        );

        ImportRoute::from(info)
    }

    /// Get inserted block info which is critical to prepare extras updates.
    fn block_info(&self, header: &HeaderView) -> BlockInfo {
        let hash = header.hash();
        let number = header.number();
        let parent_hash = header.parent_hash();
        let parent_details = self
            .block_details(&parent_hash)
            .unwrap_or_else(|| panic!("Invalid parent hash: {:?}", parent_hash));

        let (pow_td, pos_td) = match header.seal_type().unwrap_or_default() {
            SealType::PoW => {
                (
                    parent_details.pow_total_difficulty + header.difficulty(),
                    parent_details.pos_total_difficulty,
                )
            }
            SealType::PoS => {
                (
                    parent_details.pow_total_difficulty,
                    parent_details.pos_total_difficulty + header.difficulty(),
                )
            }
        };

        // TODO-UNITY: add overflow check
        let td = pow_td * pos_td;

        let is_new_best = td > self.best_block_total_difficulty();

        BlockInfo {
            hash: hash,
            number: number,
            total_difficulty: td,
            pow_total_difficulty: pow_td,
            pos_total_difficulty: pos_td,
            location: if is_new_best {
                // on new best block we need to make sure that all ancestors
                // are moved to "canon chain"
                // find the route between old best block and the new one
                let best_hash = self.best_block_hash();
                let route = self
                    .tree_route(best_hash, parent_hash)
                    .expect("blocks being imported always within recent history; qed");

                assert_eq!(number, parent_details.number + 1);

                match route.blocks.len() {
                    0 => BlockLocation::CanonChain,
                    _ => {
                        let retracted = route
                            .blocks
                            .iter()
                            .take(route.index)
                            .cloned()
                            .collect::<Vec<_>>()
                            .into_iter()
                            .collect::<Vec<_>>();
                        let enacted = route
                            .blocks
                            .into_iter()
                            .skip(route.index)
                            .collect::<Vec<_>>();
                        BlockLocation::BranchBecomingCanonChain(BranchBecomingCanonChainData {
                            ancestor: route.ancestor,
                            enacted: enacted,
                            retracted: retracted,
                        })
                    }
                }
            } else {
                BlockLocation::Branch
            },
        }
    }

    /// Prepares extras update.
    fn prepare_update(&self, batch: &mut DBTransaction, update: ExtrasUpdate, is_best: bool) {
        {
            let mut write_receipts = self.block_receipts.write();
            batch.extend_with_cache(
                db::COL_EXTRA,
                &mut *write_receipts,
                update.block_receipts,
                CacheUpdatePolicy::Remove,
            );
        }

        {
            let mut write_blocks_blooms = self.blocks_blooms.write();
            // update best block
            match update.info.location {
                BlockLocation::Branch => (),
                BlockLocation::BranchBecomingCanonChain(_) => {
                    // clear all existing blooms, cause they may be created for block
                    // number higher than current best block
                    *write_blocks_blooms = update.blocks_blooms;
                    for (key, value) in write_blocks_blooms.iter() {
                        batch.write(db::COL_EXTRA, key, value);
                    }
                }
                BlockLocation::CanonChain => {
                    // update all existing blooms groups
                    for (key, value) in update.blocks_blooms {
                        match write_blocks_blooms.entry(key) {
                            hash_map::Entry::Occupied(mut entry) => {
                                entry.get_mut().accrue_bloom_group(&value);
                                batch.write(db::COL_EXTRA, entry.key(), entry.get());
                            }
                            hash_map::Entry::Vacant(entry) => {
                                batch.write(db::COL_EXTRA, entry.key(), &value);
                                entry.insert(value);
                            }
                        }
                    }
                }
            }
        }

        // These cached values must be updated last with all four locks taken to avoid
        // cache decoherence
        {
            let mut best_block = self.pending_best_block.write();
            if is_best && update.info.location != BlockLocation::Branch {
                batch.put(db::COL_EXTRA, b"best", &update.info.hash);
                *best_block = Some(BestBlock {
                    hash: update.info.hash,
                    number: update.info.number,
                    total_difficulty: update.info.total_difficulty,
                    pow_total_difficulty: update.info.pow_total_difficulty,
                    pos_total_difficulty: update.info.pos_total_difficulty,
                    timestamp: update.timestamp,
                    block: update.block.to_vec(),
                });
            }

            let mut write_hashes = self.pending_block_hashes.write();
            let mut write_details = self.pending_block_details.write();
            let mut write_txs = self.pending_transaction_addresses.write();

            batch.extend_with_cache(
                db::COL_EXTRA,
                &mut *write_details,
                update.block_details,
                CacheUpdatePolicy::Overwrite,
            );
            batch.extend_with_cache(
                db::COL_EXTRA,
                &mut *write_hashes,
                update.block_hashes,
                CacheUpdatePolicy::Overwrite,
            );
            batch.extend_with_option_cache(
                db::COL_EXTRA,
                &mut *write_txs,
                update.transactions_addresses,
                CacheUpdatePolicy::Overwrite,
            );
        }
    }

    /// Apply pending insertion updates
    pub fn commit(&self) {
        let mut pending_best_block = self.pending_best_block.write();
        let mut pending_write_hashes = self.pending_block_hashes.write();
        let mut pending_block_details = self.pending_block_details.write();
        let mut pending_write_txs = self.pending_transaction_addresses.write();

        let mut best_block = self.best_block.write();
        let mut write_block_details = self.block_details.write();
        let mut write_hashes = self.block_hashes.write();
        let mut write_txs = self.transaction_addresses.write();
        // update best block
        if let Some(block) = pending_best_block.take() {
            *best_block = block;
        }

        let pending_txs = mem::replace(&mut *pending_write_txs, HashMap::new());
        let (retracted_txs, enacted_txs) = pending_txs
            .into_iter()
            .partition::<HashMap<_, _>, _>(|&(_, ref value)| value.is_none());

        let pending_hashes_keys: Vec<_> = pending_write_hashes.keys().cloned().collect();
        let enacted_txs_keys: Vec<_> = enacted_txs.keys().cloned().collect();
        let pending_block_hashes: Vec<_> = pending_block_details.keys().cloned().collect();

        write_hashes.extend(mem::replace(&mut *pending_write_hashes, HashMap::new()));
        write_txs.extend(
            enacted_txs
                .into_iter()
                .map(|(k, v)| (k, v.expect("Transactions were partitioned; qed"))),
        );
        write_block_details.extend(mem::replace(&mut *pending_block_details, HashMap::new()));

        for hash in retracted_txs.keys() {
            write_txs.remove(hash);
        }

        let mut cache_man = self.cache_man.lock();
        for n in pending_hashes_keys {
            cache_man.note_used(CacheId::BlockHashes(n));
        }

        for hash in enacted_txs_keys {
            cache_man.note_used(CacheId::TransactionAddresses(hash));
        }

        for hash in pending_block_hashes {
            cache_man.note_used(CacheId::BlockDetails(hash));
        }
    }

    /// Iterator that lists `first` and then all of `first`'s ancestors, by hash.
    #[cfg(test)]
    pub fn ancestry_iter(&self, first: H256) -> Option<AncestryIter> {
        if self.is_known(&first) {
            Some(AncestryIter {
                current: first,
                chain: self,
            })
        } else {
            None
        }
    }

    /// This function returns modified block hashes.
    fn prepare_block_hashes_update(
        &self,
        block_bytes: &[u8],
        info: &BlockInfo,
    ) -> HashMap<BlockNumber, H256>
    {
        let mut block_hashes = HashMap::new();
        let block = BlockView::new(block_bytes);
        let header = block.header_view();
        let number = header.number();

        match info.location {
            BlockLocation::Branch => (),
            BlockLocation::CanonChain => {
                block_hashes.insert(number, info.hash);
            }
            BlockLocation::BranchBecomingCanonChain(ref data) => {
                let ancestor_number = self
                    .block_number(&data.ancestor)
                    .expect("Block number of ancestor is always in DB");
                let start_number = ancestor_number + 1;

                for (index, hash) in data.enacted.iter().cloned().enumerate() {
                    block_hashes.insert(start_number + index as BlockNumber, hash);
                }

                block_hashes.insert(number, info.hash);
            }
        }

        block_hashes
    }

    /// This function returns modified block details.
    /// Uses the given parent details or attempts to load them from the database.
    fn prepare_block_details_update(
        &self,
        block_bytes: &[u8],
        info: &BlockInfo,
    ) -> HashMap<H256, BlockDetails>
    {
        let block = BlockView::new(block_bytes);
        let header = block.header_view();
        let parent_hash = header.parent_hash();
        let mut parent_details = self
            .block_details(&parent_hash)
            .unwrap_or_else(|| panic!("Invalid parent hash: {:?}", parent_hash));
        parent_details.children.push(info.hash);

        // Set anti seal parent
        let seal_type = header.seal_type().to_owned().unwrap_or_default();
        let parent_header = self
            .block_header_data(&parent_hash)
            .expect("block's should always have a parent.");
        let parent_seal_type: SealType = parent_header.seal_type().to_owned().unwrap_or_default();
        let anti_seal_parent_hash: H256 = if seal_type == parent_seal_type {
            parent_details.anti_seal_parent.to_owned()
        } else {
            parent_hash
        };

        // create current block details.
        let details = BlockDetails {
            number: header.number(),
            total_difficulty: info.total_difficulty,
            pow_total_difficulty: info.pow_total_difficulty,
            pos_total_difficulty: info.pos_total_difficulty,
            parent: parent_hash,
            children: vec![],
            anti_seal_parent: anti_seal_parent_hash,
        };

        // write to batch
        let mut block_details = HashMap::new();
        block_details.insert(parent_hash, parent_details);
        block_details.insert(info.hash, details);
        block_details
    }

    /// This function returns modified block receipts.
    fn prepare_block_receipts_update(
        &self,
        receipts: Vec<Receipt>,
        info: &BlockInfo,
    ) -> HashMap<H256, BlockReceipts>
    {
        let mut block_receipts = HashMap::new();
        block_receipts.insert(info.hash, BlockReceipts::new(receipts));
        block_receipts
    }

    /// This function returns modified transaction addresses.
    fn prepare_transaction_addresses_update(
        &self,
        block_bytes: &[u8],
        info: &BlockInfo,
    ) -> HashMap<H256, Option<TransactionAddress>>
    {
        let block = BlockView::new(block_bytes);
        let transaction_hashes = block.transaction_hashes();

        match info.location {
            BlockLocation::CanonChain => {
                transaction_hashes
                    .into_iter()
                    .enumerate()
                    .map(|(i, tx_hash)| {
                        (
                            tx_hash,
                            Some(TransactionAddress {
                                block_hash: info.hash,
                                index: i,
                            }),
                        )
                    })
                    .collect()
            }
            BlockLocation::BranchBecomingCanonChain(ref data) => {
                let addresses = data.enacted.iter().flat_map(|hash| {
                    let body = self
                        .block_body(hash)
                        .expect("Enacted block must be in database.");
                    let hashes = body.transaction_hashes();
                    hashes
                        .into_iter()
                        .enumerate()
                        .map(|(i, tx_hash)| {
                            (
                                tx_hash,
                                Some(TransactionAddress {
                                    block_hash: *hash,
                                    index: i,
                                }),
                            )
                        })
                        .collect::<HashMap<H256, Option<TransactionAddress>>>()
                });

                let current_addresses =
                    transaction_hashes
                        .into_iter()
                        .enumerate()
                        .map(|(i, tx_hash)| {
                            (
                                tx_hash,
                                Some(TransactionAddress {
                                    block_hash: info.hash,
                                    index: i,
                                }),
                            )
                        });

                let retracted = data.retracted.iter().flat_map(|hash| {
                    let body = self
                        .block_body(hash)
                        .expect("Retracted block must be in database.");
                    let hashes = body.transaction_hashes();
                    hashes
                        .into_iter()
                        .map(|hash| (hash, None))
                        .collect::<HashMap<H256, Option<TransactionAddress>>>()
                });

                // The order here is important! Don't remove transaction if it was part of enacted blocks as well.
                retracted
                    .chain(addresses)
                    .chain(current_addresses)
                    .collect()
            }
            BlockLocation::Branch => HashMap::new(),
        }
    }

    /// This functions returns modified blocks blooms.
    ///
    /// To accelerate blooms lookups, blomms are stored in multiple
    /// layers (BLOOM_LEVELS, currently 3).
    /// ChainFilter is responsible for building and rebuilding these layers.
    /// It returns them in HashMap, where values are Blooms and
    /// keys are BloomIndexes. BloomIndex represents bloom location on one
    /// of these layers.
    ///
    /// To reduce number of queries to databse, block blooms are stored
    /// in BlocksBlooms structure which contains info about several
    /// (BLOOM_INDEX_SIZE, currently 16) consecutive blocks blooms.
    ///
    /// Later, BloomIndexer is used to map bloom location on filter layer (BloomIndex)
    /// to bloom location in database (BlocksBloomLocation).
    ///
    fn prepare_block_blooms_update(
        &self,
        block_bytes: &[u8],
        info: &BlockInfo,
    ) -> HashMap<GroupPosition, BloomGroup>
    {
        let block = BlockView::new(block_bytes);
        let header = block.header_view();

        let log_blooms = match info.location {
            BlockLocation::Branch => HashMap::new(),
            BlockLocation::CanonChain => {
                let log_bloom = header.log_bloom();
                if log_bloom.is_zero() {
                    HashMap::new()
                } else {
                    let chain = bc::group::BloomGroupChain::new(self.blooms_config, self);
                    chain.insert(info.number as bc::Number, log_bloom)
                }
            }
            BlockLocation::BranchBecomingCanonChain(ref data) => {
                let ancestor_number = self
                    .block_number(&data.ancestor)
                    .expect("block ancestor not found, db may crashed");
                let start_number = ancestor_number + 1;
                let range = start_number as bc::Number..self.best_block_number() as bc::Number;

                let mut blooms: Vec<Bloom> = data
                    .enacted
                    .iter()
                    .map(|hash| {
                        self.block_header_data(hash)
                            .expect("block ancestor not found, db may crashed")
                    })
                    .map(|h| h.log_bloom())
                    .collect();

                blooms.push(header.log_bloom());

                let chain = bc::group::BloomGroupChain::new(self.blooms_config, self);
                chain.replace(&range, blooms)
            }
        };

        log_blooms
            .into_iter()
            .map(|p| (From::from(p.0), From::from(p.1)))
            .collect()
    }

    /// Get best block hash.
    pub fn best_block_hash(&self) -> H256 { self.best_block.read().hash }

    /// Get best block number.
    pub fn best_block_number(&self) -> BlockNumber { self.best_block.read().number }

    /// Get best block timestamp.
    pub fn best_block_timestamp(&self) -> u64 { self.best_block.read().timestamp }

    /// Get best block total difficulty.
    pub fn best_block_total_difficulty(&self) -> U256 { self.best_block.read().total_difficulty }

    /// Get best block header
    pub fn best_block_header(&self) -> encoded::Header {
        let block = self.best_block.read();
        let raw = BlockView::new(&block.block)
            .header_view()
            .rlp()
            .as_raw()
            .to_vec();
        encoded::Header::new(raw)
    }

    /// Get current cache size.
    pub fn cache_size(&self) -> CacheSize {
        CacheSize {
            blocks: self.block_headers.read().heap_size_of_children()
                + self.block_bodies.read().heap_size_of_children(),
            block_details: self.block_details.read().heap_size_of_children(),
            transaction_addresses: self.transaction_addresses.read().heap_size_of_children(),
            blocks_blooms: self.blocks_blooms.read().heap_size_of_children(),
            block_receipts: self.block_receipts.read().heap_size_of_children(),
        }
    }

    /// Ticks our cache system and throws out any old data.
    pub fn collect_garbage(&self) {
        let current_size = self.cache_size().total();

        let mut block_headers = self.block_headers.write();
        let mut block_bodies = self.block_bodies.write();
        let mut block_details = self.block_details.write();
        let mut block_hashes = self.block_hashes.write();
        let mut transaction_addresses = self.transaction_addresses.write();
        let mut blocks_blooms = self.blocks_blooms.write();
        let mut block_receipts = self.block_receipts.write();

        let mut cache_man = self.cache_man.lock();
        cache_man.collect_garbage(current_size, |ids| {
            for id in &ids {
                match *id {
                    CacheId::BlockHeader(ref h) => {
                        block_headers.remove(h);
                    }
                    CacheId::BlockBody(ref h) => {
                        block_bodies.remove(h);
                    }
                    CacheId::BlockDetails(ref h) => {
                        block_details.remove(h);
                    }
                    CacheId::BlockHashes(ref h) => {
                        block_hashes.remove(h);
                    }
                    CacheId::TransactionAddresses(ref h) => {
                        transaction_addresses.remove(h);
                    }
                    CacheId::BlocksBlooms(ref h) => {
                        blocks_blooms.remove(h);
                    }
                    CacheId::BlockReceipts(ref h) => {
                        block_receipts.remove(h);
                    }
                }
            }

            block_headers.shrink_to_fit();
            block_bodies.shrink_to_fit();
            block_details.shrink_to_fit();
            block_hashes.shrink_to_fit();
            transaction_addresses.shrink_to_fit();
            blocks_blooms.shrink_to_fit();
            block_receipts.shrink_to_fit();

            block_headers.heap_size_of_children()
                + block_bodies.heap_size_of_children()
                + block_details.heap_size_of_children()
                + block_hashes.heap_size_of_children()
                + transaction_addresses.heap_size_of_children()
                + blocks_blooms.heap_size_of_children()
                + block_receipts.heap_size_of_children()
        });
    }

    /// Create a block body from a block.
    pub fn block_to_body(block: &[u8]) -> Bytes {
        let mut body = RlpStream::new_list(1);
        let block_rlp = Rlp::new(block);
        body.append_raw(block_rlp.at(1).as_raw(), 1);
        body.out()
    }

    /// Returns general blockchain information
    pub fn chain_info(&self) -> BlockChainInfo {
        // ensure data consistencly by locking everything first
        let best_block = self.best_block.read();
        let best_ancient_block = self.best_ancient_block.read();
        BlockChainInfo {
            total_difficulty: best_block.total_difficulty.clone(),
            pow_total_difficulty: best_block.pow_total_difficulty.clone(),
            pos_total_difficulty: best_block.pos_total_difficulty.clone(),
            pending_total_difficulty: best_block.total_difficulty.clone(),
            genesis_hash: self.genesis_hash(),
            best_block_hash: best_block.hash,
            best_block_number: best_block.number,
            best_block_timestamp: best_block.timestamp,
            first_block_hash: self.first_block(),
            first_block_number: From::from(self.first_block_number()),
            ancient_block_hash: best_ancient_block.as_ref().map(|b| b.hash),
            ancient_block_number: best_ancient_block.as_ref().map(|b| b.number),
        }
    }
}
