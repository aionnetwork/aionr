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
use blake2b::{blake2b};
use aion_types::{H256, U256, U512, Address};
use vms::{ActionParams, ActionValue, CallType, EnvInfo, ExecutionResult, ExecStatus, ReturnData, ParamsType};
use crate::state::{Backend as StateBackend, State, Substate, CleanupMode};
use crate::machine::EthereumMachine as Machine;
use crate::types::error::ExecutionError;
use vms::VMType;
use vms::constants::{MAX_CALL_DEPTH, GAS_CALL_MAX, GAS_CREATE_MAX};

use crate::externalities::*;
use crate::transaction::{Action, SignedTransaction};
use crossbeam;
pub use crate::types::executed::Executed;
use crate::precompiled::builtin::{BuiltinExtImpl, BuiltinContext};
use crate::db;

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

// VM lock
lazy_static! {
    static ref VM_LOCK: Mutex<bool> = Mutex::new(false);
    static ref AVM_LOCK: Mutex<bool> = Mutex::new(false);
}

/// Returns new address created from address, nonce
pub fn contract_address(sender: &Address, nonce: &U256) -> (Address, Option<H256>) {
    use rlp::RlpStream;
    let mut stream = RlpStream::new_list(2);
    stream.append(sender);
    stream.append(nonce);
    let origin: [u8; 32] = blake2b(stream.as_raw()).into();
    let mut buffer = [0xa0u8; 32];
    &mut buffer[1..].copy_from_slice(&origin[1..]);
    (buffer.into(), None)
}

