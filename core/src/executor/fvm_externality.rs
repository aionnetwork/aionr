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
use std::cmp;
use std::sync::Arc;
use aion_types::{H256, U256, H128, Address};
use vms::{ActionParams, ActionValue, EnvInfo, CallType, FvmExecutionResult as ExecutionResult, ExecStatus, ReturnData};
use vms::traits::Ext;
use acore_bytes::Bytes;
use state::{Backend as StateBackend, State, Substate, CleanupMode};
use machine::EthereumMachine as Machine;
use executor::fvm_exec::*;
use kvdb::KeyValueDB;
use db::{self, Readable};

/// Transaction properties that externalities need to know about.
pub struct OriginInfo {
    address: Address,
    origin: Address,
    gas_price: U256,
    value: U256,
    origin_tx_hash: H256,
}

impl OriginInfo {
    /// build a OriginInfo for test
    #[cfg(test)]
    pub fn get_test_origin() -> OriginInfo {
        OriginInfo {
            address: Address::zero(),
            origin: Address::zero(),
            gas_price: U256::zero(),
            value: U256::zero(),
            origin_tx_hash: H256::default(),
        }
    }

    /// Populates origin info from action params.
    pub fn from(params: &[&ActionParams]) -> Vec<Self> {
        params
            .iter()
            .map(|p| {
                OriginInfo {
                    address: p.address.clone(),
                    origin: p.origin.clone(),
                    gas_price: p.gas_price,
                    value: match p.value {
                        ActionValue::Transfer(val) | ActionValue::Apparent(val) => val,
                    },
                    origin_tx_hash: p.original_transaction_hash.clone(),
                }
            })
            .collect()
    }
}

/// Implementation of evm Externalities.
pub struct FvmExternalities<'a, B: 'a>
where B: StateBackend
{
    state: &'a mut State<B>,
    env_info: &'a EnvInfo,
    machine: &'a Machine,
    depth: usize,
    origin_info: Vec<OriginInfo>,
    substate: &'a mut Substate,
    db: Arc<KeyValueDB>,
}

impl<'a, B: 'a> FvmExternalities<'a, B>
where B: StateBackend
{
    /// Basic `Externalities` constructor.
    pub fn new(
        state: &'a mut State<B>,
        env_info: &'a EnvInfo,
        machine: &'a Machine,
        depth: usize,
        origin_info: Vec<OriginInfo>,
        substate: &'a mut Substate,
        kvdb: Arc<KeyValueDB>,
    ) -> Self
    {
        FvmExternalities {
            state: state,
            env_info: env_info,
            machine: machine,
            depth: depth,
            origin_info: origin_info,
            substate: substate,
            db: kvdb,
        }
    }
}

