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
use std::sync::{Arc, Mutex};
use blake2b::blake2b;
use aion_types::{H256, U256, U512, Address};
#[cfg(test)]
use aion_types::U128;
use state::{Backend as StateBackend, State, Substate, CleanupMode};
use machine::EthereumMachine as Machine;
use error::ExecutionError;
use vms::{
    self,
    ActionParams,
    ActionValue,
    CallType,
    EnvInfo,
    AVMActionParams,
    ExecutionResult,
    ExecStatus,
    ReturnData,
    VMType
};
use vms::constants::{MAX_CALL_DEPTH, GAS_CALL_MAX, GAS_CREATE_MAX};

use externalities::*;
use transaction::{Action, SignedTransaction};
use crossbeam;
pub use executed::Executed;
use precompiled::builtin::{BuiltinExtImpl, BuiltinContext};

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
    static ref VM_LOCK: Mutex<bool> = Mutex::new(false);
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
    ) -> AVMExternalities<'any, B>
    {
        AVMExternalities::new(
            self.state,
            self.info,
            self.machine,
            self.depth,
            substates,
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

        self.transact(t, check_nonce, true)
    }

    pub fn transact_virtual_bulk(
        &'a mut self,
        txs: &[SignedTransaction],
        check_nonce: bool,
    ) -> Vec<Result<Executed, ExecutionError>>
    {
        self.transact_bulk(txs, check_nonce, true)
    }

    pub fn transact_bulk(
        &'a mut self,
        txs: &[SignedTransaction],
        check_nonce: bool,
        is_local_call: bool,
    ) -> Vec<Result<Executed, ExecutionError>>
    {
        let mut vm_params = Vec::new();
        // validate transactions
        for t in txs {
            let sender = t.sender();
            let nonce = self.state.nonce(&sender).unwrap();

            // 1. Check transaction nonce
            if check_nonce && t.nonce != nonce {
                return vec![Err(From::from(ExecutionError::InvalidNonce {
                    expected: nonce,
                    got: t.nonce,
                }))];
            }

            // 2. Check gas limit
            // 2.1 Gas limit should not be less than the basic gas requirement
            let base_gas_required: U256 = t.gas_required();
            if t.gas < base_gas_required {
                return vec![Err(From::from(ExecutionError::NotEnoughBaseGas {
                    required: base_gas_required,
                    got: t.gas,
                }))];
            }
            debug!(target: "vm", "base_gas_required = {}", base_gas_required);

            // 2.2 Gas limit should not exceed the maximum gas limit depending on
            // the transaction's action type
            let max_gas_limit: U256 = match t.action {
                Action::Create => GAS_CREATE_MAX,
                Action::Call(_) => GAS_CALL_MAX,
            };

            // Don't check max gas limit for local call.
            // Local node has the right (and is free) to execute "big" calls with its own resources.
            if !is_local_call && t.gas > max_gas_limit {
                return vec![Err(From::from(ExecutionError::ExceedMaxGasLimit {
                    max: max_gas_limit,
                    got: t.gas,
                }))];
            }

            // 2.3 Gas limit should not exceed the remaining gas limit of the current block
            if self.info.gas_used + t.gas > self.info.gas_limit {
                return vec![Err(From::from(ExecutionError::BlockGasLimitReached {
                    gas_limit: self.info.gas_limit,
                    gas_used: self.info.gas_used,
                    gas: t.gas,
                }))];
            }

            // 3. Check balance, avoid unaffordable transactions
            // TODO: we might need bigints here, or at least check overflows.
            let balance: U512 = U512::from(self.state.balance(&sender).unwrap());
            let gas_cost: U512 = t.gas.full_mul(t.gas_price);
            let total_cost: U512 = U512::from(t.value) + gas_cost;
            if balance < total_cost {
                return vec![Err(From::from(ExecutionError::NotEnoughCash {
                    required: total_cost,
                    got: balance,
                }))];
            }

            //TODO: gas limit for AVM; validate passed, just run AION VM
            let init_gas = t.gas - base_gas_required;

            // Transactions are now handled in different ways depending on whether it's
            // action type is Create or Call.
            let params = match t.action {
                Action::Create => {
                    AVMActionParams {
                        code_address: Address::default(),
                        code_hash: None,
                        address: Address::default(),
                        sender: sender.clone(),
                        origin: sender.clone(),
                        gas: init_gas,
                        gas_price: t.gas_price,
                        value: t.value,
                        code: Some(Arc::new(t.data.clone())),
                        data: None,
                        call_type: CallType::None,
                        transaction_hash: t.hash(),
                        original_transaction_hash: t.hash(),
                        nonce: nonce.low_u64(),
                    }
                }
                Action::Call(ref address) => {
                    AVMActionParams {
                        code_address: address.clone(),
                        address: address.clone(),
                        sender: sender.clone(),
                        origin: sender.clone(),
                        gas: init_gas,
                        gas_price: t.gas_price,
                        value: t.value,
                        code: self.state.code(address).unwrap(),
                        code_hash: Some(self.state.code_hash(address).unwrap()),
                        data: Some(t.data.clone()),
                        call_type: CallType::Call,
                        transaction_hash: t.hash(),
                        original_transaction_hash: t.hash(),
                        nonce: nonce.low_u64(),
                    }
                }
            };
            vm_params.push(params);
        }

        let mut substates = vec![Substate::new(); vm_params.len()];
        let results = self.exec_avm(vm_params, &mut substates.as_mut_slice());

        // enact results and update state separately

        self.avm_finalize(txs, substates.as_slice(), results)
    }

    fn exec_avm(
        &mut self,
        params: Vec<AVMActionParams>,
        unconfirmed_substate: &mut [Substate],
    ) -> Vec<ExecutionResult>
    {
        let local_stack_size = ::io::LOCAL_STACK_SIZE.with(|sz| sz.get());
        let depth_threshold =
            local_stack_size.saturating_sub(STACK_SIZE_ENTRY_OVERHEAD) / STACK_SIZE_PER_DEPTH;

        // Ordinary execution - keep VM in same thread
        debug!(target: "vm", "depth threshold = {:?}", depth_threshold);
        if self.depth != depth_threshold {
            let mut vm_factory = self.state.vm_factory();
            // consider put global callback in ext
            let mut ext = self
                .as_avm_externalities(unconfirmed_substate);
            //TODO: make create/exec compatible with fastvm
            let vm = vm_factory.create(VMType::AVM);
            return vm.exec_v1(params, &mut ext);
        }

        //Start in new thread with stack size needed up to max depth
        crossbeam::scope(|scope| {
            let mut vm_factory = self.state.vm_factory();

            let mut ext = self
                .as_avm_externalities(unconfirmed_substate);

            scope
                .builder()
                .stack_size(::std::cmp::max(
                    (MAX_CALL_DEPTH as usize).saturating_sub(depth_threshold)
                        * STACK_SIZE_PER_DEPTH,
                    local_stack_size,
                ))
                .spawn(move || {
                    let vm = vm_factory.create(VMType::AVM);
                    vm.exec_v1(params, &mut ext)
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
        if t.gas < base_gas_required {
            return Err(From::from(ExecutionError::NotEnoughBaseGas {
                required: base_gas_required,
                got: t.gas,
            }));
        }
        debug!(target: "vm", "base_gas_required = {}", base_gas_required);

        // 2.2 Gas limit should not exceed the maximum gas limit depending on
        // the transaction's action type
        let max_gas_limit: U256 = match t.action {
            Action::Create => GAS_CREATE_MAX,
            Action::Call(_) => GAS_CALL_MAX,
        };
        // Don't check max gas limit for local call.
        // Local node has the right (and is free) to execute "big" calls with its own resources.
        if !is_local_call && t.gas > max_gas_limit {
            return Err(From::from(ExecutionError::ExceedMaxGasLimit {
                max: max_gas_limit,
                got: t.gas,
            }));
        }

        // 2.3 Gas limit should not exceed the remaining gas limit of the current block
        if self.info.gas_used + t.gas > self.info.gas_limit {
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
        let init_gas = t.gas - base_gas_required;
        let mut substate = Substate::new();

        // NOTE: there can be no invalid transactions from this point.
        // Transactions filtered above are rejected and not included in the current block.

        // Increment nonce of the sender and deduct the cost of the entire gas limit from
        // the sender's account. After VM execution, gas left (not used) shall be refunded
        // (if applicable) to the sender's account.
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
                    code_hash: code_hash,
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
                    params_type: vms::ParamsType::Embedded,
                    transaction_hash: t.hash(),
                    original_transaction_hash: t.hash(),
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
                    params_type: vms::ParamsType::Separate,
                    transaction_hash: t.hash(),
                    original_transaction_hash: t.hash(),
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
            let mut ext =
                self.as_externalities(OriginInfo::from(&[params.clone()]), unconfirmed_substate);
            //TODO: make create/exec compatible with fastvm
            let vm = vm_factory.create(VMType::FastVM);
            return vm.exec(vec![params], &mut ext).first().unwrap().clone();
        }

        //Start in new thread with stack size needed up to max depth
        crossbeam::scope(|scope| {
            let mut vm_factory = self.state.vm_factory();

            let mut ext =
                self.as_externalities(OriginInfo::from(&[params.clone()]), unconfirmed_substate);

            scope
                .builder()
                .stack_size(::std::cmp::max(
                    (MAX_CALL_DEPTH as usize).saturating_sub(depth_threshold)
                        * STACK_SIZE_PER_DEPTH,
                    local_stack_size,
                ))
                .spawn(move || {
                    let vm = vm_factory.create(VMType::FastVM);
                    vm.exec(vec![params], &mut ext).first().unwrap().clone()
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
                    };
                }
            }
        }

        return res;
    }

    pub fn create_avm(
        &mut self,
        params: Vec<AVMActionParams>,
        substates: &mut [Substate],
    ) -> Vec<ExecutionResult>
    {
        self.state.checkpoint();

        let mut unconfirmed_substates = vec![Substate::new(); params.len()];

        let res = self.exec_avm(params, unconfirmed_substates.as_mut_slice());

        res
    }

    pub fn call_avm(
        &mut self,
        params: Vec<AVMActionParams>,
        substates: &mut [Substate],
    ) -> Vec<ExecutionResult>
    {
        self.state.checkpoint();

        let mut unconfirmed_substates = vec![Substate::new(); params.len()];

        let res = self.exec_avm(params, unconfirmed_substates.as_mut_slice());

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
            return ExecutionResult {
                gas_left: 0.into(),
                status_code: ExecStatus::Failure,
                return_data: ReturnData::empty(),
                exception: String::from(
                    "Contract creation address already exists, or checking contract existance \
                     failed.",
                ),
            };
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
        let nonce_offset = 0.into();
        // let prev_bal = self.state.balance(&params.address)?;
        let prev_bal = 0.into();
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

        for idx in 0..txs.len() {
            let result = results.get(idx).unwrap().clone();
            let t = txs[idx].clone();
            let substate = substates[idx].clone();
            // perform suicides
            for address in &substate.suicides {
                self.state.avm_mgr().kill_account(address);
            }
            let gas_left = match result.status_code {
                ExecStatus::Success | ExecStatus::Revert => result.gas_left,
                _ => 0.into(),
            };
            let gas_used = t.gas - gas_left;
            //TODO: check whether avm has already refunded
            //let refund_value = gas_left * t.gas_price;
            let fees_value = gas_used * t.gas_price;

            final_results.push(Ok(Executed {
                exception: result.exception,
                gas: t.gas,
                gas_used: gas_used,
                refunded: gas_left,
                cumulative_gas_used: self.info.gas_used + gas_used,
                logs: substate.logs,
                contracts_created: substate.contracts_created,
                output: result.return_data.mem,
                state_diff: None,
                transaction_fee: fees_value,
            }))
        }

        return final_results;
    }

    /// Finalizes the transaction (does refunds and suicides).
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

        Ok(Executed {
            exception: result.exception,
            gas: t.gas,
            gas_used: gas_used,
            refunded: gas_left,
            cumulative_gas_used: self.info.gas_used + gas_used,
            logs: substate.logs,
            contracts_created: substate.contracts_created,
            output: result.return_data.mem,
            state_diff: None,
            transaction_fee: fees_value,
        })
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

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use std::sync::Arc;
    use rustc_hex::FromHex;
    use super::*;
    use aion_types::{
        U256,
Address};
    use vms::{ActionParams, ActionValue, CallType, EnvInfo};
    use machine::EthereumMachine;
    use state::{Substate, CleanupMode};
    use tests::helpers::*;
    use transaction::{Action, Transaction, SignedTransaction};
    use bytes::Bytes;
    use error::ExecutionError;
    use avm_abi::{AbiToken, AVMEncoder, ToBytes};

    fn make_aion_machine() -> EthereumMachine {
        let machine = ::ethereum::new_aion_test_machine();
        machine
    }

    #[test]
    fn test_contract_address() {
        let address = Address::from_slice(b"0f572e5295c57f15886f9b263e2f6d2d6c7b5ec6");
        let expected_address = Address::from_slice(
            "a016b8bcce0d4c68b7e8c92ffd89ac633124c30447cd5cf48c1eb264308d5afb"
                .from_hex()
                .unwrap()
                .as_slice(),
        ); //Address::from_slice(b"3f09c73a5ed19289fb9bdc72f1742566df146f56");
        assert_eq!(
            expected_address,
            contract_address(&address, &U256::from(88),).0
        );
    }

    #[test]
    // Tracing is not suported in JIT
    fn bytearraymap_test() {
        let code = "60506040526000356c01000000000000000000000000900463ffffffff16806326121ff01461004957806375ed12351461005f578063e2179b8e146100fd57610043565b60006000fd5b34156100555760006000fd5b61005d61018d565b005b341561006b5760006000fd5b6100816004808035906010019091905050610275565b6040518080601001828103825283818151815260100191508051906010019080838360005b838110156100c25780820151818401525b6010810190506100a6565b50505050905090810190600f1680156100ef5780820380516001836010036101000a031916815260100191505b509250505060405180910390f35b34156101095760006000fd5b61011161032c565b6040518080601001828103825283818151815260100191508051906010019080838360005b838110156101525780820151818401525b601081019050610136565b50505050905090810190600f16801561017f5780820380516001836010036101000a031916815260100191505b509250505060405180910390f35b6101956103f5565b6104006040518059106101a55750595b9080825280601002601001820160405280156101bc575b5090506f610000000000000000000000000000008160008151811015156101df57fe5b9060100101906effffffffffffffffffffffffffffff1916908160001a9053506f62000000000000000000000000000000816103ff81518110151561022057fe5b9060100101906effffffffffffffffffffffffffffff1916908160001a9053508060006000506000602081526010019081526010016000209050600050908051906010019061027092919061040c565b505b50565b600060005060105280600052602060002090506000915090508054600181600116156101000203166002900480600f0160108091040260100160405190810160405280929190818152601001828054600181600116156101000203166002900480156103245780600f106102f757610100808354040283529160100191610324565b8201919060005260106000209050905b81548152906001019060100180831161030757829003600f168201915b505050505081565b6103346103f5565b600060005060006020815260100190815260100160002090506000508054600181600116156101000203166002900480600f0160108091040260100160405190810160405280929190818152601001828054600181600116156101000203166002900480156103e65780600f106103b9576101008083540402835291601001916103e6565b8201919060005260106000209050905b8154815290600101906010018083116103c957829003600f168201915b505050505090506103f2565b90565b601060405190810160405280600081526010015090565b8280546001816001161561010002031660029004906000526010600020905090600f016010900481019282600f1061044f57805160ff1916838001178555610482565b82800160010185558215610482579182015b828111156104815782518260005090905591601001919060010190610461565b5b50905061048f9190610493565b5090565b6104bb919061049d565b808211156104b7576000818150600090555060010161049d565b5090565b905600a165627a7a72305820a9b457c98ced88e9dda94a6ec2b32e69b1dc8ed693342b427da048636174f4c60029".from_hex().unwrap();

        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;
        println!("test address = {:?}", address);
        // TODO: add tests for 'callcreate'
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        params.data = Some("26121ff0".from_hex().unwrap());
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);
        let machine = make_aion_machine();
        let mut substate = Substate::new();

        let ExecutionResult {
            gas_left,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };

        assert_eq!(status_code, ExecStatus::Success);
        assert_eq!(gas_left, U256::from(441091));

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        params.data = Some("e2179b8e".from_hex().unwrap());

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };

        assert_eq!(status_code, ExecStatus::Success);
        let expected_data = "000000000000000000000000000000100000000000000000000000000000040061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000062".from_hex().unwrap();
        let expected_result = vms::ReturnData::new(expected_data.clone(), 0, expected_data.len());
        assert_eq!(return_data, expected_result);
    }

    #[test]
    fn test_create_contract() {
        // Tracing is not supported in JIT
        // code:
        //
        // 60 10 - push 16
        // 80 - duplicate first stack item
        // 60 0c - push 12
        // 60 00 - push 0
        // 39 - copy current code to memory
        // 60 00 - push 0
        // f3 - return

        let code = "601080600c6000396000f3006000355415600957005b60203560003555"
            .from_hex()
            .unwrap();

        println!("start create_contract test");
        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;
        // TODO: add tests for 'callcreate'
        println!("new contract addr = {:?}", address);
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(100_000);
        params.code = Some(Arc::new(code));
        params.value = ActionValue::Transfer(0.into());
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let info = EnvInfo::default();
        let machine = make_aion_machine();
        let mut substate = Substate::new();

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create(params, &mut substate)
        };

        assert_eq!(status_code, ExecStatus::Success);
    }

    #[test]
    // Tracing is not suported in JIT
    fn fibonacci() {
        let code = "60506040526000356c01000000000000000000000000900463ffffffff1680631dae897214610054578063231e93d41461008c5780639d4cd86c146100c4578063ff40565e146100fc5761004e565b60006000fd5b34156100605760006000fd5b6100766004808035906010019091905050610134565b6040518082815260100191505060405180910390f35b34156100985760006000fd5b6100ae6004808035906010019091905050610264565b6040518082815260100191505060405180910390f35b34156100d05760006000fd5b6100e660048080359060100190919050506102ae565b6040518082815260100191505060405180910390f35b34156101085760006000fd5b61011e60048080359060100190919050506102da565b6040518082815260100191505060405180910390f35b600061013e610344565b60006001841115156101565783925061025d5661025c565b600184016040518059106101675750595b90808252806010026010018201604052801561017e575b509150600082600081518110151561019257fe5b9060100190601002019090818152601001505060018260018151811015156101b657fe5b90601001906010020190908181526010015050600290505b838111151561023d5781600282038151811015156101e857fe5b90601001906010020151826001830381518110151561020357fe5b9060100190601002015101828281518110151561021c57fe5b906010019060100201909081815260100150505b80600101905080506101ce565b818481518110151561024b57fe5b90601001906010020151925061025d565b5b5050919050565b600060018211151561027c578190506102a9566102a8565b61028e6002830361026463ffffffff16565b6102a06001840361026463ffffffff16565b0190506102a9565b5b919050565b600060328211156102bf5760006000fd5b6102ce826102da63ffffffff16565b90506102d5565b919050565b600060006000600060006001861115156102fa5785945061033b5661033a565b600193506001925060009150600290505b858110156103325782840191508150829350835081925082505b806001019050805061030b565b82945061033b565b5b50505050919050565b6010604051908101604052806000815260100150905600a165627a7a72305820f201655c186b2446b8c23b5699eabfb66793e2b97e8cadb17b212b3bc71afe600029".from_hex().unwrap();

        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "ff40565e".from_hex().unwrap();
        call_data.append(&mut <[u8; 16]>::from(U128::from(6)).to_vec());
        params.data = Some(call_data);
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);
        let machine = make_aion_machine();
        let mut substate = Substate::new();

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            println!("call executor");
            ex.call(params, &mut substate)
        };

        assert_eq!(status_code, ExecStatus::Success);

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "231e93d4".from_hex().unwrap();
        call_data.append(&mut <[u8; 16]>::from(U128::from(6)).to_vec());
        params.data = Some(call_data);

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };

        assert_eq!(status_code, ExecStatus::Success);

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "1dae8972".from_hex().unwrap();
        call_data.append(&mut <[u8; 16]>::from(U128::from(6)).to_vec());
        params.data = Some(call_data);

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };

        assert_eq!(status_code, ExecStatus::Success);

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "9d4cd86c".from_hex().unwrap();
        call_data.append(&mut <[u8; 16]>::from(U128::from(6)).to_vec());
        params.data = Some(call_data);

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };

        assert_eq!(status_code, ExecStatus::Success);

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "9d4cd86c".from_hex().unwrap();
        call_data.append(&mut <[u8; 16]>::from(U128::from(1024)).to_vec());
        params.data = Some(call_data);

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };

        assert_eq!(status_code, ExecStatus::Revert);
    }

    #[test]
    // Tracing is not suported in JIT
    fn recursive() {
        let code = "605060405234156100105760006000fd5b610015565b610199806100246000396000f30060506040526000356c01000000000000000000000000900463ffffffff1680632d7df21a146100335761002d565b60006000fd5b341561003f5760006000fd5b6100666004808080601001359035909160200190919290803590601001909190505061007c565b6040518082815260100191505060405180910390f35b6000600060007f66fa32225b641331dff20698cd66d310b3149e86d875926af7ea2f2a9079e80b856040518082815260100191505060405180910390a18585915091506001841115156100d55783925061016456610163565b60018282632d7df21a898960018a036000604051601001526040518463ffffffff166c010000000000000000000000000281526004018084848252816010015260200182815260100193505050506010604051808303816000888881813b151561013f5760006000fd5b5af1151561014d5760006000fd5b5050505060405180519060100150019250610164565b5b505093925050505600a165627a7a72305820c4755a8b960e01280a2c8d85fae255d08e1be318b2c2685a948e7b42660c2f5c0029".from_hex().unwrap();

        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(100_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(0.into());
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let info = EnvInfo::default();
        let machine = make_aion_machine();
        let mut substate = Substate::new();

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create(params, &mut substate)
        };

        let ReturnData {
            mem,
            offset: _,
            size: _,
        } = return_data;

        assert_eq!(mem, "60506040526000356c01000000000000000000000000900463ffffffff1680632d7df21a146100335761002d565b60006000fd5b341561003f5760006000fd5b6100666004808080601001359035909160200190919290803590601001909190505061007c565b6040518082815260100191505060405180910390f35b6000600060007f66fa32225b641331dff20698cd66d310b3149e86d875926af7ea2f2a9079e80b856040518082815260100191505060405180910390a18585915091506001841115156100d55783925061016456610163565b60018282632d7df21a898960018a036000604051601001526040518463ffffffff166c010000000000000000000000000281526004018084848252816010015260200182815260100193505050506010604051808303816000888881813b151561013f5760006000fd5b5af1151561014d5760006000fd5b5050505060405180519060100150019250610164565b5b505093925050505600a165627a7a72305820c4755a8b960e01280a2c8d85fae255d08e1be318b2c2685a948e7b42660c2f5c0029".from_hex().unwrap());
        assert_eq!(status_code, ExecStatus::Success);

        let code = mem.clone();

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "2d7df21a".from_hex().unwrap();
        println!("contract address = {:?}", address);
        call_data.append(&mut <[u8; 32]>::from(address.clone()).to_vec());
        call_data.append(&mut <[u8; 16]>::from(U128::from(2)).to_vec());
        params.data = Some(call_data);
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };

        assert_eq!(status_code, ExecStatus::Success);

        let code = "60506040526000356c01000000000000000000000000900463ffffffff1680632d7df21a146100335761002d565b60006000fd5b341561003f5760006000fd5b6100666004808080601001359035909160200190919290803590601001909190505061007c565b6040518082815260100191505060405180910390f35b6000600060007f66fa32225b641331dff20698cd66d310b3149e86d875926af7ea2f2a9079e80b856040518082815260100191505060405180910390a18585915091506001841115156100d55783925061016456610163565b60018282632d7df21a898960018a036000604051601001526040518463ffffffff166c010000000000000000000000000281526004018084848252816010015260200182815260100193505050506010604051808303816000888881813b151561013f5760006000fd5b5af1151561014d5760006000fd5b5050505060405180519060100150019250610164565b5b505093925050505600a165627a7a72305820c4755a8b960e01280a2c8d85fae255d08e1be318b2c2685a948e7b42660c2f5c0029".from_hex().unwrap();

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "2d7df21a".from_hex().unwrap();
        info!(target: "vm", "contract address = {:?}", address);
        call_data.append(&mut <[u8; 32]>::from(address.clone()).to_vec());
        call_data.append(&mut <[u8; 16]>::from(U128::from(129)).to_vec());
        params.data = Some(call_data);
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };
        assert_eq!(status_code, ExecStatus::Revert);
    }

    #[test]
    fn transfer() {
        let code = "605060405260636000600050909055341561001a5760006000fd5b61001f565b60c88061002d6000396000f30060506040526000356c01000000000000000000000000900463ffffffff168063c1cfb99a14603b578063f43fa805146057576035565b60006000fd5b6041607e565b6040518082815260100191505060405180910390f35b341560625760006000fd5b6068608b565b6040518082815260100191505060405180910390f35b6000303190506088565b90565b600060006000505490506099565b905600a165627a7a723058209d7cceee22377b5b19f0cbdfb6548ba4a3c94538bd66831bf7d16349565dfdf10029".from_hex().unwrap();

        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(100_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(0.into());
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let info = EnvInfo::default();
        let machine = make_aion_machine();
        let mut substate = Substate::new();

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create(params, &mut substate)
        };

        let ReturnData {
            mem,
            offset: _,
            size: _,
        } = return_data;

        //assert_eq!(mem, "6080604052600436106049576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063b802926914604e578063f43fa80514606a575b600080fd5b60546092565b6040518082815260200191505060405180910390f35b348015607557600080fd5b50607c60b1565b6040518082815260200191505060405180910390f35b60003073ffffffffffffffffffffffffffffffffffffffff1631905090565b600080549050905600a165627a7a72305820b64352477fa36031aab85a988e2c96456bb81f07e01036304f21fc60137cc4610029".from_hex().unwrap());
        assert_eq!(status_code, ExecStatus::Success);

        let code = mem.clone();

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(10));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let call_data = "f43fa805".from_hex().unwrap();
        params.data = Some(call_data);
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };

        println!("return data = {:?}", return_data);
        assert_eq!(status_code, ExecStatus::Revert);

        let code = mem.clone();

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(89));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let call_data = "c1cfb99a".from_hex().unwrap();
        //call_data.append(&mut vec![189]);
        params.data = Some(call_data);
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);

        let ExecutionResult {
            gas_left: _,
            status_code: _,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };
    }

    #[test]
    fn wallet() {
        let code = "60506040523415620000115760006000fd5b60405162001e6338038062001e63833981016040528080518201919060100180519060100190919080519060100190919050505b805b83835b6000600183510160016000508190909055503360026000506001610100811015156200007257fe5b90906002020160005b508282909180600101839055555050506001610202600050600033825281601001526020019081526010016000209050600050819090905550600090505b82518110156200016a578281815181101515620000d257fe5b9060100190602002018060100151905160026000508360020161010081101515620000f957fe5b90906002020160005b5082829091806001018390555550505080600201610202600050600085848151811015156200012d57fe5b906010019060200201806010015190518252816010015260200190815260100160002090506000508190909055505b8060010190508050620000b9565b8160006000508190909055505b50505080610205600050819090905550620001a5620001bd6401000000000262001aa8176401000000009004565b6102076000508190909055505b505b505050620001da565b60006201518042811515620001ce57fe5b049050620001d7565b90565b611c7980620001ea6000396000f300605060405236156100ee576000356c01000000000000000000000000900463ffffffff168063173825d9146101425780632f54bf6e1461016e57806335397dc0146101b25780634123cb6b146101f1578063523750931461021b5780635c52c2f514610245578063659010e71461025b5780637065cb4814610285578063746c9171146102b1578063797af627146102db578063a915f20614610325578063b75c7dc614610395578063c2cf7326146103c7578063cbf0b0c014610422578063f00d4b5d1461044e578063f165b2fe1461048b578063f1736d86146104af578063f1f06fb6146104d9576100ee565b5b600034111561013f577fc6dcd8d437d8b3537583463d84a6ba9d7e3e013fa4e004da9b6dee1482038be5333460405180848482528160100152602001828152601001935050505060405180910390a15b5b005b341561014e5760006000fd5b61016c600480808060100135903590916020019091929050506104fd565b005b341561017a5760006000fd5b61019860048080806010013590359091602001909192905050610644565b604051808215151515815260100191505060405180910390f35b34156101be5760006000fd5b6101d46004808035906010019091905050610679565b604051808383825281601001526020019250505060405180910390f35b34156101fd5760006000fd5b6102056106b2565b6040518082815260100191505060405180910390f35b34156102275760006000fd5b61022f6106bb565b6040518082815260100191505060405180910390f35b34156102515760006000fd5b6102596106c5565b005b34156102675760006000fd5b61026f610713565b6040518082815260100191505060405180910390f35b34156102915760006000fd5b6102af6004808080601001359035909160200190919290505061071d565b005b34156102bd5760006000fd5b6102c561086f565b6040518082815260100191505060405180910390f35b34156102e75760006000fd5b61030b60048080806010013590359060001916909091602001909192905050610878565b604051808215151515815260100191505060405180910390f35b34156103315760006000fd5b61036c600480808060100135903590916020019091929080359060100190919080359060100190820180359060100191909192905050610c21565b604051808383906000191690906000191690825281601001526020019250505060405180910390f35b34156103a15760006000fd5b6103c560048080806010013590359060001916909091602001909192905050610f07565b005b34156103d35760006000fd5b610408600480808060100135903590600019169090916020019091929080806010013590359091602001909192905050611018565b604051808215151515815260100191505060405180910390f35b341561042e5760006000fd5b61044c600480808060100135903590916020019091929050506110ad565b005b341561045a5760006000fd5b6104896004808080601001359035909160200190919290808060100135903590916020019091929050506110f2565b005b34156104975760006000fd5b6104ad6004808035906010019091905050611264565b005b34156104bb5760006000fd5b6104c36112b2565b6040518082815260100191505060405180910390f35b34156104e55760006000fd5b6104fb60048080359060100190919050506112bc565b005b600060003660405180838380828437820191505092505050604051809103902061052d828261136063ffffffff16565b15156105395760006000fd5b61020260005060008686825281601001526020019081526010016000209050600050549250600083141561056c5761063c565b60016001600050540360006000505411156105865761063c565b600060006002600050856101008110151561059d57fe5b90906002020160005b508282909180600101839055555050506000610202600050600087878252816010015260200190815260100160002090506000508190909055506105ee6115f363ffffffff16565b6105fc6116ba63ffffffff16565b7f58619076adf5bb0943d100ef88d52d7c3fd691b19d3a9071b555b651fbf418da8686604051808383825281601001526020019250505060405180910390a15b5b5050505050565b600060006102026000506000858582528160100152602001908152601001600020905060005054119050610673565b92915050565b600060006002600050600184016101008110151561069357fe5b90906002020160005b5080600101549054915091506106ad565b915091565b60016000505481565b6102076000505481565b6000366040518083838082843782019150509250505060405180910390206106f3828261136063ffffffff16565b15156106ff5760006000fd5b60006102066000508190909055505b5b5050565b6102066000505481565b60003660405180838380828437820191505092505050604051809103902061074b828261136063ffffffff16565b15156107575760006000fd5b610767848461064463ffffffff16565b1561077157610868565b61077f6115f363ffffffff16565b60fa60016000505410151561079d5761079c6116ba63ffffffff16565b5b60fa6001600050541015156107b157610868565b6001600081815054809291906001019190509090555083836002600050600160005054610100811015156107e157fe5b90906002020160005b50828290918060010183905555505050600160005054610202600050600086868252816010015260200190815260100160002090506000508190909055507f994a936646fe87ffe4f1e469d3d6aa417d6b855598397f323de5b449f765f0c38585604051808383825281601001526020019250505060405180910390a15b5b50505050565b60006000505481565b6000828261088c828261136063ffffffff16565b15156108985760006000fd5b600060006102086000506000888890600019169090600019169082528160100152602001908152601001600020905060005060000160005080600101549054909114919014161515610c1757610208600050600086869060001916909060001916908252816010015260200190815260100160002090506000506000016000508060010154905461020860005060008888906000191690906000191690825281601001526020019081526010016000209050600050600201600050546102086000506000898990600019169090600019169082528160100152602001908152601001600020905060005060030160005060405180828054600181600116156101000203166002900480156109ef5780600f106109c2576101008083540402835291601001916109ef565b8201919060005260106000209050905b8154815290600101906010018083116109d257829003600f168201915b50509150506000604051808303818588885af193505050501515610a135760006000fd5b7f9e0c482edabde7c5a28339467e14a00819cb5c09ef7efa27fa2b2a29ee75319a33888861020860005060008c8c9060001916909060001916908252816010015260200190815260100160002090506000506002016000505461020860005060008d8d9060001916909060001916908252816010015260200190815260100160002090506000506000016000508060010154905461020860005060008f8f9060001916909060001916908252816010015260200190815260100160002090506000506003016000506040518089898252816010015260200187879060001916909060001916908252816010015260200185815260100184848252816010015260200180601001828103825283818154600181600116156101000203166002900481526010019150805460018160011615610100020316600290048015610b9c5780600f10610b6f57610100808354040283529160100191610b9c565b8201919060005260106000209050905b815481529060010190601001808311610b7f57829003600f168201915b5050995050505050505050505060405180910390a1610208600050600086869060001916909060001916908252816010015260200190815260100160002090506000600082016000508060009055600101600090556002820160005060009055600382016000610c0c9190611ac3565b505060019250610c18565b5b5b505092915050565b60006000610c343361064463ffffffff16565b1515610c405760006000fd5b610c4f856118e863ffffffff16565b15610d10577f5b61ec5dfbe4cce36b7d3de1b7a363c3ad8e4e3ccaadd7b7969fc9aa6e4965c033888b8b8a8a6040518088888252816010015260200186815260100185858252816010015260200180601001828103825284848281815260100192508082843782019150509850505050505050505060405180910390a18686868686604051808383808284378201915050925050506000604051808303818588885af193505050501515610d035760006000fd5b6000600091509150610efc565b60003643604051808484808284378201915050828152601001935050505060405180910390209150915081815050610d4e828261087863ffffffff16565b158015610d9d5750600060006102086000506000858590600019169090600019169082528160100152602001908152601001600020905060005060000160005080600101549054909114919014165b15610efb5786866102086000506000858590600019169090600019169082528160100152602001908152601001600020905060005060000160005082829091806001018390555550505084610208600050600084849060001916909060001916908252816010015260200190815260100160002090506000506002016000508190909055508383610208600050600085859060001916909060001916908252816010015260200190815260100160002090506000506003016000509190610e65929190611b0d565b507fc522d6b7d06299b13c7597702937d15519e7a366b35b87fe5456326e37c8c3978383338a8d8d8c8c604051808a8a9060001916909060001916908252816010015260200188888252816010015260200186815260100185858252816010015260200180601001828103825284848281815260100192508082843782019150509a505050505050505050505060405180910390a15b5b5b9550959350505050565b6000600060006102026000506000338252816010015260200190815260100160002090506000505492506000831415610f3f57611011565b8260020a9150610203600050600086869060001916909060001916908252816010015260200190815260100160002090506000509050600082826001016000505416111561101057806000016000818150548092919060010191905090905550818160010160008282825054039250508190909055507fc7fb647e59b18047309aa15aad418e5d7ca96d173ad704f1031a2c3d7591734b3388886040518085858252816010015260200183839060001916909060001916908252816010015260200194505050505060405180910390a15b5b5050505050565b600060006000600061020360005060008989906000191690906000191690825281601001526020019081526010016000209050600050925061020260005060008787825281601001526020019081526010016000209050600050549150600082141561108757600093506110a2565b8160020a9050600081846001016000505416141593506110a2565b505050949350505050565b6000366040518083838082843782019150509250505060405180910390206110db828261136063ffffffff16565b15156110e75760006000fd5b8383ff5b5b50505050565b6000600036604051808383808284378201915050925050506040518091039020611122828261136063ffffffff16565b151561112e5760006000fd5b61113e858561064463ffffffff16565b156111485761125a565b61020260005060008888825281601001526020019081526010016000209050600050549250600083141561117b5761125a565b6111896115f363ffffffff16565b84846002600050856101008110151561119e57fe5b90906002020160005b5082829091806001018390555550505060006102026000506000898982528160100152602001908152601001600020905060005081909090555082610202600050600087878252816010015260200190815260100160002090506000508190909055507fb532073b38c83145e3e5135377a08bf9aab55bc0fd7c1179cd4fb995d2a5159c888888886040518085858252816010015260200183838252816010015260200194505050505060405180910390a15b5b50505050505050565b600036604051808383808284378201915050925050506040518091039020611292828261136063ffffffff16565b151561129e5760006000fd5b826102056000508190909055505b5b505050565b6102056000505481565b6000366040518083838082843782019150509250505060405180910390206112ea828261136063ffffffff16565b15156112f65760006000fd5b6001600050548311156113085761135a565b8260006000508190909055506113226115f363ffffffff16565b7fd9a37dd2a911cc717a3127f430cde03b03bcf2694f289138cff455a5430ea9fd846040518082815260100191505060405180910390a15b5b505050565b6000600060006000610202600050600033825281601001526020019081526010016000209050600050549250600083141561139a576115ea565b6102036000506000878790600019169090600019169082528160100152602001908152601001600020905060005091506000826000016000505414156114695760006000505482600001600050819090905550600082600101600050819090905550610204600050805480919060010190906114169190611b94565b826002016000508190909055508585610204600050846002016000505481548110151561143f57fe5b9060005260106000209050906002020160005b508282906000191690909180600101839055555050505b8260020a905060008183600101600050541614156115e9577fe1c52dc63b719ade82e8bea94cc41a0d5d28e4aaf536adb5e9cccc9ff8c1aeda3389896040518085858252816010015260200183839060001916909060001916908252816010015260200194505050505060405180910390a1600182600001600050541115156115b057610204600050610203600050600088889060001916909060001916908252816010015260200190815260100160002090506000506002016000505481548110151561153357fe5b9060005260106000209050906002020160005b508060009055600101600090556102036000506000878790600019169090600019169082528160100152602001908152601001600020905060006000820160005060009055600182016000506000905560028201600050600090555050600193506115ea566115e8565b8160000160008181505480929190600190039190509090555080826001016000828282505417925050819090905550600093506115ea565b5b5b50505092915050565b60006000610204600050805490509150600090505b818110156116a75761020860005060006102046000508381548110151561162b57fe5b9060005260106000209050906002020160005b508060010154905490600019169090600019169082528160100152602001908152601001600020905060006000820160005080600090556001016000905560028201600050600090556003820160006116979190611ac3565b50505b8060010190508050611608565b6116b56119a763ffffffff16565b5b5050565b6000600190505b6001600050548110156118e4575b6001600050548110801561170f575060006000600260005083610100811015156116f557fe5b90906002020160005b508060010154905490911491901416155b156117215780806001019150506116cf565b5b600160016000505411801561176757506000600060026000506001600050546101008110151561174e57fe5b90906002020160005b5080600101549054909114919014165b15611788576001600081815054809291906001900391905090905550611722565b600160005054811080156117cd5750600060006002600050600160005054610100811015156117b357fe5b90906002020160005b508060010154905490911491901416155b8015611804575060006000600260005083610100811015156117eb57fe5b90906002020160005b5080600101549054909114919014165b156118df5760026000506001600050546101008110151561182157fe5b90906002020160005b50806001015490546002600050836101008110151561184557fe5b90906002020160005b508282909180600101839055555050508061020260005060006002600050846101008110151561187a57fe5b90906002020160005b5080600101549054825281601001526020019081526010016000209050600050819090905550600060006002600050600160005054610100811015156118c557fe5b90906002020160005b508282909180600101839055555050505b6116c1565b5b50565b60006118f93361064463ffffffff16565b15156119055760006000fd5b6102076000505461191a611aa863ffffffff16565b111561194957600061020660005081909090555061193c611aa863ffffffff16565b6102076000508190909055505b610206600050548261020660005054011015801561197557506102056000505482610206600050540111155b1561199857816102066000828282505401925050819090905550600190506119a1565b600090506119a1565b5b919050565b60006000610204600050805490509150600090505b81811015611a945760006000610204600050838154811015156119db57fe5b9060005260106000209050906002020160005b5080600101549054906000191690909114919014161515611a8657610203600050600061020460005083815481101515611a2457fe5b9060005260106000209050906002020160005b5080600101549054906000191690906000191690825281601001526020019081526010016000209050600060008201600050600090556001820160005060009055600282016000506000905550505b5b80600101905080506119bc565b6102046000611aa39190611bc8565b5b5050565b60006201518042811515611ab857fe5b049050611ac0565b90565b50805460018160011615610100020316600290046000825580600f10611ae95750611b0a565b600f0160109004906000526010600020905090810190611b099190611bef565b5b50565b8280546001816001161561010002031660029004906000526010600020905090600f016010900481019282600f10611b5057803560ff1916838001178555611b83565b82800160010185558215611b83579182015b82811115611b825782358260005090905591601001919060010190611b62565b5b509050611b909190611bef565b5090565b815481835581811511611bc3576002028160020283600052601060002090509182019101611bc29190611c1a565b5b505050565b50805460008255600202906000526010600020905090810190611beb9190611c1a565b5b50565b611c179190611bf9565b80821115611c135760008181506000905550600101611bf9565b5090565b90565b611c4a9190611c24565b80821115611c4657600081815080600090556001016000905550600201611c24565b5090565b905600a165627a7a7230582008ce2c2eb5bf9c20836844338da08c6dceae1b44c52145e973d02634c22ce9bb0029".from_hex().unwrap();

        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(0.into());
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let info = EnvInfo::default();
        let machine = make_aion_machine();
        let mut substate = Substate::new();

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create(params, &mut substate)
        };

        let ReturnData {
            mem,
            offset: _,
            size: _,
        } = return_data;

        //assert_eq!(mem, "6080604052600436106049576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063b802926914604e578063f43fa80514606a575b600080fd5b60546092565b6040518082815260200191505060405180910390f35b348015607557600080fd5b50607c60b1565b6040518082815260200191505060405180910390f35b60003073ffffffffffffffffffffffffffffffffffffffff1631905090565b600080549050905600a165627a7a72305820b64352477fa36031aab85a988e2c96456bb81f07e01036304f21fc60137cc4610029".from_hex().unwrap());
        assert_eq!(status_code, ExecStatus::Success);

        let code = mem.clone();

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "797af627".from_hex().unwrap();
        call_data.append(
            &mut "7f4a14a00225c566fc11477d5febca8dec6ba6cda77d6d3f3e5a05bab9cf54b1"
                .from_hex()
                .unwrap(),
        );
        params.data = Some(call_data);
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };

        println!("return data = {:?}", return_data);
        assert_eq!(status_code, ExecStatus::Success);
    }

    #[test]
    // Internal transactions
    fn test_internal_transactions() {
        // internal_transactions.sol
        let code = "60506040525b5b61000b565b6104f88061001a6000396000f30060506040523615610054576000356c01000000000000000000000000900463ffffffff1680631e4198e01461008f5780636d73ac71146100af578063cc8066c8146100f8578063efc81a8c1461012d57610054565b5b7f656718b7d7f0803b58a7a46a3a5ca0a26696492f223276e7a227baba40fb95b7346040518082815260100191505060405180910390a15b005b6100ad6004808080601001359035909160200190919290505061015e565b005b34156100bb5760006000fd5b6100e2600480808060100135903590916020019091929080359060100190919050506101c8565b6040518082815260100191505060405180910390f35b34156101045760006000fd5b61012b60048080806010013590359091602001909192908035906010019091905050610295565b005b34156101395760006000fd5b610141610300565b604051808383825281601001526020019250505060405180910390f35b81816108fc34908115029060405160006040518083038185898989f1945050505050151561018c5760006000fd5b7f281a259dfd2e4aaf4447339f7e35909b8a423be045a738057d6c3c01e8d1f5a2346040518082815260100191505060405180910390a15b5050565b60006000600060008686925092506002838363f65a554b886000604051601001526040518263ffffffff166c01000000000000000000000000028152600401808281526010019150506010604051808303816000888881813b151561022d5760006000fd5b5af1151561023b5760006000fd5b50505050604051805190601001500190507ff56ebbc311e11d9790970c4f650e868d7c17a8c35e5c268477d6933ae010d1f4826040518082815260100191505060405180910390a180935061028b565b5050509392505050565b82826108fc83908115029060405160006040518083038185898989f194505050505015156102c35760006000fd5b7f3f418b40de968f04f1770399699cdfb8221fb37431187bf6ba88c8ef1cde63de826040518082815260100191505060405180910390a15b505050565b600060006000600061031061037e565b604051809103906000f080158215161561032a5760006000fd5b915091507f6092db4a9f98e713a99420bc27f1f1cdfcfd435af45397aeb05ffbee8d567d8e8383604051808383825281601001526020019250505060405180910390a1818193509350610378565b50509091565b60405161013e8061038f833901905600605060405234156100105760006000fd5b610015565b61011a806100246000396000f300605060405236156030576000356c01000000000000000000000000900463ffffffff168063f65a554b14606b576030565b3415603b5760006000fd5b5b7f6684c6fb8e464ba954e17ed3f5aed6e2d49231ce285b7770f422d1f52cef503b60405160405180910390a15b005b341560765760006000fd5b608a600480803590601001909190505060a0565b6040518082815260100191505060405180910390f35b600060006001830190507f3bc83dc4da931c34301105d9c2aff52e35bb96133cd1cf0a835faa9bb607422c826040518082815260100191505060405180910390a180915060e8565b509190505600a165627a7a72305820ec84292d19105cb4d6f311689eed4db3cb4e4251249a9cf06f071e1746e74de60029a165627a7a723058209d38411c6f215aa8daa7dd8150890ea7576a1b0e117cedffc3c002a95810948e0029".from_hex().unwrap();
        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);
        let machine = make_aion_machine();
        let mut substate = Substate::new();
        let address = contract_address(&sender, &U256::zero()).0;
        // Create contract InternalTransaction
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(10.into());
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create(params, &mut substate)
        };
        let ReturnData {
            mem,
            offset: _,
            size: _,
        } = return_data;
        assert_eq!(status_code, ExecStatus::Success);
        assert_eq!(mem, "60506040523615610054576000356c01000000000000000000000000900463ffffffff1680631e4198e01461008f5780636d73ac71146100af578063cc8066c8146100f8578063efc81a8c1461012d57610054565b5b7f656718b7d7f0803b58a7a46a3a5ca0a26696492f223276e7a227baba40fb95b7346040518082815260100191505060405180910390a15b005b6100ad6004808080601001359035909160200190919290505061015e565b005b34156100bb5760006000fd5b6100e2600480808060100135903590916020019091929080359060100190919050506101c8565b6040518082815260100191505060405180910390f35b34156101045760006000fd5b61012b60048080806010013590359091602001909192908035906010019091905050610295565b005b34156101395760006000fd5b610141610300565b604051808383825281601001526020019250505060405180910390f35b81816108fc34908115029060405160006040518083038185898989f1945050505050151561018c5760006000fd5b7f281a259dfd2e4aaf4447339f7e35909b8a423be045a738057d6c3c01e8d1f5a2346040518082815260100191505060405180910390a15b5050565b60006000600060008686925092506002838363f65a554b886000604051601001526040518263ffffffff166c01000000000000000000000000028152600401808281526010019150506010604051808303816000888881813b151561022d5760006000fd5b5af1151561023b5760006000fd5b50505050604051805190601001500190507ff56ebbc311e11d9790970c4f650e868d7c17a8c35e5c268477d6933ae010d1f4826040518082815260100191505060405180910390a180935061028b565b5050509392505050565b82826108fc83908115029060405160006040518083038185898989f194505050505015156102c35760006000fd5b7f3f418b40de968f04f1770399699cdfb8221fb37431187bf6ba88c8ef1cde63de826040518082815260100191505060405180910390a15b505050565b600060006000600061031061037e565b604051809103906000f080158215161561032a5760006000fd5b915091507f6092db4a9f98e713a99420bc27f1f1cdfcfd435af45397aeb05ffbee8d567d8e8383604051808383825281601001526020019250505060405180910390a1818193509350610378565b50509091565b60405161013e8061038f833901905600605060405234156100105760006000fd5b610015565b61011a806100246000396000f300605060405236156030576000356c01000000000000000000000000900463ffffffff168063f65a554b14606b576030565b3415603b5760006000fd5b5b7f6684c6fb8e464ba954e17ed3f5aed6e2d49231ce285b7770f422d1f52cef503b60405160405180910390a15b005b341560765760006000fd5b608a600480803590601001909190505060a0565b6040518082815260100191505060405180910390f35b600060006001830190507f3bc83dc4da931c34301105d9c2aff52e35bb96133cd1cf0a835faa9bb607422c826040518082815260100191505060405180910390a180915060e8565b509190505600a165627a7a72305820ec84292d19105cb4d6f311689eed4db3cb4e4251249a9cf06f071e1746e74de60029a165627a7a723058209d38411c6f215aa8daa7dd8150890ea7576a1b0e117cedffc3c002a95810948e0029".from_hex().unwrap());
        assert_eq!(state.balance(&sender).unwrap(), U256::from(90));
        assert_eq!(state.balance(&address).unwrap(), U256::from(10));
        assert_eq!(state.nonce(&address).unwrap(), U256::from(0));

        // Transfer value through contract
        let code = mem.clone();
        let receiver = Address::from_slice(b"ef1722f3947def4cf144679da39c4c32bdc35681");
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(10));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "1e4198e0".from_hex().unwrap();
        call_data.append(&mut <[u8; 32]>::from(receiver.clone()).to_vec());
        params.data = Some(call_data);
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };
        assert_eq!(status_code, ExecStatus::Success);
        assert_eq!(state.balance(&sender).unwrap(), U256::from(80));
        assert_eq!(state.balance(&address).unwrap(), U256::from(10));
        assert_eq!(state.balance(&receiver).unwrap(), U256::from(10));
        assert_eq!(state.nonce(&address).unwrap(), U256::from(0));

        // Ask contract contract to transfer value
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "cc8066c8".from_hex().unwrap();
        call_data.append(&mut <[u8; 32]>::from(receiver.clone()).to_vec());
        call_data.append(&mut <[u8; 16]>::from(U128::from(5)).to_vec());
        params.data = Some(call_data);
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };
        assert_eq!(status_code, ExecStatus::Success);
        assert_eq!(state.balance(&sender).unwrap(), U256::from(80));
        assert_eq!(state.balance(&address).unwrap(), U256::from(5));
        assert_eq!(state.balance(&receiver).unwrap(), U256::from(15));
        assert_eq!(state.nonce(&address).unwrap(), U256::from(0));

        // Create contract NewContract from contract InternalTransaction
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(210_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let call_data = "efc81a8c".from_hex().unwrap();
        params.data = Some(call_data);
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };
        assert_eq!(status_code, ExecStatus::Success);
        let ReturnData {
            mem,
            offset: _,
            size: _,
        } = return_data;
        let new_address = Address::from(mem.clone().as_slice());
        assert_eq!(state.nonce(&address).unwrap(), U256::from(1));
        assert_eq!(state.nonce(&new_address).unwrap(), U256::from(1));

        // Call NewContract's function from InternalTransaction
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "6d73ac71".from_hex().unwrap();
        call_data.append(&mut <[u8; 32]>::from(new_address).to_vec());
        call_data.append(&mut <[u8; 16]>::from(U128::from(1)).to_vec());
        params.data = Some(call_data);
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };
        assert_eq!(status_code, ExecStatus::Success);
        let ReturnData {
            mem,
            offset: _,
            size: _,
        } = return_data;
        assert_eq!(mem, "00000000000000000000000000000004".from_hex().unwrap());
        assert_eq!(state.nonce(&address).unwrap(), U256::from(1));
        assert_eq!(state.nonce(&new_address).unwrap(), U256::from(1));
    }

    #[test]
    fn error_cases_rejected() {
        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let machine = make_aion_machine();
        let mut info = EnvInfo::default();
        info.gas_limit = U256::from(3_000_000);
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();

        // 1. Invalid gas limit
        // 1.1 Transaction gas limit exceeds block gas limit
        let transaction: Transaction = Transaction::new(
            U256::zero(),
            U256::zero(),
            U256::from(4_000_000),
            Action::Create,
            0.into(),
            Bytes::new(),
        );
        let signed_transaction: SignedTransaction = transaction.fake_sign(sender);
        let error = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.transact(&signed_transaction, true, false).unwrap_err()
        };
        assert_eq!(
            error,
            ExecutionError::BlockGasLimitReached {
                gas_limit: U256::from(3_000_000),
                gas_used: U256::from(0),
                gas: U256::from(4_000_000),
            }
        );

        // 1.2 Transaction gas limit exceeds max gas limit (create)
        info.gas_limit = U256::from(10_000_000);
        let transaction: Transaction = Transaction::new(
            U256::zero(),
            U256::zero(),
            U256::from(6_000_000),
            Action::Create,
            0.into(),
            Bytes::new(),
        );
        let signed_transaction: SignedTransaction = transaction.fake_sign(sender);
        let error = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.transact(&signed_transaction, true, false).unwrap_err()
        };
        assert_eq!(
            error,
            ExecutionError::ExceedMaxGasLimit {
                max: U256::from(5_000_000),
                got: U256::from(6_000_000),
            }
        );

        // 1.3 Transaction gas limit exceeds max gas limit (call)
        let transaction: Transaction = Transaction::new(
            U256::zero(),
            U256::zero(),
            U256::from(3_000_000),
            Action::Call(0.into()),
            0.into(),
            Bytes::new(),
        );
        let signed_transaction: SignedTransaction = transaction.fake_sign(sender);
        let error = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.transact(&signed_transaction, true, false).unwrap_err()
        };
        assert_eq!(
            error,
            ExecutionError::ExceedMaxGasLimit {
                max: U256::from(2_000_000),
                got: U256::from(3_000_000),
            }
        );

        // 1.4 Transaction does not have enough base gas (create)
        let data = "605060405234156100105760006000fd5b610015565b610199806100246000396000f30060506040526000356c01000000000000000000000000900463ffffffff1680632d7df21a146100335761002d565b60006000fd5b341561003f5760006000fd5b6100666004808080601001359035909160200190919290803590601001909190505061007c565b6040518082815260100191505060405180910390f35b6000600060007f66fa32225b641331dff20698cd66d310b3149e86d875926af7ea2f2a9079e80b856040518082815260100191505060405180910390a18585915091506001841115156100d55783925061016456610163565b60018282632d7df21a898960018a036000604051601001526040518463ffffffff166c010000000000000000000000000281526004018084848252816010015260200182815260100193505050506010604051808303816000888881813b151561013f5760006000fd5b5af1151561014d5760006000fd5b5050505060405180519060100150019250610164565b5b505093925050505600a165627a7a72305820c4755a8b960e01280a2c8d85fae255d08e1be318b2c2685a948e7b42660c2f5c0029".from_hex().unwrap();
        let transaction: Transaction = Transaction::new(
            U256::zero(),
            U256::zero(),
            U256::from(1_000),
            Action::Create,
            0.into(),
            data,
        );
        let signed_transaction: SignedTransaction = transaction.fake_sign(sender);
        let error = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.transact(&signed_transaction, true, false).unwrap_err()
        };
        assert_eq!(
            error,
            ExecutionError::NotEnoughBaseGas {
                required: U256::from(246_240),
                got: U256::from(1_000),
            }
        );

        // 1.5 Transaction does not have enough base gas (call)
        let data = "2d7df21a".from_hex().unwrap();
        let transaction: Transaction = Transaction::new(
            U256::zero(),
            U256::zero(),
            U256::from(1_000),
            Action::Call(0.into()),
            0.into(),
            data,
        );
        let signed_transaction: SignedTransaction = transaction.fake_sign(sender);
        let error = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.transact(&signed_transaction, true, false).unwrap_err()
        };
        assert_eq!(
            error,
            ExecutionError::NotEnoughBaseGas {
                required: U256::from(21_256),
                got: U256::from(1_000),
            }
        );

        // 2. Insufficient balance
        let transaction: Transaction = Transaction::new(
            U256::zero(),
            U256::from(1),
            U256::from(50_000),
            Action::Call(0.into()),
            1000.into(),
            Bytes::new(),
        );
        let signed_transaction: SignedTransaction = transaction.fake_sign(sender);
        let error = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.transact(&signed_transaction, true, false).unwrap_err()
        };
        assert_eq!(
            error,
            ExecutionError::NotEnoughCash {
                required: U512::from(51_000),
                got: U512::from(100),
            }
        );

        // 3. Invalid nonce
        let transaction: Transaction = Transaction::new(
            U256::from(1),
            U256::from(0),
            U256::from(50_000),
            Action::Call(0.into()),
            0.into(),
            Bytes::new(),
        );
        let signed_transaction: SignedTransaction = transaction.fake_sign(sender);
        let error = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.transact(&signed_transaction, true, false).unwrap_err()
        };
        assert_eq!(
            error,
            ExecutionError::InvalidNonce {
                expected: U256::zero(),
                got: U256::from(1),
            }
        );
    }

    #[test]
    fn error_cases_revert() {
        let code = "605060405234156100105760006000fd5b610015565b610199806100246000396000f30060506040526000356c01000000000000000000000000900463ffffffff1680632d7df21a146100335761002d565b60006000fd5b341561003f5760006000fd5b6100666004808080601001359035909160200190919290803590601001909190505061007c565b6040518082815260100191505060405180910390f35b6000600060007f66fa32225b641331dff20698cd66d310b3149e86d875926af7ea2f2a9079e80b856040518082815260100191505060405180910390a18585915091506001841115156100d55783925061016456610163565b60018282632d7df21a898960018a036000604051601001526040518463ffffffff166c010000000000000000000000000281526004018084848252816010015260200182815260100193505050506010604051808303816000888881813b151561013f5760006000fd5b5af1151561014d5760006000fd5b5050505060405180519060100150019250610164565b5b505093925050505600a165627a7a72305820c4755a8b960e01280a2c8d85fae255d08e1be318b2c2685a948e7b42660c2f5c0029".from_hex().unwrap();

        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;

        // 1. Create non payable contract with value transfer
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(1.into());
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let info = EnvInfo::default();
        let machine = make_aion_machine();
        let mut substate = Substate::new();
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create(params, &mut substate)
        };
        assert_eq!(status_code, ExecStatus::Revert);

        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(0.into());
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create(params, &mut substate)
        };

        let ReturnData {
            mem,
            offset: _,
            size: _,
        } = return_data;
        //assert_eq!(mem, "6080604052600436106049576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063b802926914604e578063f43fa80514606a575b600080fd5b60546092565b6040518082815260200191505060405180910390f35b348015607557600080fd5b50607c60b1565b6040518082815260200191505060405180910390f35b60003073ffffffffffffffffffffffffffffffffffffffff1631905090565b600080549050905600a165627a7a72305820b64352477fa36031aab85a988e2c96456bb81f07e01036304f21fc60137cc4610029".from_hex().unwrap());
        assert_eq!(status_code, ExecStatus::Success);
        let code = mem.clone();

        // Call non payable function with value transfer
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(1));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        println!("revert test starts");
        let mut call_data = "2d7df21a".from_hex().unwrap();
        println!("contract address = {:?}", address);
        call_data.append(&mut <[u8; 32]>::from(address.clone()).to_vec());
        call_data.append(&mut <[u8; 16]>::from(U128::from(2)).to_vec());
        params.data = Some(call_data);
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };
        println!("return data = {:?}", return_data);
        assert_eq!(status_code, ExecStatus::Revert);

        // Call contract reverts when called contract gets error or runs out of gas
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        println!("revert test starts");
        let mut call_data = "2d7df21a".from_hex().unwrap();
        println!("contract address = {:?}", address);
        call_data.append(&mut <[u8; 32]>::from(address.clone()).to_vec());
        call_data.append(&mut <[u8; 16]>::from(U128::from(129)).to_vec());
        params.data = Some(call_data);
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };
        println!("return data = {:?}", return_data);
        assert_eq!(status_code, ExecStatus::Revert);
    }

    #[test]
    fn error_cases_failure() {
        // Create contract on already existing address
        let code = "605060405234156100105760006000fd5b610015565b610199806100246000396000f30060506040526000356c01000000000000000000000000900463ffffffff1680632d7df21a146100335761002d565b60006000fd5b341561003f5760006000fd5b6100666004808080601001359035909160200190919290803590601001909190505061007c565b6040518082815260100191505060405180910390f35b6000600060007f66fa32225b641331dff20698cd66d310b3149e86d875926af7ea2f2a9079e80b856040518082815260100191505060405180910390a18585915091506001841115156100d55783925061016456610163565b60018282632d7df21a898960018a036000604051601001526040518463ffffffff166c010000000000000000000000000281526004018084848252816010015260200182815260100193505050506010604051808303816000888881813b151561013f5760006000fd5b5af1151561014d5760006000fd5b5050505060405180519060100150019250610164565b5b505093925050505600a165627a7a72305820c4755a8b960e01280a2c8d85fae255d08e1be318b2c2685a948e7b42660c2f5c0029".from_hex().unwrap();
        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let mut params = ActionParams::default();
        params.address = sender.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(100_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(0.into());
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let info = EnvInfo::default();
        let machine = make_aion_machine();
        let mut substate = Substate::new();
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create(params, &mut substate)
        };
        assert_eq!(status_code, ExecStatus::Failure);
    }

    #[test]
    fn error_cases_out_of_gas() {
        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let mut substate = Substate::new();
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);
        let machine = make_aion_machine();

        // 1. Deploy bad code
        let code = "ff".from_hex().unwrap();
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(0.into());
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create(params, &mut substate)
        };
        assert_eq!(status_code, ExecStatus::OutOfGas);

        // 2. Run out of gas
        let code = "605060405234156100105760006000fd5b610015565b610199806100246000396000f30060506040526000356c01000000000000000000000000900463ffffffff1680632d7df21a146100335761002d565b60006000fd5b341561003f5760006000fd5b6100666004808080601001359035909160200190919290803590601001909190505061007c565b6040518082815260100191505060405180910390f35b6000600060007f66fa32225b641331dff20698cd66d310b3149e86d875926af7ea2f2a9079e80b856040518082815260100191505060405180910390a18585915091506001841115156100d55783925061016456610163565b60018282632d7df21a898960018a036000604051601001526040518463ffffffff166c010000000000000000000000000281526004018084848252816010015260200182815260100193505050506010604051808303816000888881813b151561013f5760006000fd5b5af1151561014d5760006000fd5b5050505060405180519060100150019250610164565b5b505093925050505600a165627a7a72305820c4755a8b960e01280a2c8d85fae255d08e1be318b2c2685a948e7b42660c2f5c0029".from_hex().unwrap();
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(100_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(0.into());
        let machine = make_aion_machine();

        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create(params, &mut substate)
        };
        let ReturnData {
            mem,
            offset: _,
            size: _,
        } = return_data;
        assert_eq!(status_code, ExecStatus::Success);
        let code = mem.clone();
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.code_address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(100);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(U256::from(0));
        params.call_type = CallType::Call;
        params.gas_price = U256::from(0);
        let mut call_data = "2d7df21a".from_hex().unwrap();
        println!("contract address = {:?}", address);
        call_data.append(&mut <[u8; 32]>::from(address.clone()).to_vec());
        call_data.append(&mut <[u8; 16]>::from(U128::from(2)).to_vec());
        params.data = Some(call_data);
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params, &mut substate)
        };
        assert_eq!(status_code, ExecStatus::OutOfGas);
    }

    #[test]
    fn create_empty_contract() {
        // Create contract on already existing address
        let code: Vec<u8> = Vec::new();
        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(0.into());
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let info = EnvInfo::default();
        let machine = make_aion_machine();
        let mut substate = Substate::new();
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data,
            exception,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create(params, &mut substate)
        };
        println!("exception: {:?}", exception);
        assert_eq!(status_code, ExecStatus::Success);
        let ReturnData {
            mem,
            offset: _,
            size: _,
        } = return_data;
        let vec_empty: Vec<u8> = Vec::new();
        assert_eq!(mem, vec_empty);
        assert_eq!(state.commit().is_ok(), true);
        assert_eq!(
            state.root().to_vec(),
            "522f7176a4724330ee2423d5cf34c687eb32adc3fab752cbb4ab03a25dc6e0cc"
                .from_hex()
                .unwrap()
        );
    }

    #[test]
    fn static_call() {
        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;
        let mut state = get_temp_state();
        state
            .add_balance(&sender, &U256::from(100), CleanupMode::NoEmpty)
            .unwrap();
        let mut substate = Substate::new();
        let mut info = EnvInfo::default();
        info.number = 1;
        info.gas_limit = U256::from(1000000);
        info.author = Address::from(1);
        let machine = make_aion_machine();

        // Call ofter StaticCall with zero value
        let code = "6f000000000000000000000000000000006f000000000000000000000000000000006f000000000000000000000000000000006f0000000000000000000000000000000034305af1".from_hex().unwrap();
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(0.into());
        params.call_type = CallType::Call;
        params.static_flag = true;
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params.clone(), &mut substate)
        };
        assert_eq!(status_code, ExecStatus::Success);
        assert_eq!(state.balance(&params.address).unwrap(), U256::from(0));

        // Call after StaticCall with non-zero value
        let code = "6f000000000000000000000000000000006f000000000000000000000000000000006f000000000000000000000000000000006f0000000000000000000000000000000034305af1".from_hex().unwrap();
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(10.into());
        params.call_type = CallType::Call;
        params.static_flag = true;
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params.clone(), &mut substate)
        };
        assert_eq!(status_code, ExecStatus::OutOfGas);
        assert_eq!(state.balance(&params.address).unwrap(), U256::from(0));

        // CallCode after StaticCall with non-zero value
        let code = "6f000000000000000000000000000000006f000000000000000000000000000000006f000000000000000000000000000000006f0000000000000000000000000000000034305af2".from_hex().unwrap();
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(10.into());
        params.call_type = CallType::Call;
        params.static_flag = true;
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params.clone(), &mut substate)
        };
        assert_eq!(status_code, ExecStatus::Success);
        assert_eq!(state.balance(&params.address).unwrap(), U256::from(10));

        // DelegateCall after StaticCall with non-zero value
        let code = "6f000000000000000000000000000000006f000000000000000000000000000000006f000000000000000000000000000000006f0000000000000000000000000000000034305af4".from_hex().unwrap();
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(10.into());
        params.call_type = CallType::Call;
        params.static_flag = true;
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params.clone(), &mut substate)
        };
        assert_eq!(status_code, ExecStatus::Success);
        assert_eq!(state.balance(&params.address).unwrap(), U256::from(20));

        // DelegateCall after Call with non-zero value
        let code = "6f000000000000000000000000000000006f000000000000000000000000000000006f000000000000000000000000000000006f0000000000000000000000000000000034305af4".from_hex().unwrap();
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(10.into());
        params.call_type = CallType::Call;
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params.clone(), &mut substate)
        };
        assert_eq!(status_code, ExecStatus::Success);
        assert_eq!(state.balance(&params.address).unwrap(), U256::from(30));

        // StaticCall with sstore
        let code = "6f000000000000000000000000000000006f000000000000000000000000000000006f000000000000000000000000000000006f0000000000000000000000000000000060346f000000000000000000000000000000015534305af1".from_hex().unwrap();
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(10.into());
        params.call_type = CallType::Call;
        params.static_flag = true;
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params.clone(), &mut substate)
        };
        assert_eq!(status_code, ExecStatus::OutOfGas);
        assert_eq!(state.balance(&params.address).unwrap(), U256::from(30));

        // StaticCall with log
        let code = "6f000000000000000000000000000000006f000000000000000000000000000000006f000000000000000000000000000000006f00000000000000000000000000000000a034305af1".from_hex().unwrap();
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(10.into());
        params.call_type = CallType::Call;
        params.static_flag = true;
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params.clone(), &mut substate)
        };
        assert_eq!(status_code, ExecStatus::OutOfGas);
        assert_eq!(state.balance(&params.address).unwrap(), U256::from(30));

        // StaticCall with selfdestruct
        let code = "6f000000000000000000000000000000006f000000000000000000000000000000006f000000000000000000000000000000006f00000000000000000000000000000000ff34305af1".from_hex().unwrap();
        let mut params = ActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(10_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = ActionValue::Transfer(10.into());
        params.call_type = CallType::Call;
        params.static_flag = true;
        let ExecutionResult {
            gas_left: _,
            status_code,
            return_data: _,
            exception: _,
        } = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call(params.clone(), &mut substate)
        };
        assert_eq!(status_code, ExecStatus::OutOfGas);
        assert_eq!(state.balance(&params.address).unwrap(), U256::from(30));
    }

    #[test]
    fn hello_avm() {
        // Create contract on already existing address
        let code = "000006ce504b03040a0000080000fa7e114d000000000000000000000000090004004d4554412d494e462ffeca0000504b03040a0000080800f97e114dd56006437f00000096000000140000004d4554412d494e462f4d414e49464553542e4d464d8bb10ac320140077c17f904c2dc587365bb6c4a54be8d6ce0ff34a0246e529b4fdfb9aaddb71c7cd18b71795aa1fc4654b7150168c1463fc336346bf926aae456ba097c23161a5454fdf36183070bdd85e9dba3ba30fa45ce29c186bdbbbb314336e51bb80a50ccaa71de8837b0e042b8590de89c302b7039f074a21c50f504b03040a0000080000f97e114d00000000000000000000000004000000636f6d2f504b03040a0000080000f97e114d0000000000000000000000000c000000636f6d2f6578616d706c652f504b03040a0000080000f97e114d00000000000000000000000017000000636f6d2f6578616d706c652f68656c6c6f776f726c642f504b03040a0000080800f97e114d15fb44197c0200002d04000027000000636f6d2f6578616d706c652f68656c6c6f776f726c642f48656c6c6f576f726c642e636c61737385525d4f1341143d43cb6ebb6e291444a055f912db8a2ca2e207684225c6c6aa891808e98bd3edd82eec0759b7a8ffc9078c09104d8c4f3ef8a38c7786852a98d84defbd73e79e3be79e999fbfbe7c03b08807064c4ca770c5c00caeea281a4862da40096503d7302bcd751d7306d2b0d2b437afe30643e24d1030b02a450d1e3268cb8eef440f69592cad33241f054dc190ad39be78def11a227cc51b2e6572b5c0e6ee3a0f1db98e93c9a8edbc6598aad9816789f7dcdb7185d516ae1bbc0b42b7693d91e1860c97e800de6c32f416abd5129dce38fd1b940d3b3e352a96ea15721e7768955ab6dd98559a87ad8e27fc888e19acd7b6f82eb75ceeb7ac178d2d6147d436b316717bfb19df8939196b4127b4c563472eb25d0673126ba21f038451e9d971c57242c782899b28304cfc770e1db74cdc4641c7a2893bb86be21eee330c748955fd48b444686209cb26b2e863e83f4dfbafd45a143a7e8ba66e89a8f2211234e96410b62cee04bec5773d8bef3856c50dec6dbb4df2bcecf891e3d16c3ad5aff28874cc9f295fa95457854d5749373cdc54d18adf24e88613b58f398c14cfea59afa88bc8c698aef8178a72eb5f179072fc689dbb1da1de501513f4d64c7aa23df465d04791216520df0b26f5279ba395459ec96cf900ec93020c92d55452c31059f3a800e7314c9e4860240657a83a413e992fbcfe780a3ba4b0e347fb315646a34482a9688ca21e8af328c4fde663325aae671f89bd531d47fe60a3c51dd3b878829ea16af9ebfb8ae4e6017a0fa11d42df539cbb5d0a48e1d2c9f89b8a013046c53f30ac20a9a7e5c4c277a4f7617cc639a94942e173c41924a646bc3324e62875396694217b99a605092febd9282527d50053bf01504b03040a0000080800f97e114dbbf4728ca9000000e8000000110000006d6f64756c652d696e666f2e636c6173734d8d4b0e824010446bfc8082bf04e301dcdba00b8fe0ce9527186154cc0c4330a04bcfe5c2037828e380f8e94eba53957add8fe7ed0e6089a10d87c1d9e83c0bc52a968261a474944b318b939da6232f3883b5ae2c0fae879e91f380025a78e88f316070ff008649a815890b57a914741052eab3ce64c4d02d6fd1969fca1f3adb138f7542bc50c4d3f8cdf935e7ffb82bc0f0a986e97236d1327b8a76ed5a2665a35329f3e89bc70b504b010214030a0000080000fa7e114d000000000000000000000000090004000000000000001000ed41000000004d4554412d494e462ffeca0000504b010214030a0000080800f97e114dd56006437f00000096000000140000000000000000000000a4812b0000004d4554412d494e462f4d414e49464553542e4d46504b010214030a0000080000f97e114d000000000000000000000000040000000000000000001000ed41dc000000636f6d2f504b010214030a0000080000f97e114d0000000000000000000000000c0000000000000000001000ed41fe000000636f6d2f6578616d706c652f504b010214030a0000080000f97e114d000000000000000000000000170000000000000000001000ed4128010000636f6d2f6578616d706c652f68656c6c6f776f726c642f504b010214030a0000080800f97e114d15fb44197c0200002d040000270000000000000000000000a4815d010000636f6d2f6578616d706c652f68656c6c6f776f726c642f48656c6c6f576f726c642e636c617373504b010214030a0000080800f97e114dbbf4728ca9000000e8000000110000000000000000000000a4811e0400006d6f64756c652d696e666f2e636c617373504b05060000000007000700c2010000f60400000000".from_hex().unwrap();
        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;
        let mut params = AVMActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1_000_000);
        params.code = Some(Arc::new(code.clone()));
        params.value = 0.into();
        params.call_type = CallType::None;
        params.gas_price = 1.into();
        let mut state = get_temp_state();
        state
            .add_avm_balance(&sender, &U256::from(5_000_000), CleanupMode::NoEmpty)
            .unwrap();
        let info = EnvInfo::default();
        let machine = make_aion_machine();
        let substate = Substate::new();
        let execution_results = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create_avm(vec![params.clone()], &mut [substate])
        };

        for r in execution_results {
            let ExecutionResult {
                status_code,
                gas_left,
                return_data,
                exception,
            } = r;
            assert_eq!(status_code, ExecStatus::Success);

            println!(
                "(return_data = {:?}, status_code = {:?}, gas_left = {:?})",
                return_data, status_code, gas_left
            );
        }

        assert_eq!(state.commit().is_ok(), true);
        // assert_eq!(
        //     state.root().to_vec(),
        //     "596426bcd6ff5affa67adf43b1a280aed0f81b4c74c4b2c3a8eda4a5b4ac9a44"
        //         .from_hex()
        //         .unwrap()
        // );
    }

    use std::io::Error;
    use std::fs::File;
    use std::io::Read;
    use std::path::PathBuf;

    fn read_file(path: &str) -> Result<Vec<u8>, Error> {
        println!("path = {:?}", path);
        let mut file = File::open(path)?;
        let mut buf = Vec::<u8>::new();
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }

    #[test]
    fn avm_save_and_call_code() {
        // Create contract on already existing address
        let mut file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        file.push("src/tests/AVMDapps/com.example.helloworld.jar");
        let file_str = file.to_str().expect("Failed to locate the helloworld.jar");
        let mut code = read_file(file_str).expect("unable to open avm dapp");
        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero()).0;
        println!("sender = {:?}, receiver = {:?}", sender, address);
        let mut params = AVMActionParams::default();
        params.address = address.clone();
        params.sender = sender.clone();
        params.origin = sender.clone();
        params.gas = U256::from(1_000_000);
        let mut avm_code: Vec<u8> = (code.len() as u32).to_vm_bytes();
        avm_code.append(&mut code);
        params.code = Some(Arc::new(avm_code.clone()));
        params.value = 0.into();
        params.call_type = CallType::None;
        params.gas_price = 1.into();
        let mut state = get_temp_state();
        state
            .add_avm_balance(&sender, &U256::from(5_000_000), CleanupMode::NoEmpty)
            .unwrap();
        state
            .add_avm_balance(&address, &U256::from(1_000_000), CleanupMode::NoEmpty)
            .unwrap();
        let info = EnvInfo::default();
        let machine = make_aion_machine();
        let substate = Substate::new();
        let execution_results = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.create_avm(vec![params.clone()], &mut [substate])
        };

        println!("state after create = {:?}", state);

        for r in execution_results {
            let ExecutionResult {
                status_code,
                gas_left,
                return_data,
                exception,
            } = r;
            assert_eq!(status_code, ExecStatus::Success);

            println!(
                "(return_data = {:?}, status_code = {:?}, gas_left = {:?})",
                return_data, status_code, gas_left
            );
        }

        params.call_type = CallType::Call;
        //let mut call_data = 14_i32.to_vm_bytes();
        let mut call_data = vec![0x72,0x75,0x6E,0x3C,0x3E];
        //call_data.append(&mut AbiToken::STRING("run".to_string()).encode());
        params.data = Some(call_data);
        params.nonce += 1;
        println!("call data = {:?}", params.data);
        let substate = Substate::new();
        let execution_results = {
            let mut ex = Executive::new(&mut state, &info, &machine);
            ex.call_avm(vec![params.clone()], &mut [substate])
        };

        for r in execution_results {
            let ExecutionResult {
                status_code,
                gas_left,
                return_data,
                exception,
            } = r;
            assert_eq!(status_code, ExecStatus::Success);

            println!(
                "(return_data = {:?}, status_code = {:?}, gas_left = {:?})",
                return_data, status_code, gas_left
            );
        }

        assert_eq!(state.commit().is_ok(), true);
        assert_eq!(
            state.root().to_vec(),
            "596426bcd6ff5affa67adf43b1a280aed0f81b4c74c4b2c3a8eda4a5b4ac9a44"
                .from_hex()
                .unwrap()
        );
    }

    use state::AVMInterface;

    #[test]
    fn avm_storage() {
        let mut state = get_temp_state();
        let address = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        state.set_avm_storage(&address, vec![0,0,0,1], vec![0,0,0,2]).expect("avm set storage failed");
        let value = state.get_avm_storage(&address, &vec![0,0,0,1]).expect("avm get storage failed");
        assert_eq!(value, vec![0,0,0,2]);
        state.set_avm_storage(&address, vec![1,2,3,4,5,6,7,8,9,0], vec![1,2,3,4,5,0,0,0,2]).expect("avm set storage failed");
        println!("state = {:?}", state);
    }
}
