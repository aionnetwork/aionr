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
use vms::{EnvInfo, CallType, FvmExecutionResult};
use vms::traits::Ext;
use acore_bytes::Bytes;
use state::{Backend as StateBackend, State, Substate, CleanupMode};
use machine::EthereumMachine as Machine;
use db::{self, Readable};

/// Implementation of avm Externalities.
/// state: the world state
/// env_info: block info in which transaction is executed
/// machine: machine info, hard fork config .e.g
/// substates: temp state from vm, will be finalized when execution is done
/// tx: the message sender used for debugging, mainly for callback info
#[allow(unused)]
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

impl<'a, B: 'a> Ext for AVMExternalities<'a, B>
where B: StateBackend
{
    fn storage_at(&self, _key: &H128) -> H128 { unimplemented!() }

    fn set_storage(&mut self, _key: H128, _value: H128) { unimplemented!() }

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

    fn blockhash(&mut self, number: &U256) -> H256 {
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
                let db = self.state.lock().unwrap().export_kvdb();
                match db.read(db::COL_EXTRA, &number.low_u64()) {
                    Some(value) => value,
                    _ => H256::zero(),
                }
            }
        }
    }

    /// Create new contract account
    fn create(&mut self, _gas: &U256, _value: &U256, _code: &[u8]) -> FvmExecutionResult {
        unimplemented!()
    }

    /// Call contract
    fn call(
        &mut self,
        _gas: &U256,
        _sender_address: &Address,
        _receive_address: &Address,
        _value: Option<U256>,
        _data: &[u8],
        _code_address: &Address,
        _call_type: CallType,
        _static_flag: bool,
    ) -> FvmExecutionResult
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

    fn log(&mut self, _topics: Vec<H256>, _data: &[u8]) { unimplemented!() }

    fn suicide(&mut self, _refund_address: &Address) { unimplemented!() }

    fn env_info(&self) -> &EnvInfo { self.env_info }

    fn depth(&self) -> usize { self.depth }

    fn inc_sstore_clears(&mut self) { unimplemented!() }

    fn save_code(&mut self, _code: Bytes) { unimplemented!() }

    fn save_code_at(&mut self, address: &Address, code: Bytes) {
        debug!(target: "vm", "AVM save code at: {:?}", address);
        self.state
            .lock()
            .unwrap()
            .init_code(address, code)
            .expect("save avm code should not fail");
    }

    fn code(&self, address: &Address) -> Option<Arc<Vec<u8>>> {
        debug!(target: "vm", "AVM get code from: {:?}", address);
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
            Ok(value) => value,
            Err(_) => None,
        }
    }

    fn remove_storage(&mut self, a: &Address, key: Vec<u8>) {
        self.state
            .lock()
            .unwrap()
            .remove_storage(a, key)
            .expect("Fatal error during removing storage");
    }

    fn has_storage(&mut self, a: &Address) -> bool { self.state.lock().unwrap().has_storage(a) }

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
