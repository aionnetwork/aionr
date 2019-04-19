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

//! Light client header chain.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::cache::Cache;
use super::cht;
use super::BlockChainInfo;
use aion_types::{H256, H264, U256};
use encoded;
use engines::epoch::{PendingTransition as PendingEpochTransition, Transition as EpochTransition};
use error::BlockError;
use header::Header;
use heapsize::HeapSizeOf;
use kvdb::{DBTransaction, DatabaseConfig, DbRepository, KeyValueDB, RepositoryConfig};
use parking_lot::{Mutex, RwLock};
use plain_hasher::PlainHasher;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};
use smallvec::SmallVec;
use spec::Spec;
use std::collections::HashMap;
use std::hash;
use std::path::Path;
use std::time::Duration;
use types::block_status::BlockStatus;
use types::ids::BlockId;

pub type H256FastMap<T> = HashMap<H256, T, hash::BuildHasherDefault<PlainHasher>>;

const COL: &'static str = "header_chain";

/// Store at least this many candidate headers at all times.
/// Also functions as the delay for computing CHTs as they aren't
/// relevant to any blocks we've got in memory.
const HISTORY: u64 = 4096;

/// The best block key. Maps to an RLP list: [best_era, last_era]
const CURRENT_KEY: &[u8] = &*b"best_and_latest";

/// Key storing the last canonical epoch transition.
const LAST_CANONICAL_TRANSITION: &[u8] = &*b"canonical_transition";

/// Information about a block.
#[derive(Debug, Clone)]
pub struct BlockDescriptor {
    /// The block's hash
    pub hash: H256,
    /// The block's number
    pub number: u64,
    /// The block's total difficulty.
    pub total_difficulty: U256,
}

// best block data
#[derive(RlpEncodable, RlpDecodable)]
struct BestAndLatest {
    best_num: u64,
    latest_num: u64,
}

impl BestAndLatest {
    fn new(best_num: u64, latest_num: u64) -> Self {
        BestAndLatest {
            best_num,
            latest_num,
        }
    }
}

// candidate block description.
struct Candidate {
    hash: H256,
    parent_hash: H256,
    total_difficulty: U256,
}

struct Entry {
    candidates: SmallVec<[Candidate; 3]>, // 3 arbitrarily chosen
    canonical_hash: H256,
}

impl HeapSizeOf for Entry {
    fn heap_size_of_children(&self) -> usize {
        if self.candidates.spilled() {
            self.candidates.capacity() * ::std::mem::size_of::<Candidate>()
        } else {
            0
        }
    }
}

impl Encodable for Entry {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(self.candidates.len());

        for candidate in &self.candidates {
            s.begin_list(3)
                .append(&candidate.hash)
                .append(&candidate.parent_hash)
                .append(&candidate.total_difficulty);
        }
    }
}

impl Decodable for Entry {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let mut candidates = SmallVec::<[Candidate; 3]>::new();

        for item in rlp.iter() {
            candidates.push(Candidate {
                hash: item.val_at(0)?,
                parent_hash: item.val_at(1)?,
                total_difficulty: item.val_at(2)?,
            })
        }

        if candidates.is_empty() {
            return Err(DecoderError::Custom("Empty candidates vector submitted."));
        }

        // rely on the invariant that the canonical entry is always first.
        let canon_hash = candidates[0].hash;
        Ok(Entry {
            candidates,
            canonical_hash: canon_hash,
        })
    }
}

fn cht_key(number: u64) -> String { format!("{:08x}_canonical", number) }

fn era_key(number: u64) -> String { format!("candidates_{}", number) }

fn transition_key(block_hash: H256) -> H264 {
    const LEADING: u8 = 2;

    let mut key = H264::default();

    key[0] = LEADING;
    key.0[1..].copy_from_slice(&block_hash.0[..]);

    key
}

// encode last canonical transition entry: header and proof.
fn encode_canonical_transition(header: &Header, proof: &[u8]) -> Vec<u8> {
    let mut stream = RlpStream::new_list(2);
    stream.append(header).append(&proof);
    stream.out()
}

/// Pending changes from `insert` to be applied after the database write has finished.
pub struct PendingChanges {
    best_block: Option<BlockDescriptor>, // new best block.
}

/// Whether or not the hardcoded sync feature is allowed.
pub enum HardcodedSync {
    Allow,
    Deny,
}

