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
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use aion_types::{H256, U256, H128, Address};
use bytes::Bytes;
use state::{Backend as StateBackend, State, Substate, CleanupMode};
use machine::EthereumMachine as Machine;
use executive::*;
use vms::{
    ActionParams, ActionValue, Ext, EnvInfo, CallType, ExecutionResult, ExecStatus, ReturnData,
    ParamsType,
};
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
#[allow(dead_code)]
pub struct AVMExternalities<'a, B: 'a>
where B: StateBackend
{
    state: Mutex<&'a mut State<B>>,
    env_info: &'a EnvInfo,
    machine: &'a Machine,
    depth: usize,
    substates: &'a mut [Substate],
    tx: Sender<i32>,
}

impl<'a, B: 'a> AVMExternalities<'a, B>
where B: StateBackend
{
    /// Basic `Externalities` constructor.
    pub fn new(
        state: &'a mut State<B>,
        env_info: &'a EnvInfo,
        machine: &'a Machine,
        depth: usize,
        substates: &'a mut [Substate],
        tx: Sender<i32>,
    ) -> Self
    {
        AVMExternalities {
            state: Mutex::new(state),
            env_info: env_info,
            machine: machine,
            depth: depth,
            substates: substates,
            tx: tx,
        }
    }
}

/// Implementation of evm Externalities.
pub struct Externalities<'a, B: 'a>
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

