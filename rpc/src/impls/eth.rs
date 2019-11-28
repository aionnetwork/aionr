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

//! Eth rpc implementationethcore/src/state/mod.rs.

use std::sync::Arc;
use std::collections::HashMap;
use std::thread;
use std::time;

use rlp::UntrustedRlp;
use aion_types::{H128, U128, H256, U256, Address};
use serde_json::{self, Value};
use serde_json::map::Map;
use dispatch::DynamicGasPrice;

use acore::sync::SyncProvider;
use acore::account_provider::AccountProvider;
use acore::client::{MiningBlockChainClient, BlockId, TransactionId};
use acore::filter::Filter as EthcoreFilter;
use acore::header::{BlockNumber as EthBlockNumber, SealType};
use acore::log_entry::LogEntry;
use acore::miner::MinerService;
use acore::miner::external::ExternalMinerService;
use acore::transaction::SignedTransaction;
// use acore::blockchain::BlockReceipts;
use solidity::compile;

use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_core::futures::future;
use jsonrpc_macros::Trailing;

use helpers::{errors, limit_logs, fake_sign};
use helpers::dispatch::{FullDispatcher, default_gas_price};
use helpers::accounts::unwrap_provider;
use traits::Eth;
use types::{
    Block, BlockTransactions, BlockNumber, Bytes, SyncStatus, Transaction, CallRequest, Index,
Filter, Log, Receipt, Contract, ContractInfo, Abi, AbiIO , SyncInfo,
};

// const EXTRA_INFO_PROOF: &'static str = "Object exists in in blockchain (fetched earlier), extra_info is always available if object exists; qed";

/// Eth rpc implementation.
pub struct EthClient<C, S: ?Sized, M, EM>
where
    C: MiningBlockChainClient,
    S: SyncProvider,
    M: MinerService,
    EM: ExternalMinerService,
{
    client: Arc<C>,
    sync: Arc<S>,
    accounts: Option<Arc<AccountProvider>>,
    miner: Arc<M>,
    external_miner: Arc<EM>,
    dynamic_gas_price: Option<DynamicGasPrice>,
}

impl<C, S: ?Sized, M, EM> EthClient<C, S, M, EM>
where
    C: MiningBlockChainClient,
    S: SyncProvider,
    M: MinerService,
    EM: ExternalMinerService,
{
    /// Creates new EthClient.
    pub fn new(
        client: &Arc<C>,
        sync: &Arc<S>,
        accounts: &Option<Arc<AccountProvider>>,
        miner: &Arc<M>,
        em: &Arc<EM>,
        dynamic_gas_price: Option<DynamicGasPrice>,
    ) -> Self
    {
        EthClient {
            client: client.clone(),
            sync: sync.clone(),
            miner: miner.clone(),
            accounts: accounts.clone(),
            external_miner: em.clone(),
            dynamic_gas_price: dynamic_gas_price.clone(),
        }
    }

    /// Attempt to get the `Arc<AccountProvider>`, errors if provider was not
    /// set.
    fn account_provider(&self) -> Result<Arc<AccountProvider>> { unwrap_provider(&self.accounts) }

    fn block(&self, id: BlockId, include_txs: bool) -> Result<Option<Block>> {
        let client = &self.client;
        match (client.block(id.clone()), client.block_total_difficulty(id)) {
            (Some(block), Some(total_difficulty)) => {
                let view = block.header_view();
                let seal_type: SealType = view.seal_type().unwrap_or_default();
                let seal_fields: Vec<Bytes> = view.seal().into_iter().map(Into::into).collect();
                // Get seal by seal type.
                // Pending block do not yet has seal. Return empty value in this case.
                let (nonce, solution, seed, signature, public_key) = match seal_type {
                    SealType::PoW => {
                        let (nonce, solution) = if seal_fields.len() == 2 {
                            (Some(seal_fields[0].clone()), Some(seal_fields[1].clone()))
                        } else {
                            (None, None)
                        };
                        (nonce, solution, None, None, None)
                    }
                    SealType::PoS => {
                        let (seed, signature, public_key) = if seal_fields.len() == 3 {
                            (
                                Some(seal_fields[0].clone()),
                                Some(seal_fields[1].clone()),
                                Some(seal_fields[2].clone()),
                            )
                        } else {
                            (None, None, None)
                        };
                        (None, None, seed, signature, public_key)
                    }
                };
                Ok(Some(Block {
                    number: Some(view.number().into()),
                    seal_type: seal_type,
                    hash: Some(view.hash().into()),
                    parent_hash: view.parent_hash().into(),
                    miner: view.author().into(),
                    timestamp: view.timestamp().into(),
                    difficulty: view.difficulty().into(),
                    total_difficulty: Some(total_difficulty.into()),
                    size: Some(block.rlp().as_raw().len().into()),
                    gas_limit: view.gas_limit().into(),
                    gas_used: view.gas_used().into(),
                    state_root: view.state_root().into(),
                    transactions_root: view.transactions_root().into(),
                    receipts_root: view.receipts_root().into(),
                    logs_bloom: view.log_bloom().into(),
                    extra_data: Bytes::new(view.extra_data()),
                    nonce: nonce,
                    solution: solution,
                    seed: seed,
                    signature: signature,
                    public_key: public_key,
                    transactions: match include_txs {
                        true => {
                            BlockTransactions::Full(
                                block
                                    .view()
                                    .localized_transactions()
                                    .into_iter()
                                    .map(|t| {
                                        Transaction::from_localized(t, block.header().timestamp())
                                    })
                                    .collect(),
                            )
                        }
                        false => {
                            BlockTransactions::Hashes(
                                block
                                    .transaction_hashes()
                                    .into_iter()
                                    .map(Into::into)
                                    .collect(),
                            )
                        }
                    },
                }))
            }
            _ => Ok(None),
        }
    }

    fn transaction(&self, id: TransactionId) -> Result<Option<Transaction>> {
        match self.client.transaction(id) {
            Some(t) => {
                let timestamp = self
                    .client
                    .block(BlockId::Hash(t.block_hash.into()))
                    .map(|block| block.header().timestamp())
                    .unwrap_or(0);
                Ok(Some(Transaction::from_localized(t, timestamp)))
            }
            None => Ok(None),
        }
    }
}