/// Transaction executor.
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

    /// Populates executive from parent properties. Increments executive depth.
    pub fn from_parent(
        state: &'a mut State<B>,
        info: &'a EnvInfo,
        machine: &'a Machine,
        parent_depth: usize,
    ) -> Self
    {
        Executive {
            state: state,
            info: info,
            machine: machine,
            depth: parent_depth + 1,
        }
    }

    /// Creates `Externalities` from `Executive`.
    pub fn as_externalities<'any>(
        &'any mut self,
        origin_info: Vec<OriginInfo>,
        substate: &'any mut Substate,
    ) -> Externalities<'any, B>
    {
        let kvdb = self.state.export_kvdb().clone();
        Externalities::new(
            self.state,
            self.info,
            self.machine,
            self.depth,
            origin_info,
            substate,
            kvdb,
        )
    }

    pub fn as_avm_externalities<'any>(
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

    /// Execute a transaction in a "virtual" context.
    /// This will ensure the caller has enough balance to execute the desired transaction.
    /// Used for extra-block executions for things like consensus contracts and RPCs
    pub fn transact_virtual(
        &'a mut self,
        t: &SignedTransaction,
        check_nonce: bool,
    ) -> Result<Executed, ExecutionError>
    {
        let sender = t.sender();
        let balance = self.state.balance(&sender)?;
        let needed_balance = t.value.saturating_add(t.gas.saturating_mul(t.gas_price));
        if balance < needed_balance {
            // give the sender a sufficient balance
            self.state
                .add_balance(&sender, &(needed_balance - balance), CleanupMode::NoEmpty)?;
        }

        self.transact(t, check_nonce, true, false)
    }

    pub fn transact_virtual_bulk(
        &'a mut self,
        txs: &[SignedTransaction],
        _check_nonce: bool,
    ) -> Vec<Result<Executed, ExecutionError>>
    {
        self.transact_bulk(txs, true, false)
    }

    // TIPS: carefully deal with errors in parallelism
    pub fn transact_bulk(
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
        let results = self.exec_avm(
            vm_params,
            &mut substates.as_mut_slice(),
            is_local_call,
            self.machine.params().unity_update,
        );

        self.avm_finalize(txs, substates.as_slice(), results)
    }

    fn exec_avm(
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
                    _ => debug!(target: "vm", "AVMExec: unknown signal"),
                }
                signal = rx.recv().expect("Unable to receive from channel");
            }

            trace!(target: "vm", "received {:?}, kill channel", signal);
        });

        // Ordinary execution - keep VM in same thread
        debug!(target: "vm", "depth threshold = {:?}", depth_threshold);
        if self.depth != depth_threshold {
            let mut vm_factory = self.state.vm_factory();
            // consider put global callback in ext
            let mut ext = self.as_avm_externalities(unconfirmed_substate, tx.clone());
            //TODO: make create/exec compatible with fastvm
            let vm = vm_factory.create(VMType::AVM);
            return vm.exec(params, &mut ext, is_local_call, unity_update);
        }

        //Start in new thread with stack size needed up to max depth
        crossbeam::scope(|scope| {
            let mut vm_factory = self.state.vm_factory();

            let mut ext = self.as_avm_externalities(unconfirmed_substate, tx.clone());

            scope
                .builder()
                .stack_size(::std::cmp::max(
                    (MAX_CALL_DEPTH as usize).saturating_sub(depth_threshold)
                        * STACK_SIZE_PER_DEPTH,
                    local_stack_size,
                ))
                .spawn(move || {
                    let vm = vm_factory.create(VMType::AVM);
                    vm.exec(params, &mut ext, is_local_call, unity_update)
                })
                .expect("Sub-thread creation cannot fail; the host might run out of resources; qed")
        })
        .join()
    }

    /// This function should be used to execute transaction.
    pub fn transact(
        &'a mut self,
        t: &SignedTransaction,
        check_nonce: bool,
        is_local_call: bool,
        is_building_block: bool,
    ) -> Result<Executed, ExecutionError>
    {
        let _vm_lock = VM_LOCK.lock().unwrap();
        let sender = t.sender();
        let nonce = self.state.nonce(&sender)?;

        // 1. Check transaction nonce
        if check_nonce && t.nonce != nonce {
            return Err(From::from(ExecutionError::InvalidNonce {
                expected: nonce,
                got: t.nonce,
            }));
        }

        // 2. Check gas limit
        // 2.1 Gas limit should not be less than the basic gas requirement
        let base_gas_required: U256 = t.gas_required();
        // AKI-174
        let gas_required_against_rejection: U256 = match self.machine.params().unity_update {
            Some(ref fork_number) if &self.info.number > fork_number => t.gas_required(),
            _ => t.gas_required_before_unity(),
        };
        if t.gas < gas_required_against_rejection {
            return Err(From::from(ExecutionError::NotEnoughBaseGas {
                required: gas_required_against_rejection,
                got: t.gas,
            }));
        }
        debug!(target: "vm", "base_gas_required = {}", base_gas_required);
        debug!(target: "vm", "gas_required_against_rejection = {}", gas_required_against_rejection);

        // 2.2 Gas limit should not exceed the maximum gas limit depending on
        // the transaction's action type
        let max_gas_limit: U256 = match t.action {
            Action::Create => GAS_CREATE_MAX,
            Action::Call(_) => GAS_CALL_MAX,
        };
        // Don't check max gas limit for local call.
        // Local node has the right (and is free) to execute "big" calls with its own resources.
        // NOTE check gas limit during mining, always try vm execution on import
        if !is_local_call && t.gas > max_gas_limit {
            return Err(From::from(ExecutionError::ExceedMaxGasLimit {
                max: max_gas_limit,
                got: t.gas,
            }));
        }

        // 2.3 Gas limit should not exceed the remaining gas limit of the current block
        if is_building_block && self.info.gas_used + t.gas > self.info.gas_limit {
            return Err(From::from(ExecutionError::BlockGasLimitReached {
                gas_limit: self.info.gas_limit,
                gas_used: self.info.gas_used,
                gas: t.gas,
            }));
        }

        // 3. Check balance, avoid unaffordable transactions
        // TODO: we might need bigints here, or at least check overflows.
        let balance: U512 = U512::from(self.state.balance(&sender)?);
        let gas_cost: U512 = t.gas.full_mul(t.gas_price);
        let total_cost: U512 = U512::from(t.value) + gas_cost;
        if balance < total_cost {
            return Err(From::from(ExecutionError::NotEnoughCash {
                required: total_cost,
                got: balance,
            }));
        }

        // Deduct the basic gas requirement and pass the remaining gas limit to VM
        // Make sure to pass non-negative gas to vm
        let init_gas: U256 = if t.gas < base_gas_required {
            U256::from(0u64)
        } else {
            t.gas - base_gas_required
        };
        let mut substate = Substate::new();

        // NOTE: there can be no invalid transactions from this point.
        // Transactions filtered above are rejected and not included in the current block.

        // Increment nonce of the sender and deduct the cost of the entire gas limit from
        // the sender's account. After VM execution, gas left (not used) shall be refunded
        // (if applicable) to the sender's account.
        // This checkpoint aims at Rejected Transaction after vm execution, aionj client specific
        self.state.checkpoint();
        self.state.inc_nonce(&sender)?;
        self.state.sub_balance(
            &sender,
            &U256::from(gas_cost),
            &mut substate.to_cleanup_mode(),
        )?;
        // Transactions are now handled in different ways depending on whether it's
        // action type is Create or Call.

        let result = match t.action {
            Action::Create => {
                let (new_address, code_hash) = contract_address(&sender, &nonce);
                let params = ActionParams {
                    code_address: new_address.clone(),
                    code_hash,
                    address: new_address,
                    sender: sender.clone(),
                    origin: sender.clone(),
                    gas: init_gas,
                    gas_price: t.gas_price,
                    value: ActionValue::Transfer(t.value),
                    code: Some(Arc::new(t.data.clone())),
                    data: None,
                    call_type: CallType::None,
                    static_flag: false,
                    params_type: ParamsType::Embedded,
                    transaction_hash: t.hash().clone(),
                    original_transaction_hash: t.hash().clone(),
                    nonce: t.nonce.low_u64(),
                };
                self.create(params, &mut substate)
            }
            Action::Call(ref address) => {
                let params = ActionParams {
                    code_address: address.clone(),
                    address: address.clone(),
                    sender: sender.clone(),
                    origin: sender.clone(),
                    gas: init_gas,
                    gas_price: t.gas_price,
                    value: ActionValue::Transfer(t.value),
                    code: self.state.code(address)?,
                    code_hash: Some(self.state.code_hash(address)?),
                    data: Some(t.data.clone()),
                    call_type: CallType::Call,
                    static_flag: false,
                    params_type: ParamsType::Separate,
                    transaction_hash: t.hash().clone(),
                    original_transaction_hash: t.hash().clone(),
                    nonce: t.nonce.low_u64(),
                };
                self.call(params, &mut substate)
            }
        };

        // finalize here!
        Ok(self.finalize(t, substate, result)?)
    }

    fn exec_vm(
        &mut self,
        params: ActionParams,
        unconfirmed_substate: &mut Substate,
    ) -> ExecutionResult
    {
        let local_stack_size = ::io::LOCAL_STACK_SIZE.with(|sz| sz.get());
        let depth_threshold =
            local_stack_size.saturating_sub(STACK_SIZE_ENTRY_OVERHEAD) / STACK_SIZE_PER_DEPTH;

        // Ordinary execution - keep VM in same thread
        debug!(target: "vm", "depth threshold = {:?}", depth_threshold);
        if self.depth != depth_threshold {
            let mut vm_factory = self.state.vm_factory();
            // consider put global callback in ext
            let mut ext = self.as_externalities(OriginInfo::from(&[&params]), unconfirmed_substate);
            //TODO: make create/exec compatible with fastvm
            let vm = vm_factory.create(VMType::FastVM);
            // fastvm local call flag is unused
            return vm
                .exec(vec![params], &mut ext, false, None)
                .first()
                .unwrap()
                .clone();
        }

        //Start in new thread with stack size needed up to max depth
        crossbeam::scope(|scope| {
            let mut vm_factory = self.state.vm_factory();

            let mut ext = self.as_externalities(
                OriginInfo::from(&[&params as &ActionParams]),
                unconfirmed_substate,
            );

            scope
                .builder()
                .stack_size(::std::cmp::max(
                    (MAX_CALL_DEPTH as usize).saturating_sub(depth_threshold)
                        * STACK_SIZE_PER_DEPTH,
                    local_stack_size,
                ))
                .spawn(move || {
                    let vm = vm_factory.create(VMType::FastVM);
                    vm.exec(vec![params], &mut ext, false, None)
                        .first()
                        .unwrap()
                        .clone()
                })
                .expect("Sub-thread creation cannot fail; the host might run out of resources; qed")
        })
        .join()
    }

    /// Calls contract function with given contract params.
    /// NOTE. It does not finalize the transaction (doesn't do refunds, nor suicides).
    /// Modifies the substate.
    /// Returns either gas_left or `vm::Error`.
    pub fn call(&mut self, params: ActionParams, substate: &mut Substate) -> ExecutionResult {
        trace!(
            target: "executive",
            "Executive::call(params={:?}) self.env_info={:?}",
            params,
            self.info
        );

        // backup current state in case of error to rollback
        self.state.checkpoint();

        if self.depth >= 1 {
            // at first, transfer value to destination
            if let ActionValue::Transfer(val) = params.value {
                // Normally balance should have been checked before.
                // In case any error still occurs here, consider this transaction as a failure
                if let Err(_) = self.state.transfer_balance(
                    &params.sender,
                    &params.address,
                    &val,
                    substate.to_cleanup_mode(),
                ) {
                    return ExecutionResult {
                        gas_left: 0.into(),
                        status_code: ExecStatus::Failure,
                        return_data: ReturnData::empty(),
                        exception: String::from("Error in balance transfer"),
                        state_root: H256::default(),
                        invokable_hashes: Default::default(),
                    };
                }
            }
        }

        let mut res = ExecutionResult::default();
        debug!(target: "executive", "default transact result = {:?}", res);

        // Builtin contract call
        if let Some(builtin) = self.machine.builtin(&params.code_address, self.info.number) {
            // Create contract account if it does yet exist (when being called for the first time)
            if self.state.exists(&params.code_address).is_ok()
                && !self.state.exists(&params.code_address).unwrap()
            {
                self.state
                    .new_contract(&params.code_address, 0.into(), 0.into());
            }
            // Engines aren't supposed to return builtins until activation, but
            // prefer to fail rather than silently break consensus.
            if !builtin.is_active(self.info.number) {
                panic!(
                    "Consensus failure: engine implementation prematurely enabled built-in at {}",
                    params.code_address
                );
            }

            // Prepare data for builtin call
            let default = [];
            let data = if let Some(ref d) = params.data {
                d as &[u8]
            } else {
                &default as &[u8]
            };

            let cost = builtin.cost(data);
            debug!(target: "vm", "builtin gas cost = {:?}", cost);
            if cost <= params.gas {
                let mut unconfirmed_substate = Substate::new();
                let mut result = {
                    let builtin_context = BuiltinContext {
                        sender: params.sender.clone(),
                        address: params.address.clone(),
                        tx_hash: params.transaction_hash.clone(),
                        origin_tx_hash: params.original_transaction_hash.clone(),
                    };
                    let mut ext =
                        BuiltinExtImpl::new(self.state, builtin_context, &mut unconfirmed_substate);
                    builtin.execute(&mut ext, data)
                };

                debug!(target: "vm", "builtin result = {:?}", result);

                if result.status_code == ExecStatus::Success {
                    result.gas_left = params.gas - cost;
                    // Handle state and substates
                    self.enact_result(&result, substate, unconfirmed_substate);
                } else {
                    self.state.revert_to_checkpoint();
                }
                res = result;
            } else {
                // If not enough gas, rollback state and return failure status
                self.state.revert_to_checkpoint();
                res = ExecutionResult {
                    gas_left: 0.into(),
                    status_code: ExecStatus::Failure,
                    return_data: ReturnData::empty(),
                    exception: String::from("Not enough gas to execute precompiled contract."),
                    state_root: H256::default(),
                    invokable_hashes: Default::default(),
                };
            }
        } else {
            if params.code.is_some() {
                // part of substate that may be reverted
                let mut unconfirmed_substate = Substate::new();

                let result = self.exec_vm(params.clone(), &mut unconfirmed_substate);

                debug!(target: "vm", "result={:?}", result);
                // Handle state and substates
                self.enact_result(&result, substate, unconfirmed_substate);
                debug!(target: "vm", "enacted: substate={:?}\n", substate);
                res = result;
            } else {
                // otherwise it's just a basic transaction.
                debug!(target: "vm", "deal with normal tx");
                self.state.discard_checkpoint();

                res = ExecutionResult {
                    gas_left: params.gas,
                    status_code: ExecStatus::Success,
                    return_data: ReturnData::empty(),
                    exception: String::default(),
                    state_root: H256::default(),
                    invokable_hashes: Default::default(),
                };
            }
        }

        debug!(target: "executive", "final transact result = {:?}", res);
        if (self.depth == 0) && (res.status_code == ExecStatus::Success) {
            // at first, transfer value to destination
            if let ActionValue::Transfer(val) = params.value {
                // Normally balance should have been checked before.
                // In case any error still occurs here, consider this transaction as a failure
                if let Err(_) = self.state.transfer_balance(
                    &params.sender,
                    &params.address,
                    &val,
                    substate.to_cleanup_mode(),
                ) {
                    return ExecutionResult {
                        gas_left: 0.into(),
                        status_code: ExecStatus::Failure,
                        return_data: ReturnData::empty(),
                        exception: String::from("Error in balance transfer"),
                        state_root: H256::default(),
                        invokable_hashes: Default::default(),
                    };
                }
            }
        }

        return res;
    }

    #[cfg(test)]
    pub fn create_avm(
        &mut self,
        params: Vec<ActionParams>,
        _substates: &mut [Substate],
    ) -> Vec<ExecutionResult>
    {
        self.state.checkpoint();

        let mut unconfirmed_substates = vec![Substate::new(); params.len()];

        let res = self.exec_avm(params, unconfirmed_substates.as_mut_slice(), false, None);

        res
    }

    #[cfg(test)]
    pub fn call_avm(
        &mut self,
        params: Vec<ActionParams>,
        _substates: &mut [Substate],
    ) -> Vec<ExecutionResult>
    {
        self.state.checkpoint();

        let mut unconfirmed_substates = vec![Substate::new(); params.len()];

        let res = self.exec_avm(params, unconfirmed_substates.as_mut_slice(), false, None);

        println!("{:?}", unconfirmed_substates);

        res
    }

    /// Creates contract with given contract params.
    /// NOTE. It does not finalize the transaction (doesn't do refunds, nor suicides).
    /// Modifies the substate.
    pub fn create(&mut self, params: ActionParams, substate: &mut Substate) -> ExecutionResult {
        // EIP-684: If a contract creation is attempted, due to either a creation transaction or the
        // CREATE (or future CREATE2) opcode, and the destination address already has either
        // nonzero nonce, or nonempty code, then the creation throws immediately, with exactly
        // the same behavior as would arise if the first byte in the init code were an invalid
        // opcode. This applies retroactively starting from genesis.

        if self
            .state
            .exists_and_not_null(&params.address)
            .unwrap_or(true)
        {
            // AKI-83: allow internal creation of contract which has balance and no code.
            let code = self.state.code(&params.address).unwrap_or(None);
            let aion040_fork = self
                .machine
                .params()
                .monetary_policy_update
                .map_or(false, |v| self.info.number >= v);
            if !aion040_fork || code.is_some() {
                //(self.depth >= 1 && code.is_some()) || self.depth == 0 {
                return ExecutionResult {
                    gas_left: 0.into(),
                    status_code: ExecStatus::Failure,
                    return_data: ReturnData::empty(),
                    exception: String::from(
                        "Contract creation address already exists, or checking contract existance \
                         failed.",
                    ),
                    state_root: H256::default(),
                    invokable_hashes: Default::default(),
                };
            }
        }

        trace!(
            target: "executive",
            "Executive::create(params={:?}) self.env_info={:?}",
            params,
            self.info
        );

        // backup used in case of running out of gas
        self.state.checkpoint();

        // part of substate that may be reverted
        let mut unconfirmed_substate = Substate::new();
        // Normally there won't be any address collision. Set new account's nonce and balance
        // to 0.
        // the nonce of a new contract account starts at 0 (according to java version implementation)
        // AKI-83
        let nonce_offset = self.state.nonce(&params.address).unwrap_or(0.into());
        let prev_bal = self.state.balance(&params.address).unwrap_or(U256::zero());
        if let ActionValue::Transfer(val) = params.value {
            // Normally balance should have been checked before.
            // In case any error still occurs here, consider this transaction as a failure
            if let Err(_) =
                self.state
                    .sub_balance(&params.sender, &val, &mut substate.to_cleanup_mode())
            {
                return ExecutionResult {
                    gas_left: 0.into(),
                    status_code: ExecStatus::Failure,
                    return_data: ReturnData::empty(),
                    exception: String::from("Error in balance transfer"),
                    state_root: H256::default(),
                    invokable_hashes: Default::default(),
                };
            }
            self.state
                .new_contract(&params.address, val + prev_bal, nonce_offset);
        } else {
            self.state
                .new_contract(&params.address, prev_bal, nonce_offset);
        }

        let res = self.exec_vm(params, &mut unconfirmed_substate);

        self.enact_result(&res, substate, unconfirmed_substate);
        debug!(target: "vm", "create res = {:?}", res);
        res
    }

    fn avm_finalize(
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
                        .get(db::COL_EXTRA, &alias[..])
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
            batch.put(db::COL_EXTRA, &k, alias_data.as_slice());
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

    /// Finalizes the transaction (does refunds and suicides).
    /// behavior of rejected transaction(gas_used > block gas remaining):
    /// 1. pay mining fee
    /// 2. remove logs and accounts created
    fn finalize(
        &mut self,
        t: &SignedTransaction,
        mut substate: Substate,
        result: ExecutionResult,
    ) -> Result<Executed, ExecutionError>
    {
        // Calculate refund and transactions fee based on gas left.
        // If return status code is not Success or Revert, the total amount of
        // gas limit will be charged.
        let gas_left = match result.status_code {
            ExecStatus::Success | ExecStatus::Revert => result.gas_left,
            _ => 0.into(),
        };

        let rejected = t.gas - gas_left + self.info.gas_used > self.info.gas_limit;

        if rejected {
            self.state.revert_to_checkpoint();
            Err(ExecutionError::BlockGasLimitReached {
                gas_limit: self.info.gas_limit,
                gas_used: self.info.gas_used + t.gas - gas_left,
                gas: t.gas,
            })
        } else {
            let gas_used = t.gas - gas_left;

            let refund_value = gas_left * t.gas_price;
            let fees_value = gas_used * t.gas_price;
            trace!(
                target: "executive",
                "exec::finalize: t.gas={}, gas_left={}, gas_used={}, refund_value={}, fees_value={}\n",
                t.gas,
                gas_left,
                gas_used,
                refund_value,
                fees_value
            );

            // Transfer refund and transaction fee.
            let sender = t.sender();
            trace!(
                target: "executive",
                "exec::finalize: Refunding refund_value={}, sender={}\n",
                refund_value,
                sender
            );
            // Below: NoEmpty is safe since the sender must already be non-null to have sent this transaction
            self.state
                .add_balance(&sender, &refund_value, CleanupMode::NoEmpty)?;
            trace!(
                target: "executive",
                "exec::finalize: Compensating author: fees_value={}, author={}\n",
                fees_value,
                &self.info.author
            );
            self.state
                .add_balance(&self.info.author, &fees_value, substate.to_cleanup_mode())?;

            // perform suicides
            for address in &substate.suicides {
                self.state.kill_account(address);
            }

            self.state.discard_checkpoint();
            Ok(Executed {
                exception: result.exception,
                gas: t.gas,
                gas_used: gas_used,
                refunded: gas_left,
                cumulative_gas_used: self.info.gas_used + gas_used,
                logs: if rejected { vec![] } else { substate.logs },
                contracts_created: if rejected {
                    vec![]
                } else {
                    substate.contracts_created
                },
                output: if rejected {
                    vec![]
                } else {
                    result.return_data.to_vec()
                },
                state_diff: None,
                transaction_fee: fees_value,
                touched: HashSet::new(),
                state_root: H256::default(),
            })
        }
    }

    /// Commit or rollback state changes based on return status code.
    /// Attach called substate to caller substate (if applicable)
    fn enact_result(
        &mut self,
        result: &ExecutionResult,
        substate: &mut Substate,
        un_substate: Substate,
    )
    {
        match result.status_code {
            // Commit state changes by discarding checkpoint only when
            // return status code is Success
            ExecStatus::Success => {
                self.state.discard_checkpoint();
                substate.accrue(un_substate);
            }
            _ => {
                // Rollback state changes by reverting to checkpoint
                self.state.revert_to_checkpoint();
            }
        }
    }
}