impl<'a, B: 'a> Ext for FvmExternalities<'a, B>
where B: StateBackend
{
    fn storage_at(&self, key: &H128) -> H128 {
        let value = self
            .state
            .storage_at(&self.origin_info[0].address, &key[..].to_vec())
            .expect("Fatal error occurred when getting storage.");
        let mut ret: Vec<u8> = vec![0x00; 16];
        if let Some(v) = value {
            for idx in 0..v.len() {
                ret[16 - v.len() + idx] = v[idx];
            }
        }

        ret.as_slice().into()
    }

    fn set_storage(&mut self, key: H128, value: H128) {
        // deduct the leading zeros
        let mut zeros_num = 0;
        for item in value[..].to_vec() {
            if item == 0x00 {
                zeros_num += 1;
            } else {
                break;
            }
        }
        let mut vm_bytes = Vec::new();
        vm_bytes.extend_from_slice(&value[..][zeros_num..]);
        self.state
            .set_storage(&self.origin_info[0].address, key[..].to_vec(), vm_bytes)
            .expect("Fatal error occurred when putting storage.");
    }

    fn storage_at_dword(&self, key: &H128) -> H256 {
        let value = self
            .state
            .storage_at(&self.origin_info[0].address, &key[..].to_vec())
            .expect("Fatal error occurred when getting storage.");
        let mut ret: Vec<u8> = vec![0x00; 32];
        if let Some(v) = value {
            for idx in 0..v.len() {
                ret[32 - v.len() + idx] = v[idx];
            }
        }

        ret.as_slice().into()
    }

    fn set_storage_dword(&mut self, key: H128, value: H256) {
        // value of this is always 32-byte long
        self.state
            .set_storage(
                &self.origin_info[0].address,
                key[..].to_vec(),
                value[..].to_vec(),
            )
            .expect("Fatal error occurred when putting storage.")
    }

    fn exists(&self, address: &Address) -> bool {
        self.state
            .exists(address)
            .expect("Fatal error occurred when checking account existance.")
    }

    fn exists_and_not_null(&self, address: &Address) -> bool {
        self.state
            .exists_and_not_null(address)
            .expect("Fatal error occurred when checking account existance.")
    }

    fn origin_balance(&self) -> U256 { self.balance(&self.origin_info[0].address) }

    fn balance(&self, address: &Address) -> U256 {
        self.state
            .balance(address)
            .expect("Fatal error occurred when getting balance.")
    }

    fn blockhash(&mut self, number: &U256) -> H256 {
        // TODO: comment out what this function expects from env_info, since it will produce panics if the latter is inconsistent
        debug!(target: "vm", "last_hashes = {:?}", self.env_info().last_hashes.len());
        match *number < U256::from(self.env_info.number)
            && number.low_u64() >= cmp::max(256, self.env_info.number) - 256
        {
            true => {
                let index = self.env_info.number - number.low_u64() - 1;
                assert!(
                    index < self.env_info.last_hashes.len() as u64,
                    format!(
                        "Inconsistent env_info, should contain at least {:?} last hashes",
                        index + 1
                    )
                );
                let r = self.env_info.last_hashes[index as usize].clone();
                trace!(
                    target: "ext",
                    "ext: blockhash({}) -> {} self.env_info.number={}\n",
                    number,
                    r,
                    self.env_info.number
                );
                r
            }
            false => {
                trace!(
                    target: "ext",
                    "ext: blockhash({}) -> null self.env_info.number={}\n",
                    number,
                    self.env_info.number
                );
                // for Aion, always returns the real blockhash
                match self.db.read(db::COL_EXTRA, &number.low_u64()) {
                    Some(value) => value,
                    _ => H256::zero(),
                }
            }
        }
    }

    /// Create new contract account
    fn create(&mut self, gas: &U256, value: &U256, code: &[u8]) -> ExecutionResult {
        // create new contract address
        let address = match self.state.nonce(&self.origin_info[0].address) {
            Ok(nonce) => contract_address(&self.origin_info[0].address, &nonce),
            Err(e) => {
                debug!(target: "ext", "Database corruption encountered: {:?}", e);
                return ExecutionResult {
                    gas_left: 0.into(),
                    status_code: ExecStatus::Failure,
                    return_data: ReturnData::empty(),
                    exception: String::from(
                        "Cannot get origin address and nonce from database. Database corruption \
                         may encountered.",
                    ),
                    state_root: H256::default(),
                    invokable_hashes: Default::default(),
                };
            }
        };

        // prepare the params
        let params = ActionParams {
            code_address: address.clone(),
            address: address.clone(),
            sender: self.origin_info[0].address.clone(),
            origin: self.origin_info[0].origin.clone(),
            gas: *gas,
            gas_price: self.origin_info[0].gas_price,
            value: ActionValue::Transfer(*value),
            code: Some(Arc::new(code.to_vec())),
            data: None,
            call_type: CallType::None,
            static_flag: false,
            transaction_hash: H256::default(),
            original_transaction_hash: self.origin_info[0].origin_tx_hash.clone(),
            // this field is just for avm;
            nonce: 0,
        };

        let mut result = {
            let mut ex =
                Executive::from_parent(self.state, self.env_info, self.machine, self.depth);
            ex.create(params, self.substate)
        };

        // If succeed, add address into substate, set the return_data (normally should be the deployed code) to address
        if result.status_code == ExecStatus::Success {
            self.substate.contracts_created.push(address.clone());
            let address_vec: Vec<u8> = address.clone().to_vec();
            let length: usize = address_vec.len();
            result.return_data = ReturnData::new(address_vec, 0, length);

            // Increment nonce of the caller contract account
            if let Err(e) = self.state.inc_nonce(&self.origin_info[0].address) {
                debug!(target: "ext", "Database corruption encountered: {:?}", e);
                return ExecutionResult {
                    gas_left: 0.into(),
                    status_code: ExecStatus::Failure,
                    return_data: ReturnData::empty(),
                    exception: String::from(
                        "inc_nonce failed. Database corruption may encountered.",
                    ),
                    state_root: H256::default(),
                    invokable_hashes: Default::default(),
                };
            }

            // EIP-161
            // Newly created account starts at nonce 1. (to avoiding being considered as empty/null account)
            if let Err(e) = self.state.inc_nonce(&address) {
                debug!(target: "ext", "Database corruption encountered: {:?}", e);
                return ExecutionResult {
                    gas_left: 0.into(),
                    status_code: ExecStatus::Failure,
                    return_data: ReturnData::empty(),
                    exception: String::from(
                        "inc_nonce failed. Database corruption may encountered.",
                    ),
                    state_root: H256::default(),
                    invokable_hashes: Default::default(),
                };
            }
        }

        result
    }

    /// Call contract
    fn call(
        &mut self,
        gas: &U256,
        sender_address: &Address,
        receive_address: &Address,
        value: Option<U256>,
        data: &[u8],
        code_address: &Address,
        call_type: CallType,
        static_flag: bool,
    ) -> ExecutionResult
    {
        trace!(target: "ext", "call");

        // Get code from the called account
        let code_res = self.state.code(code_address);
        let code = match code_res {
            Ok(code) => code,
            Err(_) => {
                return ExecutionResult {
                    gas_left: 0.into(),
                    status_code: ExecStatus::Failure,
                    return_data: ReturnData::empty(),
                    exception: String::from("Code not founded."),
                    state_root: H256::default(),
                    invokable_hashes: Default::default(),
                }
            }
        };

        // If there is some value to transfer, set the action from Apparent to
        // Transfer which will transfered later
        // Does not transfer value in case of CallCode or DelegateCall
        let action_value: ActionValue = if value.is_some()
            && call_type != CallType::DelegateCall
            && call_type != CallType::CallCode
        {
            ActionValue::Transfer(value.unwrap())
        } else {
            ActionValue::Apparent(self.origin_info[0].value) // Apparent value will not be transfered
        };

        let params = ActionParams {
            sender: sender_address.clone(),
            address: receive_address.clone(),
            value: action_value,
            code_address: code_address.clone(),
            origin: self.origin_info[0].origin.clone(),
            gas: *gas,
            gas_price: self.origin_info[0].gas_price,
            code: code,
            data: Some(data.to_vec()),
            call_type: call_type,
            static_flag: static_flag,
            transaction_hash: H256::default(),
            original_transaction_hash: self.origin_info[0].origin_tx_hash,
            // call fastvm here, nonce has no usage
            nonce: 0,
        };

        let mut ex = Executive::from_parent(self.state, self.env_info, self.machine, self.depth);
        ex.call(params, self.substate)
    }

    fn extcode(&self, address: &Address) -> Arc<Bytes> {
        self.state
            .code(address)
            .expect("Fatal error occurred when getting code.")
            .unwrap_or_else(|| Arc::new(vec![]))
    }

    fn extcodesize(&self, address: &Address) -> usize {
        self.state
            .code_size(address)
            .expect("Fatal error occurred when getting code size.")
            .unwrap_or(0)
    }

    fn log(&mut self, topics: Vec<H256>, data: &[u8]) {
        use log_entry::LogEntry;

        // origin_info.address is always contract address for fastvm
        let address = self.origin_info[0].address.clone();
        self.substate.logs.push(LogEntry {
            address: address,
            topics: topics,
            data: data.to_vec(),
        });
    }

    fn suicide(&mut self, refund_address: &Address) {
        let address = self.origin_info[0].address.clone();
        let balance = self.balance(&address);
        if &address == refund_address {
            // TODO [todr] To be consistent with CPP client we set balance to 0 in that case.
            self.state
                .sub_balance(&address, &balance, &mut CleanupMode::NoEmpty)
                .expect(
                    "Fatal error occurred when subtracting balance from address to be destructed",
                );
        } else {
            trace!(target: "ext", "Suiciding {} -> {} (xfer: {})", address, refund_address, balance);
            self.state
                .transfer_balance(
                    &address,
                    refund_address,
                    &balance,
                    self.substate.to_cleanup_mode(),
                )
                .expect("Fatal error occurred when transfering balance.");
        }
        self.substate.suicides.insert(address);
    }

    fn env_info(&self) -> &EnvInfo { self.env_info }

    fn depth(&self) -> usize { self.depth }

    fn inc_sstore_clears(&mut self) {
        self.substate.sstore_clears_count = self.substate.sstore_clears_count + U256::one();
    }

    fn save_code(&mut self, code: Bytes) {
        // FZH: init_code exception is not handled for now. There might be a risk.
        //      Normally It shall not fail if contract is created successfully.
        //      Need more thoughts how to handle and return init_code exception
        //      from vm module to kernel.
        self.state
            .init_code(&self.origin_info[0].address, code)
            .expect(
                "init_code should not fail as account should
            already be created before",
            );
    }

    fn save_code_at(&mut self, address: &Address, code: Bytes) {
        debug!(target: "vm", "AVM save code at: {:?}", address);
        self.state
            .init_code(address, code)
            .expect("save avm code should not fail");
    }

    fn code(&self, address: &Address) -> Option<Arc<Vec<u8>>> {
        match self.state.code(address) {
            Ok(code) => {
                //println!("code = {:?}", code);
                code
            }
            Err(_x) => None,
        }
    }

    // triggered when create a contract account with code = None
    fn set_special_empty_flag(&mut self) {
        self.state
            .set_empty_but_commit(&self.origin_info[0].address)
            .expect("set empty_but_commit flags should not fail");
    }

    fn create_account(&mut self, a: &Address) { self.state.new_contract(a, 0.into(), 0.into()) }

    fn sstore(&mut self, a: &Address, key: Vec<u8>, value: Vec<u8>) {
        self.state
            .set_storage(a, key, value)
            .expect("Fatal error occured when set storage");
    }

    fn sload(&self, a: &Address, key: &Vec<u8>) -> Option<Vec<u8>> {
        match self.state.storage_at(a, key) {
            Ok(value) => value,
            Err(_) => None,
        }
    }

    fn kill_account(&mut self, a: &Address) { self.state.kill_account(a) }

    fn inc_balance(&mut self, a: &Address, value: &U256) {
        self.state
            .add_balance(a, value, CleanupMode::NoEmpty)
            .expect("add balance failed");
    }

    fn dec_balance(&mut self, a: &Address, value: &U256) {
        self.state
            .sub_balance(a, value, &mut CleanupMode::NoEmpty)
            .expect("decrease balance failed")
    }

    fn nonce(&self, a: &Address) -> u64 { self.state.nonce(a).expect("get nonce failed").low_u64() }

    fn inc_nonce(&mut self, a: &Address) {
        self.state.inc_nonce(a).expect("increment nonce failed")
    }

    /// avm specific methods
    fn touch_account(&mut self, _a: &Address, _index: i32) { unimplemented!() }

    fn send_signal(&mut self, _signal: i32) { unimplemented!() }

    fn commit(&mut self) { unimplemented!() }

    fn root(&self) -> H256 { unimplemented!() }

    fn avm_log(&mut self, _address: &Address, _topics: Vec<H256>, _data: Vec<u8>, _index: i32) {
        unimplemented!()
    }

    fn get_transformed_code(&self, _address: &Address) -> Option<Arc<Vec<u8>>> { unimplemented!() }

    fn save_transformed_code(&mut self, _address: &Address, _code: Bytes) { unimplemented!() }

    fn get_objectgraph(&self, _address: &Address) -> Option<Arc<Bytes>> { unimplemented!() }

    fn set_objectgraph(&mut self, _address: &Address, _data: Bytes) { unimplemented!() }

    fn remove_storage(&mut self, _address: &Address, _data: Bytes) { unimplemented!() }

    fn has_storage(&mut self, address: &Address) -> bool { self.state.has_storage(address) }
}