/// Header chain. See module docs for more details.
pub struct HeaderChain {
    genesis_header: encoded::Header, // special-case the genesis.
    candidates: RwLock<BTreeMap<u64, Entry>>,
    best_block: RwLock<BlockDescriptor>,
    live_epoch_proofs: RwLock<H256FastMap<EpochTransition>>,
    db: Arc<KeyValueDB>,
    cache: Arc<Mutex<Cache>>,
}

impl HeaderChain {
    /// Create a new header chain given this genesis block and database to read from.
    pub fn new(client_path: &Path, spec: &Spec) -> Result<Self, String> {
        let mut live_epoch_proofs = ::std::collections::HashMap::default();
        let cache = Arc::new(Mutex::new(Cache::new(
            Default::default(),
            Duration::from_secs(900),
        )));

        let genesis = ::rlp::encode(&spec.genesis_header());
        let decoded_header = spec.genesis_header();

        let db_path = client_path.join(COL);
        let mut db_configs = Vec::new();
        db_configs.push(RepositoryConfig {
            db_name: COL.to_string(),
            db_config: DatabaseConfig::default(),
            db_path: db_path.to_string_lossy().into(),
        });

        let header_chain_db = DbRepository::init(db_configs)
            .map_err(|e| format!("Unable to initialize DB: {}", e))?;
        let db = Arc::new(header_chain_db);

        let chain = if let Some(current) = db.get(COL, CURRENT_KEY).unwrap_or(None) {
            let curr_rlp = UntrustedRlp::new(current.as_ref());
            let curr_value = curr_rlp.as_val().expect("decoding db value failed");
            let curr: BestAndLatest = curr_value;
            //let curr: BestAndLatest = ::rlp::decode(&current).expect("decoding db value failed");

            let mut cur_number = curr.latest_num;
            let mut candidates = BTreeMap::new();

            // load all era entries, referenced headers within them,
            // and live epoch proofs.
            while let Some(entry) = db.get(COL, era_key(cur_number).as_bytes()).unwrap_or(None) {
                let entry_rlp = UntrustedRlp::new(entry.as_ref());
                let entry_value = entry_rlp.as_val().expect("decoding db value failed");
                let entry: Entry = entry_value;

                //let entry: Entry = ::rlp::decode(&entry).expect("decoding db value failed");
                trace!(target: "chain", "loaded header chain entry for era {} with {} candidates",
					cur_number, entry.candidates.len());

                for c in &entry.candidates {
                    let key = transition_key(c.hash);

                    if let Some(proof) = db.get(COL, &*key).unwrap_or(None) {
                        live_epoch_proofs.insert(
                            c.hash,
                            EpochTransition {
                                block_hash: c.hash,
                                block_number: cur_number,
                                proof: proof.into_vec(),
                            },
                        );
                    }
                }

                if candidates.len() < 3072 {
                    candidates.insert(cur_number, entry);
                }

                if cur_number > 0 {
                    cur_number -= 1;
                } else {
                    break;
                }
            }

            // fill best block block descriptor.
            let best_block = {
                if let Some(era) = candidates.get(&curr.best_num) {
                    let best = &era.candidates[0];
                    BlockDescriptor {
                        hash: best.hash,
                        number: curr.best_num,
                        total_difficulty: best.total_difficulty,
                    }
                } else {
                    BlockDescriptor {
                        hash: H256::from(0),
                        number: curr.best_num,
                        total_difficulty: U256::from(0),
                    }
                }
            };

            HeaderChain {
                genesis_header: encoded::Header::new(genesis.to_vec()),
                best_block: RwLock::new(best_block),
                candidates: RwLock::new(candidates),
                live_epoch_proofs: RwLock::new(live_epoch_proofs),
                db,
                cache,
            }
        } else {
            let chain = HeaderChain {
                genesis_header: encoded::Header::new(genesis.to_vec()),
                best_block: RwLock::new(BlockDescriptor {
                    hash: decoded_header.hash(),
                    number: 0,
                    total_difficulty: *decoded_header.difficulty(),
                }),
                candidates: RwLock::new(BTreeMap::new()),
                live_epoch_proofs: RwLock::new(live_epoch_proofs),
                db: db.clone(),
                cache,
            };

            let mut tx = DBTransaction::new();
            if let Ok(pending) =
                chain.insert(&mut tx, &spec.genesis_header().encoded(), None, false)
            {
                chain.apply_pending(tx, pending);
            }

            chain
        };

        // instantiate genesis epoch data if it doesn't exist.
        if chain
            .db
            .get(COL, LAST_CANONICAL_TRANSITION)
            .unwrap_or(None)
            .is_none()
        {
            let genesis_data = spec.genesis_epoch_data()?;

            {
                let mut batch = DBTransaction::new();
                let data = encode_canonical_transition(&decoded_header, &genesis_data);
                batch.put_vec(COL, LAST_CANONICAL_TRANSITION, data);
                chain.db.write(batch).unwrap();
            }
        }

        Ok(chain)
    }

