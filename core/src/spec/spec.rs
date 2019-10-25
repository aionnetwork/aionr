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

//! Parameters for a block chain.

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::Arc;

use aion_types::{Address, H256, U256};
use ajson;
use blake2b::{blake2b, BLAKE2B_NULL_RLP};
use acore_bytes::Bytes;
use ethbloom::Bloom;
use kvdb::{MemoryDB, MockDbRepository};
use parking_lot::RwLock;
use rlp::{Rlp, RlpStream};
use types::BlockNumber;
use vms::{ActionParams, ActionValue, CallType, EnvInfo, ParamsType};
use engine::{Engine, UnityEngine};
use types::error::Error;
use executive::Executive;
use factory::Factories;
use header::{Header, SealType};
use machine::EthereumMachine;
use pod_state::PodState;
use precompiled::builtin::{builtin_contract, BuiltinContract};
use spec::seal::Generic as GenericSeal;
use spec::Genesis;
use state::backend::Basic as BasicBackend;
use state::{Backend, State, Substate};

#[cfg(test)]
use tests::common::null_engine::NullEngine;

// helper for formatting errors.
fn fmt_err<F: ::std::fmt::Display>(f: F) -> String { format!("Spec json is invalid: {}", f) }

/// Parameters common to ethereum-like blockchains.
#[derive(Debug, PartialEq, Default)]
#[cfg_attr(test, derive(Clone))]
pub struct CommonParams {
    /// Maximum size of extra data.
    pub maximum_extra_data_size: usize,
    /// Minimum gas limit.
    pub min_gas_limit: U256,
    /// Gas limit bound divisor (how much gas limit can change per block)
    pub gas_limit_bound_divisor: U256,
    /// monetary policy update block number.
    pub monetary_policy_update: Option<BlockNumber>,
    /// Transaction permission managing contract address.
    pub transaction_permission_contract: Option<Address>,
    /// unity update block number.
    pub unity_update: Option<BlockNumber>,
}

impl From<ajson::spec::Params> for CommonParams {
    fn from(p: ajson::spec::Params) -> Self {
        let data_size = p.maximum_extra_data_size.into();
        CommonParams {
            maximum_extra_data_size: if data_size > 0 { data_size } else { 32usize },
            min_gas_limit: p.min_gas_limit.into(),
            gas_limit_bound_divisor: p.gas_limit_bound_divisor.into(),
            monetary_policy_update: p.monetary_policy_update.map(Into::into),
            transaction_permission_contract: p.transaction_permission_contract.map(Into::into),
            unity_update: p.unity_update.map(Into::into),
        }
    }
}

/// Parameters for a block chain; includes both those intrinsic to the design of the
/// chain and those to be interpreted by the active chain engine.
pub struct Spec {
    /// User friendly spec name
    pub name: String,
    /// What engine are we using for this?
    pub engine: Arc<Engine>,
    /// Name of the subdir inside the main data dir to use for chain data and settings.
    pub data_dir: String,
    /// The genesis block's parent hash field.
    pub parent_hash: H256,
    /// The genesis block's author field.
    pub author: Address,
    /// The genesis block's difficulty field.
    pub difficulty: U256,
    /// The genesis block's gas limit field.
    pub gas_limit: U256,
    /// The genesis block's gas used field.
    pub gas_used: U256,
    /// The genesis block's timestamp field.
    pub timestamp: u64,
    /// Transactions root of the genesis block. Should be BLAKE2B_NULL_RLP.
    pub transactions_root: H256,
    /// Receipts root of the genesis block. Should be BLAKE2B_NULL_RLP.
    pub receipts_root: H256,
    /// The genesis block's extra data field.
    pub extra_data: Bytes,
    /// Each seal field, expressed as RLP, concatenated.
    pub seal_rlp: Bytes,

    /// Contract constructors to be executed on genesis.
    constructors: Vec<(Address, Bytes)>,

    /// May be prepopulated if we know this in advance.
    state_root_memo: RwLock<H256>,

    /// Genesis state as plain old data.
    genesis_state: PodState,
}

#[cfg(test)]
macro_rules! load_bundled {
    ($e:expr) => {
        Spec::load(include_bytes!(concat!("../../../resources/", $e, ".json")) as &[u8])
            .expect(concat!("Chain spec ", $e, " is invalid."))
    };
    ($e:expr,$u:expr) => {
        Spec::load_with_unity_update(
            include_bytes!(concat!("../../../resources/", $e, ".json")) as &[u8],
            $u,
        )
        .expect(concat!("Chain spec ", $e, " is invalid."))
    };
}

