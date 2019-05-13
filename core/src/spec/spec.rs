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
use std::path::Path;
use std::sync::Arc;

use aion_types::{Address, H256, U256};
use ajson;
use blake2b::{blake2b, BLAKE2B_NULL_RLP};
use bytes::Bytes;
use ethbloom::Bloom;
use kvdb::{MemoryDB, MemoryDBRepository};
use parking_lot::RwLock;
use rlp::{Rlp, RlpStream};
use types::BlockNumber;
use vms::{ActionParams, ActionValue, CallType, EnvInfo, ParamsType};

use engines::{EthEngine, InstantSeal, NullEngine, POWEquihashEngine};
use error::Error;
use executive::Executive;
use factory::Factories;
use header::Header;
use machine::EthereumMachine;
use pod_state::PodState;
use precompiled::builtin::{builtin_contract, BuiltinContract};
use spec::seal::Generic as GenericSeal;
use spec::Genesis;
use state::backend::Basic as BasicBackend;
use state::{Backend, State, Substate};
use transaction::DEFAULT_TRANSACTION_TYPE;

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
    /// Registrar contract address.
    pub registrar: Address,
    /// monetary policy update block number.
    pub monetary_policy_update: Option<BlockNumber>,
    /// Transaction permission managing contract address.
    pub transaction_permission_contract: Option<Address>,
}

impl From<ajson::spec::Params> for CommonParams {
    fn from(p: ajson::spec::Params) -> Self {
        let data_size = p.maximum_extra_data_size.into();
        CommonParams {
            maximum_extra_data_size: if data_size > 0 { data_size } else { 32usize },
            min_gas_limit: p.min_gas_limit.into(),
            gas_limit_bound_divisor: p.gas_limit_bound_divisor.into(),
            registrar: p.registrar.map_or_else(Address::new, Into::into),
            monetary_policy_update: p.monetary_policy_update.map(Into::into),
            transaction_permission_contract: p.transaction_permission_contract.map(Into::into),
        }
    }
}

/// Runtime parameters for the spec that are related to how the software should run the chain,
/// rather than integral properties of the chain itself.
#[derive(Debug, Clone, Copy)]
pub struct SpecParams<'a> {
    /// The path to the folder used to cache nodes. This is typically /tmp/ on Unix-like systems
    pub cache_dir: &'a Path,
}

impl<'a> SpecParams<'a> {
    /// Create from a cache path, with null values for the other fields
    pub fn from_path(path: &'a Path) -> Self {
        SpecParams {
            cache_dir: path,
        }
    }

    /// Create from a cache path and an optimization setting
    pub fn new(path: &'a Path) -> Self {
        SpecParams {
            cache_dir: path,
        }
    }
}

impl<'a, T: AsRef<Path>> From<&'a T> for SpecParams<'a> {
    fn from(path: &'a T) -> Self { Self::from_path(path.as_ref()) }
}