    /// Insert a pre-verified header.
    ///
    /// This blindly trusts that the data given to it is sensible.
    /// Returns a set of pending changes to be applied with `apply_pending`
    /// before the next call to insert and after the transaction has been written.
    ///
    /// If the block is an epoch transition, provide the transition along with
    /// the header.
    pub fn insert(
        &self,
        transaction: &mut DBTransaction,
        header: &encoded::Header,
        transition_proof: Option<Vec<u8>>,
        is_force_reorg: bool,
    ) -> Result<PendingChanges, String>
    {
        self.insert_inner(transaction, header, None, transition_proof, is_force_reorg)
    }

    /// Insert a pre-verified header, with a known total difficulty. Similary to `insert`.
    ///
    /// This blindly trusts that the data given to it is sensible.
    pub fn insert_with_td(
        &self,
        transaction: &mut DBTransaction,
        header: &encoded::Header,
        total_difficulty: Option<U256>,
        transition_proof: Option<Vec<u8>>,
        is_force_reorg: bool,
    ) -> Result<PendingChanges, String>
    {
        self.insert_inner(
            transaction,
            header,
            total_difficulty,
            transition_proof,
            is_force_reorg,
        )
    }

    fn insert_inner(
        &self,
        transaction: &mut DBTransaction,
        header: &encoded::Header,
        total_difficulty: Option<U256>,
        transition_proof: Option<Vec<u8>>,
        is_force_reorg: bool,
    ) -> Result<PendingChanges, String>
    {
        let hash = header.hash();
        let number = header.number();
        let parent_hash = header.parent_hash();
        let transition = transition_proof.map(|proof| {
            EpochTransition {
                block_hash: hash,
                block_number: number,
                proof,
            }
        });

        let mut pending = PendingChanges {
            best_block: None,
        };

        // hold candidates the whole time to guard import order.
        let mut candidates = self.candidates.write();

        // find total difficulty.
        let total_difficulty = match total_difficulty {
            Some(td) => td,
            None => {
                let parent_td = if number <= 1 {
                    self.genesis_header.difficulty()
                } else {
                    candidates
                        .get(&(number - 1))
                        .and_then(|entry| entry.candidates.iter().find(|c| c.hash == parent_hash))
                        .map(|c| c.total_difficulty)
                        .ok_or_else(|| BlockError::UnknownParent(parent_hash))
                        .expect("!!!")
                };

                parent_td + header.difficulty()
            }
        };

        // insert headers and candidates entries and write era to disk.
        {
            let cur_era = candidates.entry(number).or_insert_with(|| {
                Entry {
                    candidates: SmallVec::new(),
                    canonical_hash: hash,
                }
            });
            cur_era.candidates.push(Candidate {
                hash,
                parent_hash,
                total_difficulty,
            });

            // fix ordering of era before writing.
            if total_difficulty > cur_era.candidates[0].total_difficulty {
                let cur_pos = cur_era.candidates.len() - 1;
                cur_era.candidates.swap(cur_pos, 0);
                cur_era.canonical_hash = hash;
            }

            transaction.put(COL, era_key(number).as_bytes(), &::rlp::encode(&*cur_era))
        }

        if let Some(transition) = transition {
            transaction.put(COL, &*transition_key(hash), &transition.proof);
            self.live_epoch_proofs.write().insert(hash, transition);
        }

        let raw = header.clone().into_inner();
        transaction.put_vec(COL, &hash[..], raw);

        // TODO: For engines when required, use cryptoeconomic guarantees.
        let (best_num, is_new_best) = {
            let cur_best = self.best_block.read();
            if cur_best.total_difficulty < total_difficulty {
                (number, true)
            } else {
                (cur_best.number, false)
            }
        };

        // reorganize ancestors so canonical entries are first in their
        // respective candidates vectors.
        if is_new_best || is_force_reorg {
            let mut canon_hash = hash;
            for (&height, entry) in candidates
                .iter_mut()
                .rev()
                .skip_while(|&(height, _)| *height > number)
            {
                if height == 0 || height != number && entry.canonical_hash == canon_hash {
                    break;
                }

                trace!(target: "chain", "Setting new canonical block {} for block height {}",
					canon_hash, height);

                if let Some(canon_pos) = entry.candidates.iter().position(|x| x.hash == canon_hash)
                {
                    // move the new canonical entry to the front and set the
                    // era's canonical hash.
                    entry.candidates.swap(0, canon_pos);

                    entry.canonical_hash = canon_hash;

                    // what about reorgs > cht::SIZE + HISTORY?
                    // resetting to the last block of a given CHT should be possible.
                    canon_hash = entry.candidates[0].parent_hash;

                    // write altered era to disk
                    if height != number {
                        let rlp_era = ::rlp::encode(&*entry);
                        transaction.put(COL, era_key(height).as_bytes(), &rlp_era);
                    }
                } else {
                    break;
                }
            }

            trace!(target: "chain", "New best block: ({}, {}), TD {}", number, hash, total_difficulty);
            pending.best_block = Some(BlockDescriptor {
                hash,
                number,
                total_difficulty,
            });

            // produce next CHT root if it's time.
            let earliest_era = *candidates
                .keys()
                .next()
                .expect("at least one era just created; qed");

            if earliest_era != 0
                && earliest_era + cht::SIZE <= number
                && earliest_era + HISTORY + cht::SIZE >= number
            {
                let cht_num = cht::block_to_cht_number(earliest_era)
                    .expect("fails only for number == 0; genesis never imported; qed");

                let mut last_canonical_transition = None;
                let cht_root = {
                    let mut i = earliest_era;
                    let mut live_epoch_proofs = self.live_epoch_proofs.write();

                    // iterable function which removes the candidates as it goes
                    // along. this will only be called until the CHT is complete.
                    let iter = || {
                        if let Some(era_entry) = candidates.remove(&i) {
                            // transaction.delete(COL, era_key(i).as_bytes());

                            i += 1;

                            // prune old blocks and epoch proofs.
                            for ancient in &era_entry.candidates {
                                let maybe_transition = live_epoch_proofs.remove(&ancient.hash);
                                if let Some(epoch_transition) = maybe_transition {
                                    // transaction.delete(COL, &*transition_key(ancient.hash));

                                    if ancient.hash == era_entry.canonical_hash {
                                        last_canonical_transition = match self
                                            .db
                                            .get(COL, &ancient.hash)
                                        {
                                            Err(e) => {
                                                warn!(target: "chain", "Error reading from DB: {}\n
												", e);
                                                None
                                            }
                                            Ok(None) => {
                                                panic!(
                                                    "stored candidates always have corresponding \
                                                     headers; qed"
                                                )
                                            }
                                            Ok(Some(header)) => {
                                                let header_rlp = UntrustedRlp::new(header.as_ref());
                                                let header_value = header_rlp
                                                    .as_val()
                                                    .expect("decoding db value failed");
                                                Some((epoch_transition, header_value))
                                            }
                                        };
                                    }
                                }

                                // transaction.delete(COL, &ancient.hash);
                            }

                            let canon = &era_entry.candidates[0];
                            (canon.hash, canon.total_difficulty)
                        } else {
                            (H256::default(), U256::default())
                        }
                    };
                    cht::compute_root(cht_num, ::itertools::repeat_call(iter))
                        .expect("fails only when too few items; this is checked; qed")
                };

                // write the CHT root to the database.
                debug!(target: "chain", "Produced CHT {} root: {:?}", cht_num, cht_root);
                transaction.put(COL, cht_key(cht_num).as_bytes(), &::rlp::encode(&cht_root));

                // update the last canonical transition proof
                if let Some((epoch_transition, header)) = last_canonical_transition {
                    let x = encode_canonical_transition(&header, &epoch_transition.proof);
                    transaction.put_vec(COL, LAST_CANONICAL_TRANSITION, x);
                }
            }
        }

        // write the best and latest eras to the database.
        {
            let latest_num = *candidates
                .iter()
                .rev()
                .next()
                .expect("at least one era just inserted; qed")
                .0;
            let curr = BestAndLatest::new(best_num, latest_num);
            transaction.put(COL, CURRENT_KEY, &::rlp::encode(&curr))
        }
        Ok(pending)
    }

    /// Apply pending changes from a previous `insert` operation.
    /// Must be done before the next `insert` call.
    pub fn apply_pending(&self, tx: DBTransaction, pending: PendingChanges) {
        let _ = self.db.write_buffered(tx);
        if let Some(best_block) = pending.best_block {
            *self.best_block.write() = best_block;
        }
    }

    /// Flush db
    pub fn flush(&self) { let _ = self.db.flush(); }

    /// Get a block's hash by ID. In the case of query by number, only canonical results
    /// will be returned.
    pub fn block_hash(&self, id: BlockId) -> Option<H256> {
        match id {
            BlockId::Earliest | BlockId::Number(0) => Some(self.genesis_hash()),
            BlockId::Hash(hash) => Some(hash),
            BlockId::Number(num) => {
                if self.best_block.read().number < num {
                    return None;
                }
                if let Some(entry) = self.candidates.read().get(&num) {
                    return Some(entry.canonical_hash);
                } else {
                    if let Some(entry) = self.db.get(COL, era_key(num).as_bytes()).unwrap_or(None) {
                        let entry_rlp = UntrustedRlp::new(entry.as_ref());
                        let entry_value = entry_rlp.as_val().expect("decoding db value failed");
                        let entry: Entry = entry_value;
                        return Some(entry.canonical_hash);
                    } else {
                        return None;
                    }
                }
            }
            BlockId::Pending | BlockId::Latest => Some(self.best_block.read().hash),
        }
    }

    /// Get a block header. In the case of query by number, only canonical blocks
    /// will be returned.
    pub fn block_header(&self, id: BlockId) -> Option<encoded::Header> {
        let load_from_db = |hash: H256| {
            let mut cache = self.cache.lock();

            match cache.block_header(&hash) {
                Some(header) => Some(header),
                None => {
                    match self.db.get(COL, &hash) {
                        Ok(db_value) => {
                            db_value
                                .map(|x| x.into_vec())
                                .map(encoded::Header::new)
                                .and_then(|header| {
                                    cache.insert_block_header(hash, header.clone());
                                    Some(header)
                                })
                        }
                        Err(e) => {
                            warn!(target: "chain", "Failed to read from database: {}", e);
                            None
                        }
                    }
                }
            }
        };

        match id {
            BlockId::Earliest | BlockId::Number(0) => Some(self.genesis_header.clone()),
            BlockId::Hash(hash) if hash == self.genesis_hash() => Some(self.genesis_header.clone()),
            BlockId::Hash(hash) => load_from_db(hash),
            BlockId::Number(num) => {
                if self.best_block.read().number < num {
                    return None;
                }

                self.candidates
                    .read()
                    .get(&num)
                    .map(|entry| entry.canonical_hash)
                    .and_then(load_from_db)
            }
            BlockId::Pending | BlockId::Latest => {
                // hold candidates hear to prevent deletion of the header
                // as we read it.
                let _candidates = self.candidates.read();
                let hash = {
                    let best = self.best_block.read();
                    if best.number == 0 {
                        return Some(self.genesis_header.clone());
                    }

                    best.hash
                };

                load_from_db(hash)
            }
        }
    }

    /// Get a block's chain score.
    /// Returns nothing for non-canonical blocks.
    pub fn score(&self, id: BlockId) -> Option<U256> {
        let genesis_hash = self.genesis_hash();
        match id {
            BlockId::Earliest | BlockId::Number(0) => Some(self.genesis_header.difficulty()),
            BlockId::Hash(hash) if hash == genesis_hash => Some(self.genesis_header.difficulty()),
            BlockId::Hash(hash) => {
                match self.block_header(BlockId::Hash(hash)) {
                    Some(header) => {
                        self.candidates
                            .read()
                            .get(&header.number())
                            .and_then(|era| era.candidates.iter().find(|e| e.hash == hash))
                            .map(|c| c.total_difficulty)
                    }
                    None => None,
                }
            }
            BlockId::Number(num) => {
                let candidates = self.candidates.read();
                if self.best_block.read().number < num {
                    return None;
                }
                candidates
                    .get(&num)
                    .map(|era| era.candidates[0].total_difficulty)
            }
            BlockId::Pending | BlockId::Latest => Some(self.best_block.read().total_difficulty),
        }
    }

    /// Get the best block's header.
    pub fn best_header(&self) -> encoded::Header {
        self.block_header(BlockId::Latest)
            .expect("Header for best block always stored; qed")
    }

    /// Get an iterator over a block and its ancestry.
    pub fn ancestry_iter(&self, start: BlockId) -> AncestryIter {
        AncestryIter {
            next: self.block_header(start),
            chain: self,
        }
    }

    /// Get the nth CHT root, if it's been computed.
    ///
    /// CHT root 0 is from block `1..2048`.
    /// CHT root 1 is from block `2049..4096`
    /// and so on.
    ///
    /// This is because it's assumed that the genesis hash is known,
    /// so including it within a CHT would be redundant.
    pub fn cht_root(&self, n: usize) -> Option<H256> {
        match self.db.get(COL, cht_key(n as u64).as_bytes()) {
            Ok(db_fetch) => {
                if let Some(db_fetch_some) = db_fetch {
                    let db_fetch_rlp = UntrustedRlp::new(&db_fetch_some);
                    let db_fetch_value = db_fetch_rlp
                        .as_val()
                        .expect("decoding value from db failed");
                    Some(db_fetch_value)
                } else {
                    warn!(target: "chain", "No data found in database");
                    None
                }
            }
            Err(e) => {
                warn!(target: "chain", "Error reading from database: {}", e);
                None
            }
        }
    }

    /// Get the genesis hash.
    pub fn genesis_hash(&self) -> H256 { self.genesis_header.hash() }

    /// Get the best block's data.
    pub fn best_block(&self) -> BlockDescriptor { self.best_block.read().clone() }

    /// If there is a gap between the genesis and the rest
    /// of the stored blocks, return the first post-gap block.
    pub fn first_block(&self) -> Option<BlockDescriptor> {
        let candidates = self.candidates.read();
        match candidates.iter().next() {
            None | Some((&1, _)) => None,
            Some((&height, entry)) => {
                Some(BlockDescriptor {
                    number: height,
                    hash: entry.canonical_hash,
                    total_difficulty: entry
                        .candidates
                        .iter()
                        .find(|x| x.hash == entry.canonical_hash)
                        .expect("entry always stores canonical candidate; qed")
                        .total_difficulty,
                })
            }
        }
    }

    /// Get block status.
    pub fn status(&self, hash: &H256) -> BlockStatus {
        if self.db.get(COL, hash).ok().map_or(false, |x| x.is_some()) {
            BlockStatus::InChain
        } else {
            BlockStatus::Unknown
        }
    }

    /// Insert a pending transition.
    pub fn insert_pending_transition(
        &self,
        _batch: &mut DBTransaction,
        _hash: H256,
        _t: &PendingEpochTransition,
    )
    {
    }

    /// Get pending transition for a specific block hash.
    pub fn pending_transition(&self, _hash: H256) -> Option<PendingEpochTransition> { None }

    /// Get the transition to the epoch the given parent hash is part of
    /// or transitions to.
    /// This will give the epoch that any children of this parent belong to.
    ///
    /// The header corresponding the the parent hash must be stored already.
    pub fn epoch_transition_for(&self, _parent_hash: H256) -> Option<(Header, Vec<u8>)> { None }

    /// chain info
    pub fn chain_info(&self) -> BlockChainInfo {
        let best_block = self.best_block();
        BlockChainInfo {
            total_difficulty: best_block.total_difficulty,
            pending_total_difficulty: best_block.total_difficulty,
            genesis_hash: self.genesis_hash(),
            best_block_hash: best_block.hash,
            best_block_number: best_block.number,
            best_block_timestamp: 0,
            ancient_block_hash: None,
            ancient_block_number: None,
            first_block_hash: None,
            first_block_number: None,
        }
    }
}

impl HeapSizeOf for HeaderChain {
    fn heap_size_of_children(&self) -> usize { self.candidates.read().heap_size_of_children() }
}

/// Iterator over a block's ancestry.
pub struct AncestryIter<'a> {
    next: Option<encoded::Header>,
    chain: &'a HeaderChain,
}