#[cfg(test)]
impl Clone for Spec {
    fn clone(&self) -> Spec {
        Spec {
            name: self.name.clone(),
            engine: self.engine.clone(),
            data_dir: self.data_dir.clone(),
            parent_hash: self.parent_hash.clone(),
            transactions_root: self.transactions_root.clone(),
            receipts_root: self.receipts_root.clone(),
            author: self.author.clone(),
            difficulty: self.difficulty.clone(),
            gas_limit: self.gas_limit.clone(),
            gas_used: self.gas_used.clone(),
            timestamp: self.timestamp.clone(),
            extra_data: self.extra_data.clone(),
            seal_rlp: self.seal_rlp.clone(),
            constructors: self.constructors.clone(),
            state_root_memo: RwLock::new(*self.state_root_memo.read()),
            genesis_state: self.genesis_state.clone(),
        }
    }
}

#[cfg(test)]
fn load_machine_from(s: ajson::spec::Spec) -> EthereumMachine {
    let builtins = s
        .accounts
        .builtins()
        .into_iter()
        .map(|p| (p.0.into(), builtin_contract(From::from(p.1))))
        .collect();
    let params = CommonParams::from(s.params);

    Spec::machine(params, builtins, s.accounts.premine())
}

/// Load from JSON object.
fn load_from(s: ajson::spec::Spec) -> Result<Spec, Error> {
    let builtins = s
        .accounts
        .builtins()
        .into_iter()
        .map(|p| (p.0.into(), builtin_contract(From::from(p.1))))
        .collect();
    let g = Genesis::from(s.genesis);
    let GenericSeal(seal_rlp) = g.seal.into();
    let params = CommonParams::from(s.params);

    let mut s = Spec {
        name: s.name.clone().into(),
        engine: Spec::engine(s.engine, params, builtins, s.accounts.premine()),
        data_dir: s.data_dir.unwrap_or(s.name).into(),
        parent_hash: g.parent_hash,
        transactions_root: g.transactions_root,
        receipts_root: g.receipts_root,
        author: g.author,
        difficulty: g.difficulty,
        gas_limit: g.gas_limit,
        gas_used: g.gas_used,
        timestamp: g.timestamp,
        extra_data: g.extra_data,
        seal_rlp,
        constructors: s
            .accounts
            .constructors()
            .into_iter()
            .map(|(a, c)| (a.into(), c.into()))
            .collect(),
        state_root_memo: RwLock::new(Default::default()), // will be overwritten right after.
        genesis_state: s.accounts.into(),
    };

    // use memoized state root if provided.
    match g.state_root {
        Some(root) => *s.state_root_memo.get_mut() = root,
        None => {
            let _ = s.run_constructors(&Default::default(), BasicBackend(MemoryDB::new()))?;
        }
    }

    Ok(s)
}

#[cfg(test)]
/// Load from JSON object.
fn load_from_with_unity_update(s: ajson::spec::Spec, unity_update: u64) -> Result<Spec, Error> {
    let builtins = s
        .accounts
        .builtins()
        .into_iter()
        .map(|p| (p.0.into(), builtin_contract(From::from(p.1))))
        .collect();
    let g = Genesis::from(s.genesis);
    let GenericSeal(seal_rlp) = g.seal.into();
    let mut params = CommonParams::from(s.params);
    params.unity_update = Some(unity_update);

    let mut s = Spec {
        name: s.name.clone().into(),
        engine: Spec::engine(s.engine, params, builtins, s.accounts.premine()),
        data_dir: s.data_dir.unwrap_or(s.name).into(),
        parent_hash: g.parent_hash,
        transactions_root: g.transactions_root,
        receipts_root: g.receipts_root,
        author: g.author,
        difficulty: g.difficulty,
        gas_limit: g.gas_limit,
        gas_used: g.gas_used,
        timestamp: g.timestamp,
        extra_data: g.extra_data,
        seal_rlp,
        constructors: s
            .accounts
            .constructors()
            .into_iter()
            .map(|(a, c)| (a.into(), c.into()))
            .collect(),
        state_root_memo: RwLock::new(Default::default()), // will be overwritten right after.
        genesis_state: s.accounts.into(),
    };

    // use memoized state root if provided.
    match g.state_root {
        Some(root) => *s.state_root_memo.get_mut() = root,
        None => {
            let _ = s.run_constructors(&Default::default(), BasicBackend(MemoryDB::new()))?;
        }
    }

    Ok(s)
}

