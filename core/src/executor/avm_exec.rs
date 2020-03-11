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

//! Transaction Execution environment.
use std::thread;
use std::sync::mpsc::{channel, Sender};
use std::clone::Clone;
use std::sync::{Arc, Mutex};
use std::collections::{HashSet, HashMap};
use std::time::SystemTime;
use aion_types::{H256, U256, Address};
use vms::{ActionParams, ActionValue, CallType, EnvInfo, AvmExecutionResult as ExecutionResult, ParamsType};
use state::{Backend as StateBackend, State, Substate, CleanupMode};
use machine::EthereumMachine as Machine;
use types::error::ExecutionError;
use vms::constants::{MAX_CALL_DEPTH};

use executor::avm_externality::*;
use transaction::{Action, SignedTransaction};
use crossbeam;
pub use types::executed::Executed;

use kvdb::{DBTransaction};

#[cfg(debug_assertions)]
/// Roughly estimate what stack size each level of evm depth will use. (Debug build)
const STACK_SIZE_PER_DEPTH: usize = 128 * 1024;

#[cfg(not(debug_assertions))]
/// Roughly estimate what stack size each level of evm depth will use.
const STACK_SIZE_PER_DEPTH: usize = 128 * 1024;

#[cfg(debug_assertions)]
// /// Entry stack overhead prior to execution. (Debug build)
const STACK_SIZE_ENTRY_OVERHEAD: usize = 100 * 1024;

#[cfg(not(debug_assertions))]
/// Entry stack overhead prior to execution.
const STACK_SIZE_ENTRY_OVERHEAD: usize = 20 * 1024;

/// VM lock
lazy_static! {
    static ref AVM_LOCK: Mutex<bool> = Mutex::new(false);
}

pub struct Executive<'a, B: 'a + StateBackend> {
    state: &'a mut State<B>,
    info: &'a EnvInfo,
    machine: &'a Machine,
    depth: usize,
}