impl<'a> Iterator for AncestryIter<'a> {
    type Item = encoded::Header;

    fn next(&mut self) -> Option<encoded::Header> {
        let next = self.next.take();
        if let Some(p_hash) = next.as_ref().map(|hdr| hdr.parent_hash()) {
            self.next = self.chain.block_header(BlockId::Hash(p_hash));
        }

        next
    }
}

#[cfg(test)]
mod tests {
    use super::{HeaderChain, HardcodedSync};
    use std::sync::Arc;

    use header::Header;
    use ids::BlockId;
    use spec::Spec;
    use aion_types::U256;
    use kvdb::{KeyValueDB,DBTransaction};

    use std::time::Duration;
    use std::path::Path;
    use parking_lot::Mutex;

    #[test]
    fn basic_chain() {
        let spec = Spec::new_test();
        let genesis_header = spec.genesis_header();
        let path = Path::new("./test_HC");

        let chain = HeaderChain::new(path.clone(), &spec).unwrap();

        let mut parent_hash = genesis_header.hash();
        let mut rolling_timestamp = genesis_header.timestamp();
        for i in 1..10000 {
            let mut header = Header::new();
            header.set_parent_hash(parent_hash);
            header.set_number(i);
            header.set_timestamp(rolling_timestamp);
            header.set_difficulty(*genesis_header.difficulty() * i as u32);
            parent_hash = header.hash();

            let mut tx = DBTransaction::new();
            let pending = chain
                .insert(&mut tx, &header.encoded(), None, false)
                .unwrap();
            chain.apply_pending(tx, pending);

            rolling_timestamp += 10;
        }

        assert!(chain.block_header(BlockId::Number(10)).is_some());
        assert!(chain.block_header(BlockId::Number(9000)).is_some());
        ::std::fs::remove_dir_all(path);
    }

