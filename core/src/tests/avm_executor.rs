/*******************************************************************************
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
use std::sync::Arc;
use std::io::Error;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::path::Path;

use aion_types::{U256, Address};
use vms::{
    ActionParams,
    ActionValue,
    CallType,
    EnvInfo,
    AvmExecutionResult,
    AvmStatusCode,
};
use state::{Substate, CleanupMode};
use transaction::{Action, Transaction, SignedTransaction, DEFAULT_TRANSACTION_TYPE, AVM_TRANSACTION_TYPE};
use types::error::{ExecutionError};
use executor::fvm_exec::{contract_address};
use executor::avm_exec::{Executive as AvmExecutive};
use avm_abi::{AVMEncoder, AbiToken, ToBytes};
use helpers::{get_temp_state, make_aion_machine};

fn read_file(path: &str) -> Result<Vec<u8>, Error> {
    let path = Path::new(path);
    println!("path = {:?}", path);
    let mut file = File::open(path)?;
    let mut buf = Vec::<u8>::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

#[test]
fn avm_recursive() {
    let mut file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // NOTE: tested with avm v1.3
    file.push("src/tests/avmjars/demo-0.3.0.jar");
    let file_str = file.to_str().expect("Failed to locate the demo.jar");
    let mut code = read_file(file_str).expect("unable to open avm dapp");
    let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
    let address = contract_address(&sender, &U256::zero()).0;
    let mut params = ActionParams::default();
    params.address = address.clone();
    params.sender = sender.clone();
    params.origin = sender.clone();
    params.gas = U256::from(5_000_000);
    let mut avm_code: Vec<u8> = (code.len() as u32).to_vm_bytes();
    println!("code of hello_avm = {:?}", code.len());
    avm_code.append(&mut code);
    params.code = Some(Arc::new(avm_code.clone()));
    params.value = ActionValue::Transfer(0.into());
    params.call_type = CallType::None;
    params.gas_price = 1.into();
    let mut state = get_temp_state();
    state
        .add_balance(&sender, &U256::from(200_000_000), CleanupMode::NoEmpty)
        .unwrap();
    let info = EnvInfo::default();
    let machine = make_aion_machine();
    let substate = Substate::new();
    let execution_results = {
        let mut ex = AvmExecutive::new(&mut state, &info, &machine);
        ex.call_vm(vec![params.clone()], &mut [substate])
    };

    for r in execution_results {
        let AvmExecutionResult {
            status_code,
            gas_left: _,
            return_data,
            exception: _,
            state_root: _,
            invokable_hashes: _,
        } = r;

        assert_eq!(status_code, AvmStatusCode::Success);

        params.address = (*return_data).into();
        println!("return data = {:?}", return_data);
    }

    // Hello avm is deployed

    assert!(state.code(&params.address).unwrap().is_some());

    params.call_type = CallType::Call;
    // let call_data = AbiToken::STRING(String::from("callExt")).encode();
    let mut call_data = AbiToken::STRING(String::from("recursive")).encode();
    // for QA recursive
    let mut target = [0u8; 32];
    target.copy_from_slice(&params.address[..]);
    call_data.append(&mut AbiToken::ADDRESS(target).encode());
    call_data.append(&mut AbiToken::INT32(10).encode());
    // call_data.append(&mut AbiToken::INT32(1).encode());
    // temp QA ends
    params.data = Some(call_data);
    params.nonce += 1;
    params.gas = U256::from(2_000_000);
    println!("call data = {:?}", params.data);
    let substate = Substate::new();
    let execution_results = {
        let mut ex = AvmExecutive::new(&mut state, &info, &machine);
        ex.call_vm(vec![params.clone()], &mut [substate.clone()])
    };

    for r in execution_results {
        let AvmExecutionResult {
            status_code,
            gas_left,
            return_data: _,
            exception: _,
            state_root: _,
            invokable_hashes: _,
        } = r;

        println!("gas left = {:?}", gas_left);
        assert_eq!(status_code, AvmStatusCode::Success);
    }
}

#[test]
fn get_vote() {
    let mut file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // NOTE: tested with avm v1.3
    file.push("src/tests/avmjars/unity-staking.jar");
    let file_str = file
        .to_str()
        .expect("Failed to locate the unity-staking.jar");
    let mut code = read_file(file_str).expect("unable to open avm dapp");
    let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
    let address = contract_address(&sender, &U256::zero()).0;
    let mut params = ActionParams::default();
    params.address = address.clone();
    params.sender = sender.clone();
    params.origin = sender.clone();
    params.gas = U256::from(5_000_000);
    let mut avm_code: Vec<u8> = (code.len() as u32).to_vm_bytes();
    println!("code of hello_avm = {:?}", code.len());
    avm_code.append(&mut code);
    params.code = Some(Arc::new(avm_code.clone()));
    params.value = ActionValue::Transfer(0.into());
    params.call_type = CallType::None;
    params.gas_price = 1.into();
    let mut state = get_temp_state();
    state
        .add_balance(&sender, &U256::from(200_000_000), CleanupMode::NoEmpty)
        .unwrap();
    let info = EnvInfo::default();
    let machine = make_aion_machine();
    let substate = Substate::new();
    let execution_results = {
        let mut ex = AvmExecutive::new(&mut state, &info, &machine);
        ex.call_vm(vec![params.clone()], &mut [substate])
    };

    for r in execution_results {
        let AvmExecutionResult {
            status_code,
            gas_left: _,
            return_data,
            exception: _,
            state_root: _,
            invokable_hashes: _,
        } = r;

        assert_eq!(status_code, AvmStatusCode::Success);

        params.address = (*return_data).into();
        println!("return data = {:?}", return_data);
    }

    assert!(state.code(&params.address).unwrap().is_some());

    // register
    params.call_type = CallType::Call;
    let mut call_data = AbiToken::STRING(String::from("register")).encode();
    call_data.append(&mut AbiToken::ADDRESS(params.sender.into()).encode());
    params.data = Some(call_data);
    params.nonce += 1;
    params.gas = U256::from(2_000_000);
    params.value = ActionValue::Transfer(0.into());
    println!("call data = {:?}", params.data);
    let substate = Substate::new();
    let execution_results = {
        let mut ex = AvmExecutive::new(&mut state, &info, &machine);
        ex.call_vm(vec![params.clone()], &mut [substate.clone()])
    };

    for r in execution_results {
        let AvmExecutionResult {
            status_code,
            gas_left,
            return_data,
            exception: _,
            state_root: _,
            invokable_hashes: _,
        } = r;

        println!("gas left = {:?}, output = {:?}", gas_left, return_data);
        assert_eq!(status_code, AvmStatusCode::Success);
    }

    // Vote
    params.call_type = CallType::Call;
    let mut call_data = AbiToken::STRING(String::from("vote")).encode();
    call_data.append(&mut AbiToken::ADDRESS(params.sender.into()).encode());
    params.data = Some(call_data);
    params.nonce += 1;
    params.gas = U256::from(2_000_000);
    params.value = ActionValue::Transfer(999.into());
    println!("call data = {:?}", params.data);
    let substate = Substate::new();
    let execution_results = {
        let mut ex = AvmExecutive::new(&mut state, &info, &machine);
        ex.call_vm(vec![params.clone()], &mut [substate.clone()])
    };

    for r in execution_results {
        let AvmExecutionResult {
            status_code,
            gas_left,
            return_data,
            exception: _,
            state_root: _,
            invokable_hashes: _,
        } = r;

        println!("gas left = {:?}, output = {:?}", gas_left, return_data);
        assert_eq!(status_code, AvmStatusCode::Success);
    }

    params.call_type = CallType::Call;
    let mut call_data = AbiToken::STRING(String::from("getVote")).encode();
    call_data.append(&mut AbiToken::ADDRESS(params.sender.into()).encode());
    params.data = Some(call_data);
    params.nonce += 1;
    params.gas = U256::from(2_000_000);
    params.value = ActionValue::Transfer(0.into());
    println!("call data = {:?}", params.data);
    let substate = Substate::new();
    let execution_results = {
        let mut ex = AvmExecutive::new(&mut state, &info, &machine);
        ex.call_vm(vec![params.clone()], &mut [substate.clone()])
    };

    for r in execution_results {
        let AvmExecutionResult {
            status_code,
            gas_left,
            return_data,
            exception: _,
            state_root: _,
            invokable_hashes: _,
        } = r;

        println!("gas left = {:?}, output = {:?}", gas_left, return_data);
        assert_eq!(status_code, AvmStatusCode::Success);
        assert_eq!(return_data.to_vec(), vec![3, 231]);
    }
}

#[test]
/// HelloWorld with extra storage test
fn hello_avm() {
    let mut file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // NOTE: tested with avm v1.3
    file.push("src/tests/avmjars/demo-0.2.0.jar");
    let file_str = file.to_str().expect("Failed to locate the demo.jar");
    let mut code = read_file(file_str).expect("unable to open avm dapp");
    let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
    let address = contract_address(&sender, &U256::zero()).0;
    let mut params = ActionParams::default();
    params.address = address.clone();
    params.sender = sender.clone();
    params.origin = sender.clone();
    params.gas = U256::from(5_000_000);
    let mut avm_code: Vec<u8> = (code.len() as u32).to_vm_bytes();
    println!("code of hello_avm = {:?}", code.len());
    avm_code.append(&mut code);
    params.code = Some(Arc::new(avm_code.clone()));
    params.value = ActionValue::Transfer(0.into());
    params.call_type = CallType::None;
    params.gas_price = 1.into();
    let mut state = get_temp_state();
    state
        .add_balance(&sender, &U256::from(200_000_000), CleanupMode::NoEmpty)
        .unwrap();
    let info = EnvInfo::default();
    let machine = make_aion_machine();
    let substate = Substate::new();
    let execution_results = {
        let mut ex = AvmExecutive::new(&mut state, &info, &machine);
        ex.call_vm(vec![params.clone()], &mut [substate])
    };

    for r in execution_results {
        let AvmExecutionResult {
            status_code,
            gas_left: _,
            return_data,
            exception: _,
            state_root: _,
            invokable_hashes: _,
        } = r;

        assert_eq!(status_code, AvmStatusCode::Success);

        params.address = (*return_data).into();
        println!("return data = {:?}", return_data);
    }

    // Hello avm is deployed

    assert!(state.code(&params.address).unwrap().is_some());

    params.call_type = CallType::Call;
    let call_data = AbiToken::STRING(String::from("callExt")).encode();
    params.data = Some(call_data);
    params.nonce += 1;
    params.gas = U256::from(2_000_000);
    println!("call data = {:?}", params.data);
    let substate = Substate::new();
    let execution_results = {
        let mut ex = AvmExecutive::new(&mut state, &info, &machine);
        ex.call_vm(vec![params.clone()], &mut [substate.clone()])
    };

    for r in execution_results {
        let AvmExecutionResult {
            status_code,
            gas_left,
            return_data: _,
            exception: _,
            state_root: _,
            invokable_hashes: _,
        } = r;

        println!("gas left = {:?}", gas_left);
        assert_eq!(status_code, AvmStatusCode::Success);
    }
}

#[test]
fn avm_storage() {
    let mut state = get_temp_state();
    let address = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
    state
        .set_storage(&address, vec![0, 0, 0, 1], vec![0])
        .expect("avm set storage failed");
    let value = state
        .storage_at(&address, &vec![0, 0, 0, 1])
        .expect("avm get storage failed");
    assert_eq!(value, Some(vec![0]));
    state
        .set_storage(
            &address,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 0],
            vec![1, 2, 3, 4, 5, 0, 0, 0, 2],
        )
        .expect("avm set storage failed");
    state
        .remove_storage(&address, vec![0, 0, 0, 1])
        .expect("remove failed");
    let value = state
        .storage_at(&address, &vec![0, 0, 0, 1])
        .expect("set storage failed");
    assert_eq!(value, None);
    // println!("state = {:?}", state);
}

#[test]
fn avm_balance_transfer() {
    let mut state = get_temp_state();
    let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
    let mut params = ActionParams::default();
    let address = contract_address(&sender, &U256::zero()).0;
    params.address = address.clone();
    params.sender = sender.clone();
    params.origin = sender.clone();
    params.gas = U256::from(1_000_000);
    params.code = Some(Arc::new(vec![]));
    params.value = ActionValue::Transfer(100.into());
    params.call_type = CallType::BulkBalance;
    params.gas_price = 1.into();
    state
        .add_balance(&sender, &U256::from(50_000_000), CleanupMode::NoEmpty)
        .unwrap();
    let info = EnvInfo::default();
    let machine = make_aion_machine();
    let substate = Substate::new();
    let mut params2 = params.clone();
    params2.address = contract_address(&sender, &U256::one()).0;
    params2.value = ActionValue::Transfer(99.into());
    params2.nonce = params.nonce + 1;
    let results = {
        let mut ex = AvmExecutive::new(&mut state, &info, &machine);
        ex.call_vm(
            vec![params.clone(), params2.clone()],
            &mut [substate.clone(), substate.clone()],
        )
    };

    assert_eq!(results.len(), 2);
    assert_eq!(state.balance(&address), Ok(100.into()));
    assert_eq!(state.balance(&params2.address), Ok(99.into()));
}

#[test]
fn avm_status_rejected() {
    let mut file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // NOTE: tested with avm v1.3
    file.push("src/tests/avmjars/demo-0.2.0.jar");
    let file_str = file.to_str().expect("Failed to locate the demo.jar");
    let mut code = read_file(file_str).expect("unable to open avm dapp");
    let mut avm_code: Vec<u8> = (code.len() as u32).to_vm_bytes();
    println!("code of hello_avm = {:?}", code.len());
    avm_code.append(&mut code);

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
        U256::one(),
        U256::zero(),
        U256::from(4_000_000),
        Action::Create,
        0.into(),
        avm_code.clone(),
        DEFAULT_TRANSACTION_TYPE,
        None,
    );
    let signed_transaction: SignedTransaction = transaction.fake_sign(sender);
    let results = {
        let mut ex = AvmExecutive::new(&mut state, &info, &machine);
        ex.transact(&[signed_transaction.clone()], false, true)
    };
    assert_eq!(
        results[0].clone().unwrap_err(),
        ExecutionError::BlockGasLimitReached {
            gas_limit: U256::from(3_000_000),
            gas_used: U256::from(0),
            gas: U256::from(4_000_000),
        }
    );

    let transaction: Transaction = Transaction::new(
        U256::zero(),
        U256::zero(),
        U256::from(2_000_000),
        Action::Create,
        0.into(),
        avm_code.clone(),
        AVM_TRANSACTION_TYPE,
        None,
    );

    let signed_transaction: SignedTransaction = transaction.fake_sign(sender);
    let results = {
        let mut ex = AvmExecutive::new(&mut state, &info, &machine);
        ex.transact(&[signed_transaction], false, true)
    };
    assert!(results[0].is_err());
}