impl<'a, B: 'a + StateBackend> Executive<'a, B> {
    /// Basic constructor.
    pub fn new(state: &'a mut State<B>, info: &'a EnvInfo, machine: &'a Machine) -> Self {
        Executive {
            state: state,
            info: info,
            machine: machine,
            depth: 0,
        }
    }

    pub(crate) fn as_externalities<'any>(
        &'any mut self,
        substates: &'any mut [Substate],
        tx_chnnl: Sender<i32>,
    ) -> AVMExternalities<'any, B>
    {
        AVMExternalities::new(
            self.state,
            self.info,
            self.machine,
            self.depth,
            substates,
            tx_chnnl,
        )
    }

    pub fn transact_virtual(
        &'a mut self,
        txs: &[SignedTransaction],
        _check_nonce: bool,
    ) -> Vec<Result<Executed, ExecutionError>>
    {
        self.transact(txs, true, false)
    }

    /*
     * send transactions to avm
     */
    pub fn transact(
        &'a mut self,
        txs: &[SignedTransaction],
        is_local_call: bool,
        is_building_block: bool,
    ) -> Vec<Result<Executed, ExecutionError>>
    {
        let _vm_lock = AVM_LOCK.lock().unwrap();
        let mut vm_params = Vec::new();

        for t in txs {
            let sender = t.sender();
            let nonce = t.nonce;

            let init_gas = t.gas;

            if is_local_call {
                let sender = t.sender();
                let balance = self.state.balance(&sender).unwrap_or(0.into());
                let needed_balance = t.value.saturating_add(t.gas.saturating_mul(t.gas_price));
                if balance < needed_balance {
                    // give the sender a sufficient balance
                    let _ =
                        self.state
                            .add_balance(&sender, &(needed_balance), CleanupMode::NoEmpty);
                }
                debug!(target: "vm", "sender: {:?}, balance: {:?}", sender, self.state.balance(&sender).unwrap_or(0.into()));
            } else if is_building_block && self.info.gas_used + t.gas > self.info.gas_limit {
                // check gas limit
                return vec![Err(From::from(ExecutionError::BlockGasLimitReached {
                    gas_limit: self.info.gas_limit,
                    gas_used: self.info.gas_used,
                    gas: t.gas,
                }))];
            }

            // Transactions are now handled in different ways depending on whether it's
            // action type is Create or Call.
            let params = match t.action {
                Action::Create => {
                    ActionParams {
                        code_address: Address::default(),
                        code_hash: None,
                        address: Address::default(),
                        sender: sender.clone(),
                        origin: sender.clone(),
                        gas: init_gas,
                        gas_price: t.gas_price,
                        value: ActionValue::Transfer(t.value),
                        code: Some(Arc::new(t.data.clone())),
                        data: None,
                        call_type: CallType::None,
                        transaction_hash: t.hash().to_owned(),
                        original_transaction_hash: t.hash().to_owned(),
                        nonce: nonce.low_u64(),
                        static_flag: false,
                        params_type: ParamsType::Embedded,
                    }
                }
                Action::Call(ref address) => {
                    let call_type = match self.state.code(&address).unwrap().is_some() {
                        true => CallType::Call,
                        false => CallType::BulkBalance,
                    };

                    ActionParams {
                        code_address: address.clone(),
                        address: address.clone(),
                        sender: sender.clone(),
                        origin: sender.clone(),
                        gas: init_gas,
                        gas_price: t.gas_price,
                        value: ActionValue::Transfer(t.value),
                        code: self.state.code(address).unwrap(),
                        code_hash: Some(self.state.code_hash(address).unwrap()),
                        data: Some(t.data.clone()),
                        call_type,
                        transaction_hash: t.hash().to_owned(),
                        original_transaction_hash: t.hash().to_owned(),
                        nonce: nonce.low_u64(),
                        params_type: ParamsType::Embedded,
                        static_flag: false,
                    }
                }
            };
            vm_params.push(params);
        }

        let mut substates = vec![Substate::new(); vm_params.len()];
        let now = SystemTime::now();
        let results = self.exec_vm(
            vm_params,
            &mut substates.as_mut_slice(),
            is_local_call,
            self.machine.params().unity_update,
        );
        trace!(target: "vm", "exec duration: {:?}ms", now.elapsed().map(|e| e.subsec_millis()));

        self.finalize(txs, substates.as_slice(), results)
    }

    fn exec_vm(
        &mut self,
        params: Vec<ActionParams>,
        unconfirmed_substate: &mut [Substate],
        is_local_call: bool,
        unity_update: Option<u64>,
    ) -> Vec<ExecutionResult>
    {
        let local_stack_size = ::io::LOCAL_STACK_SIZE.with(|sz| sz.get());
        let depth_threshold =
            local_stack_size.saturating_sub(STACK_SIZE_ENTRY_OVERHEAD) / STACK_SIZE_PER_DEPTH;

        // start a new thread to listen avm signal
        let (tx, rx) = channel();
        thread::spawn(move || {
            let mut signal = rx.recv().expect("Unable to receive from channel");
            while signal >= 0 {
                match signal {
                    0 => debug!(target: "vm", "AVMExec: commit state"),
                    1 => debug!(target: "vm", "AVMExec: get state"),
                    _ => println!("unknown signal"),
                }
                signal = rx.recv().expect("Unable to receive from channel");
            }

            trace!(target: "vm", "received {:?}, kill channel", signal);
        });

        // Ordinary execution - keep VM in same thread
        debug!(target: "vm", "depth threshold = {:?}", depth_threshold);
        if self.depth != depth_threshold {
            let mut vm_factory = self.state.vm_factory();
            let mut ext = self.as_externalities(unconfirmed_substate, tx.clone());
            let mut vm = vm_factory.create_avm();
            return vm.exec(params, &mut ext, is_local_call, unity_update);
        }

        //Start in new thread with stack size needed up to max depth
        crossbeam::scope(|scope| {
            let mut vm_factory = self.state.vm_factory();

            let mut ext = self.as_externalities(unconfirmed_substate, tx.clone());

            scope
                .builder()
                .stack_size(::std::cmp::max(
                    (MAX_CALL_DEPTH as usize).saturating_sub(depth_threshold)
                        * STACK_SIZE_PER_DEPTH,
                    local_stack_size,
                ))
                .spawn(move || {
                    let mut vm = vm_factory.create_avm();
                    vm.exec(params, &mut ext, is_local_call, unity_update)
                })
                .expect("Sub-thread creation cannot fail; the host might run out of resources; qed")
        })
        .join()
    }

    #[cfg(test)]
    pub fn create_vm(
        &mut self,
        params: Vec<ActionParams>,
        _substates: &mut [Substate],
    ) -> Vec<ExecutionResult>
    {
        self.state.checkpoint();

        let mut unconfirmed_substates = vec![Substate::new(); params.len()];

        let res = self.exec_vm(params, unconfirmed_substates.as_mut_slice(), false, None);

        res
    }

    #[cfg(test)]
    pub fn call_vm(
        &mut self,
        params: Vec<ActionParams>,
        _substates: &mut [Substate],
    ) -> Vec<ExecutionResult>
    {
        self.state.checkpoint();

        let mut unconfirmed_substates = vec![Substate::new(); params.len()];

        let res = self.exec_vm(params, unconfirmed_substates.as_mut_slice(), false, None);

        println!("{:?}", unconfirmed_substates);

        res
    }

    fn finalize(
        &mut self,
        txs: &[SignedTransaction],
        substates: &[Substate],
        results: Vec<ExecutionResult>,
    ) -> Vec<Result<Executed, ExecutionError>>
    {
        assert_eq!(txs.len(), results.len());

        let mut final_results = Vec::new();

        let mut total_gas_used: U256 = U256::from(0);
        let mut multiple_sets: HashMap<H256, HashSet<H256>> = HashMap::new();
        for idx in 0..txs.len() {
            let result = results.get(idx).unwrap().clone();
            let t = txs[idx].clone();
            let substate = substates[idx].clone();
            // perform suicides
            for address in &substate.suicides {
                self.state.kill_account(address);
            }

            let gas_used = t.gas - result.gas_left;

            //TODO: check whether avm has already refunded
            //let refund_value = gas_left * t.gas_price;
            let fees_value = gas_used * t.gas_price;

            let mut touched = HashSet::new();
            for account in substate.touched {
                touched.insert(account);
            }

            if gas_used + total_gas_used + self.info.gas_used > self.info.gas_limit {
                final_results.push(Err(ExecutionError::BlockGasLimitReached {
                    gas_limit: self.info.gas_limit,
                    gas_used: self.info.gas_used + total_gas_used,
                    gas: t.gas,
                }));
            } else {
                total_gas_used = total_gas_used + gas_used;
                final_results.push(Ok(Executed {
                    exception: result.exception,
                    gas: t.gas,
                    gas_used: gas_used,
                    refunded: result.gas_left,
                    cumulative_gas_used: self.info.gas_used + gas_used,
                    logs: substate.logs,
                    contracts_created: substate.contracts_created,
                    output: result.return_data.to_vec(),
                    state_diff: None,
                    transaction_fee: fees_value,
                    touched: touched,
                    state_root: result.state_root,
                }))
            }

            // store Meta transaction hashes
            // encode as: b"alias" + hash + hash + ...
            for (alias, tx_hash) in result.invokable_hashes {
                let mut set = if let Some(ref mut set) = multiple_sets.get_mut(&alias) {
                    set.clone()
                } else {
                    let mut set = HashSet::new();
                    match self
                        .state
                        .export_kvdb()
                        .get(::db::COL_EXTRA, &alias[..])
                        .unwrap()
                    {
                        Some(invoked_set) => {
                            Self::decode_alias_and_set(&invoked_set[..], &mut set);
                        }
                        None => {}
                    }
                    set
                };

                set.insert(tx_hash);
                multiple_sets.insert(alias, set);
            }
        }

        debug!(target: "vm", "meta alias sets: {:?}", multiple_sets);

        // store alias sets
        for (k, set) in multiple_sets.drain() {
            // Step 1: encode alias set
            let mut alias_data = Vec::new();
            alias_data.append(&mut b"alias".to_vec());
            for mut hash in set {
                alias_data.append(&mut hash[..].to_vec());
            }

            // Step 2: write into database
            let mut batch = DBTransaction::new();
            batch.put(::db::COL_EXTRA, &k, alias_data.as_slice());
            self.state
                .export_kvdb()
                .write(batch)
                .expect("EXTRA DB write failed");
        }

        return final_results;
    }

    fn decode_alias_and_set(raw_set: &[u8], set: &mut HashSet<H256>) {
        assert!(raw_set.len() >= 5);
        let mut index = 5;
        while index <= raw_set.len() - 32 {
            set.insert(raw_set[index..(index + 32)].into());
            index += 32;
        }
    }
}