/// Parameters for a block chain; includes both those intrinsic to the design of the
/// chain and those to be interpreted by the active chain engine.
pub struct Spec {
    /// User friendly spec name
    pub name: String,
    /// What engine are we using for this?
    pub engine: Arc<EthEngine>,
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

fn load_machine_from(s: ajson::spec::Spec) -> EthereumMachine {
    let builtins = s
        .accounts
        .builtins()
        .into_iter()
        .map(|p| (p.0.into(), builtin_contract(From::from(p.1))))
        .collect();
    let params = CommonParams::from(s.params);

    Spec::machine(&s.engine, params, builtins, s.accounts.premine())
}

/// Load from JSON object.
fn load_from(spec_params: SpecParams, s: ajson::spec::Spec) -> Result<Spec, Error> {
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
        engine: Spec::engine(
            spec_params,
            s.engine,
            params,
            builtins,
            s.accounts.premine(),
        ),
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
        seal_rlp: seal_rlp,
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

macro_rules! load_bundled {
    ($e:expr) => {
        Spec::load(
            &::std::env::temp_dir(),
            include_bytes!(concat!("../../res/", $e, ".json")) as &[u8],
        )
        .expect(concat!("Chain spec ", $e, " is invalid."))
    };
}

macro_rules! load_machine_bundled {
    ($e:expr) => {
        Spec::load_machine(include_bytes!(concat!("../../res/", $e, ".json")) as &[u8])
            .expect(concat!("Chain spec ", $e, " is invalid."))
    };
}

impl Spec {
    // create an instance of an Ethereum state machine, minus consensus logic.
    fn machine(
        _engine_spec: &ajson::spec::Engine,
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
        _spec_params: SpecParams,
        engine_spec: ajson::spec::Engine,
        params: CommonParams,
        builtins: BTreeMap<Address, Box<BuiltinContract>>,
        premine: U256,
    ) -> Arc<EthEngine>
    {
        let machine = Self::machine(&engine_spec, params, builtins, premine);

        match engine_spec {
            ajson::spec::Engine::POWEquihashEngine(pow_equihash_engine) => {
                Arc::new(POWEquihashEngine::new(
                    pow_equihash_engine.params.into(),
                    machine,
                ))
            }
            ajson::spec::Engine::Null(null) => {
                Arc::new(NullEngine::new(null.params.into(), machine))
            }
            ajson::spec::Engine::InstantSeal => Arc::new(InstantSeal::new(machine)),
        }
    }

    // given a pre-constructor state, run all the given constructors and produce a new state and
    // state root.
    fn run_constructors<T: Backend>(&self, factories: &Factories, mut db: T) -> Result<T, Error> {
        let mut root = BLAKE2B_NULL_RLP;

        // basic accounts in spec.
        {
            let mut t = factories.trie.create(db.as_hashstore_mut(), &mut root);

            //            let network_address = FromHex::from_hex(
            //                "0000000000000000000000000000000000000000000000000000000000000100",
            //            ).unwrap();
            //            let network_account_rlp = FromHex::from_hex("f8448080a005fd02342b56544ead95ab1e477b0baaa70f75b109b23098a84b67bf52c25deba00e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8").unwrap();
            //            t.insert(&network_address, &network_account_rlp)?;

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

        let start_nonce = self.engine.account_start_nonce(0);

        let (root, db) = {
            let mut state = State::from_existing(
                db,
                root,
                start_nonce,
                factories.clone(),
                Arc::new(MemoryDBRepository::new()),
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

    /// Get common blockchain parameters.
    pub fn params(&self) -> &CommonParams { &self.engine.params() }

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

    /// Overwrite the genesis components.
    pub fn overwrite_genesis_params(&mut self, g: Genesis) {
        let GenericSeal(seal_rlp) = g.seal.into();
        self.parent_hash = g.parent_hash;
        self.transactions_root = g.transactions_root;
        self.receipts_root = g.receipts_root;
        self.author = g.author;
        self.difficulty = g.difficulty;
        self.gas_limit = g.gas_limit;
        self.gas_used = g.gas_used;
        self.timestamp = g.timestamp;
        self.extra_data = g.extra_data;
        self.seal_rlp = seal_rlp;
    }

    /// Alter the value of the genesis state.
    pub fn set_genesis_state(&mut self, s: PodState) -> Result<(), Error> {
        self.genesis_state = s;
        let _ = self.run_constructors(&Default::default(), BasicBackend(MemoryDB::new()))?;

        Ok(())
    }

    /// Returns `false` if the memoized state root is invalid. `true` otherwise.
    pub fn is_state_root_valid(&self) -> bool {
        // TODO: get rid of this function and ensure state root always is valid.
        // we're mostly there, but `self.genesis_state.root()` doesn't encompass
        // post-constructor state.
        *self.state_root_memo.read() == self.genesis_state.root()
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
    pub fn load_machine<R: Read>(reader: R) -> Result<EthereumMachine, String> {
        ajson::spec::Spec::load(reader)
            .map_err(fmt_err)
            .map(load_machine_from)
    }

    /// Loads spec from json file. Provide factories for executing contracts and ensuring
    /// storage goes to the right place.
    pub fn load<'a, T: Into<SpecParams<'a>>, R>(params: T, reader: R) -> Result<Self, String>
    where R: Read {
        ajson::spec::Spec::load(reader)
            .map_err(fmt_err)
            .and_then(|x| load_from(params.into(), x).map_err(fmt_err))
    }

    /// initialize genesis epoch data, using in-memory database for
    /// constructor.
    pub fn genesis_epoch_data(&self) -> Result<Vec<u8>, String> {
        use journaldb;
        use kvdb::MockDbRepository;
        use transaction::{Action, Transaction};

        let genesis = self.genesis_header();
        let db_configs = vec!["epoch".into()];
        let factories = Default::default();
        let mut db = journaldb::new(
            Arc::new(MockDbRepository::init(db_configs)),
            journaldb::Algorithm::Archive,
            "epoch",
        );

        self.ensure_db_good(BasicBackend(db.as_hashstore_mut()), &factories)
            .map_err(|e| format!("Unable to initialize genesis state: {}", e))?;

        let call = |a, d| {
            let mut db = db.boxed_clone();
            let env_info = ::vms::EnvInfo {
                number: 0,
                author: *genesis.author(),
                timestamp: genesis.timestamp(),
                difficulty: *genesis.difficulty(),
                gas_limit: *genesis.gas_limit(),
                last_hashes: Arc::new(Vec::new()),
                gas_used: 0.into(),
            };

            let from = Address::default();
            let tx = Transaction::new(
                self.engine.account_start_nonce(0),
                U256::default(),
                U256::from(50_000_000), // TODO: share with client.
                Action::Call(a),
                U256::default(),
                d,
                DEFAULT_TRANSACTION_TYPE,
            )
            .fake_sign(from);

            let res = ::state::prove_transaction(
                db.as_hashstore_mut(),
                *genesis.state_root(),
                &tx,
                self.engine.machine(),
                &env_info,
                factories.clone(),
                true,
                Arc::new(MemoryDBRepository::new()),
            );

            res.map(|(out, proof)| (out, proof.into_iter().map(|x| x.into_vec()).collect()))
                .ok_or_else(|| "Failed to prove call: insufficient state".into())
        };

        self.engine.genesis_epoch_data(&genesis, &call)
    }

    /// Create a new Spec which conforms to the Frontier-era Morden chain except that it's a
    /// NullEngine consensus.
    pub fn new_test() -> Spec { load_bundled!("null_morden") }

    /// Create the EthereumMachine corresponding to Spec::new_test.
    pub fn new_test_machine() -> EthereumMachine { load_machine_bundled!("null_morden") }

    /// Create a new Spec which is a NullEngine consensus with a premine of address whose
    /// secret is blake2b('').
    pub fn new_null() -> Spec { load_bundled!("null") }

    /// Create a new Spec with InstantSeal consensus which does internal sealing (not requiring
    /// work).
    pub fn new_instant() -> Spec { load_bundled!("instant_seal") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use views::BlockView;
    #[test]
    fn test_load_empty() {
        assert!(Spec::load(&::std::env::temp_dir(), &[] as &[u8]).is_err());
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
            "0b10f11ef884982ebeba4e34eb4ee15126ff7f513f6d3dc55528e92c6cb86ab4".into()
        );
    }

}