impl Spec {
    #[cfg(test)]
    /// Create a new Spec which conforms to the Frontier-era Morden chain except that it's a
    /// NullEngine consensus.
    pub fn new_test() -> Spec { load_bundled!("null_morden") }

    #[cfg(test)]
    /// Create a new Spec which is a NullEngine consensus with a premine of address whose
    /// secret is blake2b('').
    pub fn new_null() -> Spec { load_bundled!("null") }

    #[cfg(test)]
    /// Create a new Spec which is a UnityEngine consensus
    pub fn new_unity(unity_update: Option<u64>) -> Spec {
        if let Some(u) = unity_update {
            load_bundled!("null_unity", u)
        } else {
            load_bundled!("null_unity")
        }
    }

    // create an instance of an Ethereum state machine, minus consensus logic.
    fn machine(
        params: CommonParams,
        builtins: BTreeMap<Address, Box<BuiltinContract>>,
        premine: U256,
    ) -> EthereumMachine
    {
        EthereumMachine::regular(params, builtins, premine)
    }

    /// Convert engine spec into a arc'd Engine of the right underlying type.
    /// TODO avoid this hard-coded nastiness - use dynamic-linked plugin framework instead.
    fn engine(
        engine_spec: ajson::spec::Engine,
        params: CommonParams,
        builtins: BTreeMap<Address, Box<BuiltinContract>>,
        premine: U256,
    ) -> Arc<Engine>
    {
        let machine = Self::machine(params, builtins, premine);

        match engine_spec {
            ajson::spec::Engine::UnityEngine(unity_engine) => {
                Arc::new(UnityEngine::new(unity_engine.params.into(), machine))
            }
            ajson::spec::Engine::Null(_null_engine) => {
                #[cfg(test)]
                {
                    Arc::new(NullEngine::new(_null_engine.params.into(), machine))
                }
                #[cfg(not(test))]
                {
                    panic!("NullEngine Should not be used in normal builds");
                }
            }
        }
    }

    // given a pre-constructor state, run all the given constructors and produce a new state and
    // state root.
    fn run_constructors<T: Backend>(&self, factories: &Factories, mut db: T) -> Result<T, Error> {
        let mut root = BLAKE2B_NULL_RLP;

        // basic accounts in spec.
        {
            let mut t = factories.trie.create(db.as_hashstore_mut(), &mut root);
            for (address, account) in self.genesis_state.get().iter() {
                t.insert(&**address, &account.rlp())?;
            }
        }

        for (address, account) in self.genesis_state.get().iter() {
            db.note_non_null_account(address);
            account.insert_additional(
                &mut *factories
                    .accountdb
                    .create(db.as_hashstore_mut(), blake2b(address)),
                &factories.trie,
            );
        }

        let (root, db) = {
            let mut state = State::from_existing(
                db,
                root,
                U256::zero(),
                factories.clone(),
                Arc::new(MockDbRepository::init(vec![String::new()])),
            )?;

            // Execute contract constructors.
            let env_info = EnvInfo {
                number: 0,
                author: self.author,
                timestamp: self.timestamp,
                difficulty: self.difficulty,
                last_hashes: Default::default(),
                gas_used: U256::zero(),
                gas_limit: U256::max_value(),
            };

            let from = Address::default();
            for &(ref address, ref constructor) in self.constructors.iter() {
                trace!(target: "spec", "run_constructors: Creating a contract at {}.", address);
                trace!(target: "spec", "  .. root before = {}", state.root());
                let params = ActionParams {
                    code_address: address.clone(),
                    code_hash: Some(blake2b(constructor)),
                    address: address.clone(),
                    sender: from.clone(),
                    origin: from.clone(),
                    gas: U256::max_value(),
                    gas_price: Default::default(),
                    value: ActionValue::Transfer(Default::default()),
                    code: Some(Arc::new(constructor.clone())),
                    data: None,
                    call_type: CallType::None,
                    static_flag: false,
                    params_type: ParamsType::Embedded,
                    transaction_hash: H256::default(),
                    original_transaction_hash: H256::default(),
                    nonce: 0,
                };

                let mut substate = Substate::new();

                {
                    let mut exec = Executive::new(&mut state, &env_info, self.engine.machine());
                    let result = exec.create(params, &mut substate);
                    let exception: &str = result.exception.as_str();
                    if exception != "" {
                        warn!(target: "spec", "Genesis constructor execution at {} failed: {}.", address, exception);
                    }
                }

                if let Err(e) = state.commit() {
                    warn!(target: "spec", "Genesis constructor trie commit at {} failed: {}.", address, e);
                }

                trace!(target: "spec", "  .. root after = {}", state.root());
            }

            state.drop()
        };

        *self.state_root_memo.write() = root;
        Ok(db)
    }