impl<'a, B: 'a> Externalities<'a, B>
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
        Externalities {
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

impl<'a, B: 'a> Ext for Externalities<'a, B>
where B: StateBackend
{
    fn storage_at(&self, key: &H128) -> H128 {
        let value = self
            .state
            .storage_at(&self.origin_info[0].address, &key[..].to_vec())
            .expect("Fatal error occurred when getting storage.");
        let mut ret: Vec<u8> = vec![0x00; 16];
        for idx in 0..value.len() {
            ret[16 - value.len() + idx] = value[idx];
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
        for idx in 0..value.len() {
            ret[32 - value.len() + idx] = value[idx];
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
        let (address, code_hash) = match self.state.nonce(&self.origin_info[0].address) {
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
            code_hash: code_hash,
            data: None,
            call_type: CallType::None,
            static_flag: false,
            params_type: ParamsType::Embedded,
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
        let code_res = self
            .state
            .code(code_address)
            .and_then(|code| self.state.code_hash(code_address).map(|hash| (code, hash)));
        let (code, code_hash) = match code_res {
            Ok((code, hash)) => (code, hash),
            Err(_) => {
                return ExecutionResult {
                    gas_left: 0.into(),
                    status_code: ExecStatus::Failure,
                    return_data: ReturnData::empty(),
                    exception: String::from("Code not founded."),
                    state_root: H256::default(),
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
            code_hash: Some(code_hash),
            data: Some(data.to_vec()),
            call_type: call_type,
            static_flag: static_flag,
            params_type: ParamsType::Separate,
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
        println!("AVM get code from: {:?}", address);
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
            Ok(value) => Some(value),
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
}

#[allow(unused)]
impl<'a, B: 'a> Ext for AVMExternalities<'a, B>
where B: StateBackend
{
    fn storage_at(&self, _key: &H128) -> H128 { unimplemented!() }

    fn set_storage(&mut self, _key: H128, value: H128) { unimplemented!() }

    fn storage_at_dword(&self, _key: &H128) -> H256 { unimplemented!() }

    fn set_storage_dword(&mut self, _key: H128, _value: H256) { unimplemented!() }

    fn exists(&self, address: &Address) -> bool {
        self.state
            .lock()
            .unwrap()
            .exists(address)
            .expect("Fatal error occurred when checking account existance.")
    }

    fn exists_and_not_null(&self, address: &Address) -> bool {
        self.state
            .lock()
            .unwrap()
            .exists_and_not_null(address)
            .expect("Fatal error occurred when checking account existance.")
    }

    fn origin_balance(&self) -> U256 { unimplemented!() }

    fn balance(&self, address: &Address) -> U256 {
        self.state
            .lock()
            .unwrap()
            .balance(address)
            .expect("Fatal error occurred when getting balance.")
    }

    fn blockhash(&mut self, number: &U256) -> H256 { unimplemented!() }

    /// Create new contract account
    fn create(&mut self, gas: &U256, value: &U256, code: &[u8]) -> ExecutionResult {
        unimplemented!()
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
        unimplemented!()
    }

    fn extcode(&self, address: &Address) -> Arc<Bytes> {
        self.state
            .lock()
            .unwrap()
            .code(address)
            .expect("Fatal error occurred when getting code.")
            .unwrap_or_else(|| Arc::new(vec![]))
    }

    fn extcodesize(&self, address: &Address) -> usize {
        self.state
            .lock()
            .unwrap()
            .code_size(address)
            .expect("Fatal error occurred when getting code size.")
            .unwrap_or(0)
    }

    fn log(&mut self, topics: Vec<H256>, data: &[u8]) { unimplemented!() }

    fn suicide(&mut self, refund_address: &Address) { unimplemented!() }

    fn env_info(&self) -> &EnvInfo { self.env_info }

    fn depth(&self) -> usize { self.depth }

    fn inc_sstore_clears(&mut self) { unimplemented!() }

    fn save_code(&mut self, code: Bytes) { unimplemented!() }

    fn save_code_at(&mut self, address: &Address, code: Bytes) {
        debug!(target: "vm", "AVM save code at: {:?}", address);
        self.state
            .lock()
            .unwrap()
            .init_code(address, code)
            .expect("save avm code should not fail");
    }

    fn code(&self, address: &Address) -> Option<Arc<Vec<u8>>> {
        println!("AVM get code from: {:?}", address);
        match self.state.lock().unwrap().code(address) {
            Ok(code) => {
                //println!("code = {:?}", code);
                code
            }
            Err(_x) => None,
        }
    }

    // triggered when create a contract account with code = None
    fn set_special_empty_flag(&mut self) { unimplemented!() }

    fn create_account(&mut self, a: &Address) {
        self.state
            .lock()
            .unwrap()
            .new_contract(a, 0.into(), 0.into())
    }

    fn sstore(&mut self, a: &Address, key: Vec<u8>, value: Vec<u8>) {
        self.state
            .lock()
            .unwrap()
            .set_storage(a, key, value)
            .expect("Fatal error occured when set storage");
    }

    fn sload(&self, a: &Address, key: &Vec<u8>) -> Option<Vec<u8>> {
        match self.state.lock().unwrap().storage_at(a, key) {
            Ok(value) => Some(value),
            Err(_) => None,
        }
    }

    fn kill_account(&mut self, a: &Address) { self.state.lock().unwrap().kill_account(a) }

    fn inc_balance(&mut self, a: &Address, value: &U256) {
        self.state
            .lock()
            .unwrap()
            .add_balance(a, value, CleanupMode::NoEmpty)
            .expect("add balance failed");
    }

    fn dec_balance(&mut self, a: &Address, value: &U256) {
        self.state
            .lock()
            .unwrap()
            .sub_balance(a, value, &mut CleanupMode::NoEmpty)
            .expect("decrease balance failed")
    }

    fn nonce(&self, a: &Address) -> u64 {
        self.state
            .lock()
            .unwrap()
            .nonce(a)
            .expect("get nonce failed")
            .low_u64()
    }

    fn inc_nonce(&mut self, a: &Address) {
        self.state
            .lock()
            .unwrap()
            .inc_nonce(a)
            .expect("increment nonce failed")
    }

    fn touch_account(&mut self, a: &Address, index: i32) {
        self.substates[index as usize].touched.insert(*a);
    }

    fn send_signal(&mut self, signal: i32) { self.tx.send(signal).expect("ext send failed"); }

    fn commit(&mut self) {
        self.state
            .lock()
            .unwrap()
            .commit()
            .expect("commit state should not fail");
    }

    fn root(&self) -> H256 { self.state.lock().unwrap().root().clone() }

    fn avm_log(&mut self, address: &Address, topics: Vec<H256>, data: Vec<u8>, index: i32) {
        use log_entry::LogEntry;
        self.substates[index as usize].logs.push(LogEntry {
            address: address.clone(),
            topics,
            data,
        });
    }

    fn get_transformed_code(&self, address: &Address) -> Option<Arc<Vec<u8>>> {
        debug!(target: "vm", "AVMExt get transformed code at: {:?}", address);
        match self.state.lock().unwrap().transformed_code(address) {
            Ok(code) => {
                // println!("transformed code = {:?}", code);
                code
            }
            Err(_x) => None,
        }
    }

    fn save_transformed_code(&mut self, address: &Address, code: Bytes) {
        debug!(target: "vm", "AVMExt save transformed code: address = {:?}", address);
        self.state
            .lock()
            .unwrap()
            .init_transformed_code(address, code)
            .expect("save avm transformed code should not fail");
    }

    fn get_objectgraph(&self, address: &Address) -> Option<Arc<Bytes>> {
        debug!(target: "vm", "AVMExt get object graph");
        match self.state.lock().unwrap().get_objectgraph(address) {
            Ok(data) => {
                // println!("objectgraph = {:?}", data);
                data
            }
            Err(_x) => None,
        }
    }

    fn set_objectgraph(&mut self, address: &Address, data: Bytes) {
        debug!(target: "vm", "AVMExt save object graph: address = {:?}", address);
        self.state
            .lock()
            .unwrap()
            .set_objectgraph(address, data)
            .expect("save avm object graph should not fail");
    }
}

#[cfg(test)]
mod tests {
    use aion_types::{U256, Address};
    use vms::{EnvInfo, Ext, CallType};
    use state::{State, Substate};
    use tests::helpers::*;
    use super::*;
    use kvdb::MemoryDBRepository;

    fn get_test_origin() -> OriginInfo {
        OriginInfo {
            address: Address::zero(),
            origin: Address::zero(),
            gas_price: U256::zero(),
            value: U256::zero(),
            origin_tx_hash: H256::default(),
        }
    }

    fn get_test_env_info() -> EnvInfo {
        EnvInfo {
            number: 100,
            author: 0.into(),
            timestamp: 0,
            difficulty: 0.into(),
            last_hashes: Arc::new(vec![]),
            gas_used: 0.into(),
            gas_limit: 0.into(),
        }
    }

    struct TestSetup {
        state: State<::state_db::StateDB>,
        machine: ::machine::EthereumMachine,
        sub_state: Substate,
        env_info: EnvInfo,
    }

    impl Default for TestSetup {
        fn default() -> Self { TestSetup::new() }
    }

    impl TestSetup {
        fn new() -> Self {
            TestSetup {
                state: get_temp_state(),
                machine: ::spec::Spec::new_test_machine(),
                sub_state: Substate::new(),
                env_info: get_test_env_info(),
            }
        }
    }

    #[test]
    fn can_be_created() {
        let mut setup = TestSetup::new();
        let state = &mut setup.state;
        let ext = Externalities::new(
            state,
            &setup.env_info,
            &setup.machine,
            0,
            vec![get_test_origin()],
            &mut setup.sub_state,
            Arc::new(MemoryDBRepository::new()),
        );

        assert_eq!(ext.env_info().number, 100);
    }

    #[test]
    fn can_return_block_hash() {
        let test_hash =
            H256::from("afafafafafafafafafafafbcbcbcbcbcbcbcbcbcbeeeeeeeeeeeeedddddddddd");
        let test_env_number = 0x120001;

        let mut setup = TestSetup::new();
        {
            let env_info = &mut setup.env_info;
            env_info.number = test_env_number;
            let mut last_hashes = (*env_info.last_hashes).clone();
            last_hashes.push(test_hash.clone());
            env_info.last_hashes = Arc::new(last_hashes);
        }
        let state = &mut setup.state;

        let mut ext = Externalities::new(
            state,
            &setup.env_info,
            &setup.machine,
            0,
            vec![get_test_origin()],
            &mut setup.sub_state,
            Arc::new(MemoryDBRepository::new()),
        );

        let hash = ext.blockhash(
            &"0000000000000000000000000000000000000000000000000000000000120000"
                .parse::<U256>()
                .unwrap(),
        );

        assert_eq!(test_hash, hash);
    }

    #[test]
    #[should_panic]
    fn can_call_fail_empty() {
        let mut setup = TestSetup::new();
        let state = &mut setup.state;

        let mut ext = Externalities::new(
            state,
            &setup.env_info,
            &setup.machine,
            0,
            vec![get_test_origin()],
            &mut setup.sub_state,
            Arc::new(MemoryDBRepository::new()),
        );

        // this should panic because we have no balance on any account
        ext.call(
            &"0000000000000000000000000000000000000000000000000000000000120000"
                .parse::<U256>()
                .unwrap(),
            &Address::new(),
            &Address::new(),
            Some(
                "0000000000000000000000000000000000000000000000000000000000150000"
                    .parse::<U256>()
                    .unwrap(),
            ),
            &[],
            &Address::new(),
            CallType::Call,
            false,
        );
    }

    #[test]
    fn can_log() {
        let log_data = vec![120u8, 110u8];
        let log_topics = vec![H256::from(
            "af0fa234a6af46afa23faf23bcbc1c1cb4bcb7bcbe7e7e7ee3ee2edddddddddd",
        )];

        let mut setup = TestSetup::new();
        let state = &mut setup.state;

        {
            let mut ext = Externalities::new(
                state,
                &setup.env_info,
                &setup.machine,
                0,
                vec![get_test_origin()],
                &mut setup.sub_state,
                Arc::new(MemoryDBRepository::new()),
            );
            ext.log(log_topics, &log_data);
        }

        assert_eq!(setup.sub_state.logs.len(), 1);
    }

    #[test]
    fn can_suicide() {
        let refund_account = &Address::new();

        let mut setup = TestSetup::new();
        let state = &mut setup.state;

        {
            let mut ext = Externalities::new(
                state,
                &setup.env_info,
                &setup.machine,
                0,
                vec![get_test_origin()],
                &mut setup.sub_state,
                Arc::new(MemoryDBRepository::new()),
            );
            ext.suicide(refund_account);
        }

        assert_eq!(setup.sub_state.suicides.len(), 1);
    }
}