    #[test]
    fn reorganize() {
        let spec = Spec::new_test();
        let genesis_header = spec.genesis_header();
        let path = Path::new("./test_HC");

        let chain = HeaderChain::new(path.clone(), &spec).unwrap();

        let mut parent_hash = genesis_header.hash();
        let mut rolling_timestamp = genesis_header.timestamp();
        for i in 1..6 {
            let mut header = Header::new();
            header.set_parent_hash(parent_hash);
            header.set_number(i);
            header.set_timestamp(rolling_timestamp);
            header.set_difficulty(*genesis_header.difficulty() * i as u32);
            parent_hash = header.hash();

            let mut tx = DBTransaction::new();
            let pending = chain
                .insert(&mut tx, &header.encoded(), None, false)
                .unwrap();
            chain.apply_pending(tx, pending);

            rolling_timestamp += 10;
        }

        {
            let mut rolling_timestamp = rolling_timestamp;
            let mut parent_hash = parent_hash;
            for i in 6..16 {
                let mut header = Header::new();
                header.set_parent_hash(parent_hash);
                header.set_number(i);
                header.set_timestamp(rolling_timestamp);
                header.set_difficulty(*genesis_header.difficulty() * i as u32);
                parent_hash = header.hash();

                let mut tx = DBTransaction::new();
                let pending = chain
                    .insert(&mut tx, &header.encoded(), None, false)
                    .unwrap();
                chain.apply_pending(tx, pending);

                rolling_timestamp += 10;
            }
        }
        assert_eq!(chain.best_block().number, 15);

        {
            let mut rolling_timestamp = rolling_timestamp;
            let mut parent_hash = parent_hash;

            // import a shorter chain which has better TD.
            for i in 6..13 {
                let mut header = Header::new();
                header.set_parent_hash(parent_hash);
                header.set_number(i);
                header.set_timestamp(rolling_timestamp);
                header.set_difficulty(*genesis_header.difficulty() * U256::from(i * i));
                parent_hash = header.hash();

                let mut tx = DBTransaction::new();
                let pending = chain
                    .insert(&mut tx, &header.encoded(), None, false)
                    .unwrap();
                chain.apply_pending(tx, pending);

                rolling_timestamp += 11;
            }
        }

        let (mut num, mut canon_hash) = (chain.best_block().number, chain.best_block().hash);
        assert_eq!(num, 12);

        while num > 0 {
            let header = chain.block_header(BlockId::Number(num)).unwrap();
            assert_eq!(header.hash(), canon_hash);

            canon_hash = header.parent_hash();
            num -= 1;
        }
        ::std::fs::remove_dir_all(path);
    }