pub fn pending_logs<M>(miner: &M, best_block: EthBlockNumber, filter: &EthcoreFilter) -> Vec<Log>
where M: MinerService {
    let receipts = miner.pending_receipts(best_block);

    let pending_logs = receipts
        .into_iter()
        .flat_map(|(hash, r)| {
            r.logs()
                .clone()
                .into_iter()
                .map(|l| (hash.clone(), l))
                .collect::<Vec<(H256, LogEntry)>>()
        })
        .collect::<Vec<(H256, LogEntry)>>();

    let result = pending_logs
        .into_iter()
        .filter(|pair| filter.matches(&pair.1))
        .map(|pair| {
            let mut log = Log::from(pair.1);
            log.transaction_hash = Some(pair.0.into());
            log
        })
        .collect();

    result
}

fn check_known<C>(client: &C, number: BlockNumber) -> Result<()>
where C: MiningBlockChainClient {
    use acore::block_status::BlockStatus;

    match client.block_status(number.into()) {
        BlockStatus::InChain => Ok(()),
        BlockStatus::Pending => Ok(()),
        _ => Err(errors::unknown_block()),
    }
}

impl<C, S: ?Sized, M, EM> Eth for EthClient<C, S, M, EM>
where
    C: MiningBlockChainClient + 'static,
    S: SyncProvider + 'static,
    M: MinerService + 'static,
    EM: ExternalMinerService + 'static,
{
    fn protocol_version(&self) -> Result<String> {
        let version = self.sync.status().protocol_version.to_owned();
        Ok(format!("{}", version))
    }

    fn syncing(&self) -> Result<SyncStatus> {
        let status = self.sync.status();
        let client = &self.client;

        let chain_info = client.chain_info();
        let current_block = chain_info.best_block_number;
        let highest_block = status.highest_block_number.unwrap_or(0u64);

        // refer to java's impl: AionImpl.java isSyncComplete.
        if (current_block + 5) < highest_block {
            let info = SyncInfo {
                // to comply with java's impl, return hex string.
                starting_block: format!("{:#x}", status.start_block_number),
                current_block: format!("{:#x}", current_block),
                highest_block: format!("{:#x}", highest_block),
            };
            Ok(SyncStatus::Info(info))
        } else {
            Ok(SyncStatus::None)
        }
    }

    fn author(&self) -> Result<H256> { Ok(H256::from(self.miner.author())) }

    fn is_mining(&self) -> Result<bool> { Ok(self.miner.is_currently_sealing()) }

    fn hashrate(&self) -> Result<String> { Ok(format!("{}", self.external_miner.hashrate())) }

    fn gas_price(&self) -> Result<U256> {
        Ok(U256::from(default_gas_price(
            &*self.client,
            &*self.miner,
            self.dynamic_gas_price.clone(),
        )))
    }

    fn accounts(&self) -> Result<Vec<H256>> {
        let store = self.account_provider()?;
        let accounts = store
            .accounts()
            .map_err(|e| errors::account("Could not fetch accounts.", e))?;
        Ok(accounts.into_iter().map(Into::into).collect::<Vec<H256>>())
    }

    fn block_number(&self) -> Result<u64> { Ok(self.client.chain_info().best_block_number) }

    fn balance(&self, address: H256, num: Trailing<BlockNumber>) -> BoxFuture<U256> {
        let address = address.into();

        let id = num.unwrap_or_default();

        try_bf!(check_known(&*self.client, id.clone()));
        let res = match self.client.balance(&address, id.into()) {
            Some(balance) => Ok(balance.into()),
            None => Err(errors::state_pruned()),
        };

        Box::new(future::done(res))
    }

    fn storage_at(
        &self,
        address: Address,
        pos: U128,
        num: Trailing<BlockNumber>,
    ) -> BoxFuture<H128>
    {
        let id = num.unwrap_or_default();

        try_bf!(check_known(&*self.client, id.clone()));
        let res = match self
            .client
            .storage_at(&address, &H128::from(pos), id.into())
        {
            Some(s) => Ok(s),
            None => Err(errors::state_pruned()),
        };

        Box::new(future::done(res))
    }

    fn transaction_count(&self, address: Address, num: Trailing<BlockNumber>) -> BoxFuture<U256> {
        let res = match num.unwrap_or_default() {
            BlockNumber::Pending => {
                let nonce = self
                    .miner
                    .last_nonce(&address)
                    .map(|n| n + U256::from(1))
                    .or_else(|| self.client.nonce(&address, BlockNumber::Pending.into()));
                match nonce {
                    Some(nonce) => Ok(nonce),
                    None => Err(errors::database("latest nonce missing")),
                }
            }
            id => {
                try_bf!(check_known(&*self.client, id.clone()));
                match self.client.nonce(&address, id.into()) {
                    Some(nonce) => Ok(nonce),
                    None => Err(errors::state_pruned()),
                }
            }
        };

        Box::new(future::done(res))
    }

    fn block_transaction_count_by_hash(&self, hash: H256) -> BoxFuture<Option<U256>> {
        Box::new(future::ok(
            self.client
                .block(BlockId::Hash(hash))
                .map(|block| block.transactions_count().into()),
        ))
    }

    fn block_transaction_count_by_number(&self, num: BlockNumber) -> BoxFuture<Option<U256>> {
        Box::new(future::ok(match num {
            BlockNumber::Pending => Some(self.miner.status().transactions_in_pending_block.into()),
            _ => {
                self.client
                    .block(num.into())
                    .map(|block| block.transactions_count().into())
            }
        }))
    }

    fn code_at(&self, address: Address, num: Trailing<BlockNumber>) -> BoxFuture<Bytes> {
        let id = num.unwrap_or_default();
        try_bf!(check_known(&*self.client, id.clone()));

        let res = match self.client.code(&address, id.into()) {
            Some(code) => Ok(code.map_or_else(Bytes::default, Bytes::new)),
            None => Err(errors::state_pruned()),
        };

        Box::new(future::done(res))
    }

    fn block_by_hash(&self, hash: H256, include_txs: bool) -> BoxFuture<Option<Block>> {
        Box::new(future::done(self.block(BlockId::Hash(hash), include_txs)))
    }

    fn block_by_number(&self, num: BlockNumber, include_txs: bool) -> BoxFuture<Option<Block>> {
        Box::new(future::done(self.block(num.into(), include_txs)))
    }

    fn transaction_by_hash(&self, hash: H256) -> BoxFuture<Option<Transaction>> {
        let block_number = self.client.chain_info().best_block_number;
        let tx = try_bf!(self.transaction(TransactionId::Hash(hash))).or_else(|| {
            self.miner
                .transaction(block_number, &hash)
                .map(|t| Transaction::from_pending(t))
        });

        Box::new(future::ok(tx))
    }

    fn transaction_by_block_hash_and_index(
        &self,
        hash: H256,
        index: Index,
    ) -> BoxFuture<Option<Transaction>>
    {
        Box::new(future::done(self.transaction(TransactionId::Location(
            BlockId::Hash(hash),
            index.value(),
        ))))
    }

    fn transaction_by_block_number_and_index(
        &self,
        num: BlockNumber,
        index: Index,
    ) -> BoxFuture<Option<Transaction>>
    {
        Box::new(future::done(
            self.transaction(TransactionId::Location(num.into(), index.value())),
        ))
    }

    fn transaction_receipt(&self, hash: H256) -> BoxFuture<Option<Receipt>> {
        let receipt = self.client.transaction_receipt(TransactionId::Hash(hash));
        Box::new(future::ok(receipt.map(Into::into)))
    }

    fn compilers(&self) -> Result<Vec<String>> { Ok(vec![String::from("solidity")]) }

    fn logs(&self, filter: Filter) -> BoxFuture<Vec<Log>> {
        let include_pending = filter.to_block == Some(BlockNumber::Pending);
        let filter: EthcoreFilter = filter.into();
        let mut logs = self
            .client
            .logs(filter.clone())
            .into_iter()
            .map(From::from)
            .collect::<Vec<Log>>();

        if include_pending {
            let best_block = self.client.chain_info().best_block_number;
            let pending = pending_logs(&*self.miner, best_block, &filter);
            logs.extend(pending);
        }

        let logs = limit_logs(logs, filter.limit);

        Box::new(future::ok(logs))
    }

    fn submit_hashrate(&self, rate: U256, id: H256) -> Result<bool> {
        self.external_miner.submit_hashrate(rate, id);
        Ok(true)
    }

    fn send_raw_transaction(&self, raw: Bytes) -> Result<H256> {
        UntrustedRlp::new(&raw.into_vec())
            .as_val()
            .map_err(errors::rlp)
            .and_then(|tx| SignedTransaction::new(tx).map_err(errors::transaction))
            .and_then(|signed_transaction| {
                debug!(target: "rpc_tx", "{:?} tx in rpc [{:?}]", thread::current().id(), time::Instant::now());
                FullDispatcher::dispatch_transaction(
                    &*self.client,
                    &*self.miner,
                    signed_transaction.into(),
                )
            })
    }

    fn submit_transaction(&self, raw: Bytes) -> Result<H256> { self.send_raw_transaction(raw) }

    fn call(&self, request: CallRequest, num: Trailing<BlockNumber>) -> BoxFuture<Bytes> {
        let request = CallRequest::into(request);
        let signed = try_bf!(fake_sign::sign_call(request));

        let num = num.unwrap_or_default();
        let result = self.client.call(&signed, Default::default(), num.into());

        Box::new(future::done(
            result.map(|b| b.output.into()).map_err(errors::call),
        ))
    }

    fn estimate_gas(&self, request: CallRequest, num: Trailing<BlockNumber>) -> BoxFuture<U256> {
        let request = CallRequest::into(request);
        let signed = try_bf!(fake_sign::sign_call(request));
        Box::new(future::done(
            self.client
                .estimate_gas(&signed, num.unwrap_or_default().into())
                .map_err(errors::call),
        ))
    }

    fn compile_solidity(&self, contract_texts: String) -> Result<HashMap<String, Contract>> {
        let field_error = errors::compilation_failed(
            "Parsing compilation result failed. Something is wrong in ther contract.".to_string(),
        );
        let mut contract_result: HashMap<String, Contract> = HashMap::new();
        match compile(contract_texts.as_bytes()) {
            Ok(result) => {
                if result.stdout == "" {
                    return Err(errors::compilation_failed(result.stderr));
                }
                match serde_json::from_str::<Value>(result.stdout.as_ref()) {
                    Ok(json) => {
                        if !json.is_object() {
                            return Err(errors::compilation_failed(
                                "Output does not fit an object.".to_string(),
                            ));
                        }
                        let language = "Solidity";
                        let language_version = "0";
                        let version: &Value = json.get("version").ok_or(field_error.clone())?;
                        if version == "" {
                            return Err(errors::compilation_failed("Version is empty.".to_string()));
                        }
                        let contracts: &Map<String, Value> = json
                            .get("contracts")
                            .ok_or(field_error.clone())?
                            .as_object()
                            .ok_or(field_error.clone())?;
                        for (contract_name, contract_json) in contracts {
                            let code: &Value =
                                contract_json.get("bin").ok_or(field_error.clone())?;
                            let abis_json_str: &str = contract_json
                                .get("abi")
                                .ok_or(field_error.clone())?
                                .as_str()
                                .ok_or(field_error.clone())?;
                            let abis_json_object: Value =
                                serde_json::from_str::<Value>(abis_json_str)
                                    .or(Err(field_error.clone()))?;
                            let abis_json: &Vec<Value> =
                                abis_json_object.as_array().ok_or(field_error.clone())?;
                            let mut abis: Vec<Abi> = Vec::new();
                            for abi_json in abis_json {
                                let abi_name = match abi_json.get("name") {
                                    Some(value) => {
                                        Some(value.as_str().ok_or(field_error.clone())?.to_owned())
                                    }
                                    _ => None,
                                };
                                let abi_type: String = abi_json
                                    .get("type")
                                    .ok_or(field_error.clone())?
                                    .as_str()
                                    .ok_or(field_error.clone())?
                                    .to_owned();
                                let constant = match abi_json.get("constant") {
                                    Some(value) => value.as_bool(),
                                    _ => None,
                                };
                                let payable = match abi_json.get("payable") {
                                    Some(value) => value.as_bool(),
                                    _ => None,
                                };
                                let anonymous = match abi_json.get("anonymous") {
                                    Some(value) => value.as_bool(),
                                    _ => None,
                                };
                                let mut inputs = match abi_json.get("inputs") {
                                    Some(value) => {
                                        let inputs_json =
                                            value.as_array().ok_or(field_error.clone())?;
                                        let mut inputs_vec: Vec<AbiIO> = Vec::new();
                                        for input_json in inputs_json {
                                            let input_name = match input_json.get("name") {
                                                Some(value) => {
                                                    Some(
                                                        value
                                                            .as_str()
                                                            .ok_or(field_error.clone())?
                                                            .to_owned(),
                                                    )
                                                }
                                                _ => None,
                                            };
                                            let input_type = match input_json.get("type") {
                                                Some(value) => {
                                                    Some(
                                                        value
                                                            .as_str()
                                                            .ok_or(field_error.clone())?
                                                            .to_string(),
                                                    )
                                                }
                                                _ => None,
                                            };
                                            let input_indexed = match input_json.get("indexed") {
                                                Some(value) => {
                                                    Some(
                                                        value
                                                            .as_bool()
                                                            .ok_or(field_error.clone())?,
                                                    )
                                                }
                                                _ => None,
                                            };
                                            inputs_vec.push(AbiIO {
                                                name: input_name,
                                                abi_io_type: input_type,
                                                indexed: input_indexed,
                                            });
                                        }
                                        Some(inputs_vec)
                                    }
                                    _ => None,
                                };
                                let mut outputs = match abi_json.get("outputs") {
                                    Some(value) => {
                                        let outputs_json =
                                            value.as_array().ok_or(field_error.clone())?;
                                        let mut outputs_vec: Vec<AbiIO> = Vec::new();
                                        for output_json in outputs_json {
                                            let output_name = match output_json.get("name") {
                                                Some(value) => {
                                                    Some(
                                                        value
                                                            .as_str()
                                                            .ok_or(field_error.clone())?
                                                            .to_owned(),
                                                    )
                                                }
                                                _ => None,
                                            };
                                            let output_type = match output_json.get("type") {
                                                Some(value) => {
                                                    Some(
                                                        value
                                                            .as_str()
                                                            .ok_or(field_error.clone())?
                                                            .to_string(),
                                                    )
                                                }
                                                _ => None,
                                            };
                                            let output_indexed = match output_json.get("indexed") {
                                                Some(value) => {
                                                    Some(
                                                        value
                                                            .as_bool()
                                                            .ok_or(field_error.clone())?,
                                                    )
                                                }
                                                _ => None,
                                            };
                                            outputs_vec.push(AbiIO {
                                                name: output_name,
                                                abi_io_type: output_type,
                                                indexed: output_indexed,
                                            });
                                        }
                                        Some(outputs_vec)
                                    }
                                    _ => None,
                                };
                                abis.push(Abi {
                                    constant: constant,
                                    inputs: inputs,
                                    name: abi_name,
                                    outputs: outputs,
                                    payable: payable,
                                    abi_type: abi_type,
                                    anonymous: anonymous,
                                });
                            }

                            contract_result.insert(
                                contract_name.to_string(),
                                Contract {
                                    code: format!(
                                        "0x{}",
                                        code.as_str().ok_or(field_error.clone())?
                                    ),
                                    info: ContractInfo {
                                        abi: abis,
                                        language_version: language_version.to_string(),
                                        language: language.to_string(),
                                        compiler_version: version.to_string(),
                                        source: contract_texts.clone(),
                                    },
                                },
                            );
                        }
                    }
                    _ => {
                        return Err(errors::compilation_failed(
                            "Parsing compilation result failed".to_string(),
                        ))
                    }
                }
            }
            _ => return Err(errors::compilation_failed("Compilation failed".to_string())),
        }
        Ok(contract_result)
    }
}