    /// Return the state root for the genesis state, memoising accordingly.
    pub fn state_root(&self) -> H256 { self.state_root_memo.read().clone() }

    /// Get the header of the genesis block.
    pub fn genesis_header(&self) -> Header {
        let mut header: Header = Default::default();
        header.set_parent_hash(self.parent_hash.clone());
        header.set_timestamp(self.timestamp);
        header.set_number(0);
        header.set_author(self.author.clone());
        header.set_transactions_root(self.transactions_root.clone());
        header.set_extra_data(self.extra_data.clone());
        header.set_state_root(self.state_root());
        header.set_receipts_root(self.receipts_root.clone());
        header.set_log_bloom(Bloom::default());
        header.set_gas_used(self.gas_used.clone());
        header.set_gas_limit(self.gas_limit.clone());
        header.set_difficulty(self.difficulty.clone());
        header.set_seal_type(SealType::PoW);
        header.set_seal({
            let r = Rlp::new(&self.seal_rlp);
            r.iter().map(|f| f.as_val::<Bytes>()).collect()
        });
        trace!(target: "spec", "Header hash is {}", header.hash());
        header
    }

    /// Compose the genesis block for this chain.
    pub fn genesis_block(&self) -> Bytes {
        let empty_list = RlpStream::new_list(0).out();
        let header = self.genesis_header();
        let mut ret = RlpStream::new_list(2);
        ret.append(&header);
        ret.append_raw(&empty_list, 1);
        ret.out()
    }

    /// Alter the value of the genesis state.
    pub fn set_genesis_state(&mut self, s: PodState) -> Result<(), Error> {
        self.genesis_state = s;
        let _ = self.run_constructors(&Default::default(), BasicBackend(MemoryDB::new()))?;

        Ok(())
    }

    /// Ensure that the given state DB has the trie nodes in for the genesis state.
    pub fn ensure_db_good<T: Backend>(&self, db: T, factories: &Factories) -> Result<T, Error> {
        if db.as_hashstore().contains(&self.state_root()) {
            return Ok(db);
        }

        // TODO: could optimize so we don't re-run, but `ensure_db_good` is barely ever
        // called anyway.
        let db = self.run_constructors(factories, db)?;
        Ok(db)
    }

    /// Loads just the state machine from a json file.
    #[cfg(test)]
    pub fn load_machine<R: Read>(reader: R) -> Result<EthereumMachine, String> {
        ajson::spec::Spec::load(reader)
            .map_err(fmt_err)
            .map(load_machine_from)
    }

    /// Loads spec from json file. Provide factories for executing contracts and ensuring
    /// storage goes to the right place.
    pub fn load<'a, R>(reader: R) -> Result<Self, String>
    where R: Read {
        ajson::spec::Spec::load(reader)
            .map_err(fmt_err)
            .and_then(|x| load_from(x).map_err(fmt_err))
    }

    /// Loads spec from json file. Provide factories for executing contracts and ensuring
    /// storage goes to the right place.
    #[cfg(test)]
    pub fn load_with_unity_update<'a, R>(reader: R, update_unity: u64) -> Result<Self, String>
    where R: Read {
        ajson::spec::Spec::load(reader)
            .map_err(fmt_err)
            .and_then(|x| load_from_with_unity_update(x, update_unity).map_err(fmt_err))
    }
}

#[cfg(test)]
mod tests {
    use super::Spec;
    use views::BlockView;
    #[test]
    fn test_load_empty() {
        assert!(Spec::load(&[] as &[u8]).is_err());
    }

    #[test]
    fn test_chain() {
        let test_spec = Spec::new_test();

        assert_eq!(
            test_spec.state_root(),
            "b3fd94094ccb910e058c00d6763b61472e7bf1b8a9cb2549a83a4d5a397e194e".into()
        );
        let genesis = test_spec.genesis_block();
        assert_eq!(
            BlockView::new(&genesis).header_view().hash(),
            "6b1db4a3d0482aa864e8f0ee27870c6c03db9ff8649b8dce8b78db7f23967bc5".into()
        );
    }

}