    #[test]
    fn earliest_is_latest() {
        let spec = Spec::new_test();
        let path = Path::new("./test_HC");

        let chain = HeaderChain::new(path.clone(), &spec).unwrap();

        assert!(chain.block_header(BlockId::Earliest).is_some());
        assert!(chain.block_header(BlockId::Latest).is_some());
        assert_eq!(
            chain.block_header(BlockId::Earliest).unwrap(),
            chain.block_header(BlockId::Latest).unwrap()
        );
        ::std::fs::remove_dir_all(path);
    }

    #[test]
    fn restore_from_db() {
        let spec = Spec::new_test();
        let genesis_header = spec.genesis_header();
        let path = Path::new("./test_HC");

        {
            let chain = HeaderChain::new(path.clone(), &spec).unwrap();
            let mut parent_hash = genesis_header.hash();
            let mut rolling_timestamp = genesis_header.timestamp();
            for i in 1..10000 {
                let mut header = Header::new();
                header.set_parent_hash(parent_hash);
                header.set_number(i);
                header.set_timestamp(rolling_timestamp);
                header.set_difficulty(*genesis_header.difficulty() * i as u32);
                parent_hash = header.hash();

                let mut tx = DBTransaction::new();
                let pending = chain
                    .insert(&mut tx, &header.encoded(), None, false)
                    .unwrap();
                chain.apply_pending(tx, pending);

                rolling_timestamp += 10;
            }
        }

        let chain = HeaderChain::new(path.clone(), &spec).unwrap();
        assert!(chain.block_header(BlockId::Number(6800)).is_none());
        assert!(chain.block_header(BlockId::Number(9000)).is_some());
        assert_eq!(chain.block_header(BlockId::Latest).unwrap().number(), 9999);
        ::std::fs::remove_dir_all(path);
    }

    #[test]
    fn restore_higher_non_canonical() {
        let spec = Spec::new_test();
        let genesis_header = spec.genesis_header();
        let path = Path::new("./test_HC");

        {
            let chain = HeaderChain::new(path.clone(), &spec).unwrap();
            let mut parent_hash = genesis_header.hash();
            let mut rolling_timestamp = genesis_header.timestamp();

            // push 100 low-difficulty blocks.
            for i in 1..101 {
                let mut header = Header::new();
                header.set_parent_hash(parent_hash);
                header.set_number(i);
                header.set_timestamp(rolling_timestamp);
                header.set_difficulty(*genesis_header.difficulty() * i as u32);
                parent_hash = header.hash();

                let mut tx = DBTransaction::new();
                let pending = chain
                    .insert(&mut tx, &header.encoded(), None, false)
                    .unwrap();
                chain.apply_pending(tx, pending);

                rolling_timestamp += 10;
            }

            // push fewer high-difficulty blocks.
            for i in 1..11 {
                let mut header = Header::new();
                header.set_parent_hash(parent_hash);
                header.set_number(i);
                header.set_timestamp(rolling_timestamp);
                header
                    .set_difficulty(*genesis_header.difficulty() * U256::from(i as u32 * 1000u32));
                parent_hash = header.hash();

                let mut tx = DBTransaction::new();
                let pending = chain
                    .insert(&mut tx, &header.encoded(), None, false)
                    .unwrap();
                chain.apply_pending(tx, pending);

                rolling_timestamp += 10;
            }

            assert_eq!(chain.block_header(BlockId::Latest).unwrap().number(), 10);
        }

        // after restoration, non-canonical eras should still be loaded.
        let chain = HeaderChain::new(path.clone(), &spec).unwrap();
        assert_eq!(chain.block_header(BlockId::Latest).unwrap().number(), 10);
        assert!(chain.candidates.read().get(&100).is_some());
        ::std::fs::remove_dir_all(path);
    }

    #[test]
    fn genesis_header_available() {
        let spec = Spec::new_test();
        let genesis_header = spec.genesis_header();
        let path = Path::new("./test_HC");

        let chain = HeaderChain::new(path.clone(), &spec).unwrap();

        assert!(chain.block_header(BlockId::Earliest).is_some());
        assert!(chain.block_header(BlockId::Number(0)).is_some());
        assert!(
            chain
                .block_header(BlockId::Hash(genesis_header.hash()))
                .is_some()
        );
        ::std::fs::remove_dir_all(path);
    }

}
