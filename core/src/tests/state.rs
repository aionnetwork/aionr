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
use aion_types::{Address, H256, U256};
use key::Ed25519Secret;
use receipt::{SimpleReceipt,Receipt};
use rustc_hex::FromHex;
use std::str::FromStr;
use std::sync::Arc;
use vms::EnvInfo;
use state::{State,CleanupMode};
use super::common::helpers::{get_temp_state,get_temp_state_db};
use kvdb::MockDbRepository;
use transaction::{Transaction,Action};
use machine::EthereumMachine;
use spec::spec::Spec;

use std::io::Error;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::path::Path;

use avm_abi::{AVMEncoder, AbiToken, ToBytes};

fn read_file(path: &str) -> Result<Vec<u8>, Error> {
    let path = Path::new(path);
    println!("path = {:?}", path);
    let mut file = File::open(path)?;
    let mut buf = Vec::<u8>::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

fn secret() -> Ed25519Secret {
    Ed25519Secret::from_str("7ea8af7d0982509cd815096d35bc3a295f57b2a078e4e25731e3ea977b9544626702b86f33072a55f46003b1e3e242eb18556be54c5ab12044c3c20829e0abb5").unwrap()
}

fn new_frontier_test_machine() -> EthereumMachine {
    Spec::load_machine(include_bytes!("../../../resources/mastery.json").as_ref())
        .expect("chain spec is invalid")
}

fn make_frontier_machine() -> EthereumMachine {
    let machine = new_frontier_test_machine();
    machine
}

#[test]
fn no_log_on_failure() {
    let mut state = get_temp_state();
    let mut info = EnvInfo::default();
    info.gas_limit = 10_000_000.into();
    let machine = make_frontier_machine();

    let mut file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    file.push("src/tests/avmjars/APITest-1.0-SNAPSHOT.jar");
    let file_str = file.to_str().expect("Failed to locate the APITest.jar");
    let mut code = read_file(file_str).expect("unable to open avm dapp");
    let mut avm_code: Vec<u8> = (code.len() as u32).to_vm_bytes();
    println!("code of hello_avm: {:?}", code.len());
    avm_code.append(&mut code);

    let t = Transaction {
        nonce: 0.into(),
        nonce_bytes: Vec::new(),
        gas_price: 1.into(),
        gas_price_bytes: Vec::new(),
        gas: 5_000_000.into(),
        gas_bytes: (5_000_000 as u32).to_vm_bytes(),
        action: Action::Create,
        value: 0.into(),
        value_bytes: Vec::new(),
        transaction_type: 2.into(),
        data: avm_code,
        beacon: None,
    }
    .sign(&secret());

    state
        .add_balance(&t.sender(), &(200_000_000.into()), CleanupMode::NoEmpty)
        .unwrap();
    let outcome = state.apply_batch(&info, &machine, &[t], true);
    for res in outcome {
        let receipt = res.unwrap().receipt.clone();
        assert_eq!(
            receipt,
            Receipt {
                simple_receipt: SimpleReceipt {
                    log_bloom: "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into(),
                    logs: vec![],
                    state_root: H256::from("0x0291b9a76731d9a3dd46e9e278d71bd631abba9ea332da7c92293cb826705c32")
                },
                output: vec![160, 200, 30, 216, 168, 193, 192, 177, 233, 149, 176, 45, 136, 179, 60, 129, 75, 104, 122, 126, 153, 49, 53, 107, 191, 83, 242, 85, 210, 185, 25, 80],
                gas_used: U256::from(767347),
                error_message: String::new(),
                transaction_fee: U256::from(767347)
            }
        );
    }

    let call_data = AbiToken::STRING(String::from("log_on_failure")).encode();
    let target_addr = vec![
        160u8, 200, 30, 216, 168, 193, 192, 177, 233, 149, 176, 45, 136, 179, 60, 129, 75, 104,
        122, 126, 153, 49, 53, 107, 191, 83, 242, 85, 210, 185, 25, 80,
    ];
    let t = Transaction {
        nonce: 1.into(),
        nonce_bytes: vec![0x1u8],
        gas_price: 1.into(),
        gas_price_bytes: Vec::new(),
        gas: 1_000_000.into(),
        gas_bytes: Vec::new(),
        action: Action::Call(target_addr.as_slice().into()),
        value: 0.into(),
        value_bytes: Vec::new(),
        transaction_type: 1.into(),
        data: call_data,
        beacon: None,
    }
    .sign(&secret());

    state
        .add_balance(&t.sender(), &(5_000_000.into()), CleanupMode::NoEmpty)
        .unwrap();
    let outcome = state.apply_batch(&info, &machine, &[t], true);

    for res in outcome {
        let expected_receipt = Receipt {
            simple_receipt: SimpleReceipt{log_bloom: "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into(),
            logs: vec![], state_root: H256::from(
                    "0x69087be2ec1fef65c9cfbdc143d9a669bd250b5ab1a0f44ee3a15686e1081aef"
                ), },
            output: vec![],
            gas_used: U256::from(1000000),
            error_message:  String::from("AvmFailure"),
            transaction_fee: U256::from(1000000),
        };

        assert_eq!(res.unwrap().receipt, expected_receipt);
    }
}

#[test]
fn should_apply_create_transaction() {
    let mut state = get_temp_state();
    let mut info = EnvInfo::default();
    info.gas_limit = 1_000_000.into();
    let machine = make_frontier_machine();

    let t = Transaction {
        nonce: 0.into(),
        nonce_bytes: Vec::new(),
        gas_price: 0.into(),
        gas_price_bytes: Vec::new(),
        gas: 500_000.into(),
        gas_bytes: Vec::new(),
        action: Action::Create,
        value: 100.into(),
        value_bytes: Vec::new(),
        transaction_type: 1.into(),
        data: FromHex::from_hex("601080600c6000396000f3006000355415600957005b60203560003555")
            .unwrap(),
        beacon: None,
    }
    .sign(&secret());

    state
        .add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty)
        .unwrap();
    let result = state.apply(&info, &machine, &t, true).unwrap();

    let expected_receipt = Receipt {
            simple_receipt: SimpleReceipt{log_bloom: "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into(),
            logs: vec![], state_root: H256::from(
                    "0xadfb0633de8b1effff5c6b4f347b435f99e48339164160ee04bac13115c90dc9"
                ), },
            output: vec![96, 0, 53, 84, 21, 96, 9, 87, 0, 91, 96, 32, 53, 96, 0, 53],
            gas_used: U256::from(222506),
            error_message:  String::new(),
            transaction_fee: U256::from(0),
        };

    assert_eq!(result.receipt, expected_receipt);
}

#[test]
fn should_work_when_cloned() {
    let a = Address::zero();

    let mut state = {
        let mut state = get_temp_state();
        assert_eq!(state.exists(&a).unwrap(), false);
        state.inc_nonce(&a).unwrap();
        state.commit().unwrap();
        state.clone()
    };

    state.inc_nonce(&a).unwrap();
    state.commit().unwrap();
}

#[test]
fn remove() {
    let a = Address::zero();
    let mut state = get_temp_state();
    assert_eq!(state.exists(&a).unwrap(), false);
    assert_eq!(state.exists_and_not_null(&a).unwrap(), false);
    state.inc_nonce(&a).unwrap();
    assert_eq!(state.exists(&a).unwrap(), true);
    assert_eq!(state.exists_and_not_null(&a).unwrap(), true);
    assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
    state.kill_account(&a);
    assert_eq!(state.exists(&a).unwrap(), false);
    assert_eq!(state.exists_and_not_null(&a).unwrap(), false);
    assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
}

#[test]
fn empty_account_is_not_created() {
    let a = Address::zero();
    let db = get_temp_state_db();
    let (root, db) = {
        let mut state = State::new(
            db,
            U256::from(0),
            Default::default(),
            Arc::new(MockDbRepository::init(vec![])),
        );
        state
            .add_balance(&a, &U256::default(), CleanupMode::NoEmpty)
            .unwrap(); // create an empty account
        state.commit().unwrap();
        state.drop()
    };
    let state = State::from_existing(
        db,
        root,
        U256::from(0u8),
        Default::default(),
        Arc::new(MockDbRepository::init(vec![])),
    )
    .unwrap();
    assert!(!state.exists(&a).unwrap());
    assert!(!state.exists_and_not_null(&a).unwrap());
}

#[test]
fn empty_account_exists_when_creation_forced() {
    let a = Address::zero();
    let db = get_temp_state_db();
    let (root, db) = {
        println!("default balance = {}", U256::default());
        let mut state = State::new(
            db,
            U256::from(0),
            Default::default(),
            Arc::new(MockDbRepository::init(vec![])),
        );
        state
            .add_balance(&a, &U256::default(), CleanupMode::ForceCreate)
            .unwrap(); // create an empty account
        state.commit().unwrap();
        state.drop()
    };
    let state = State::from_existing(
        db,
        root,
        U256::from(0u8),
        Default::default(),
        Arc::new(MockDbRepository::init(vec![])),
    )
    .unwrap();

    assert!(!state.exists(&a).unwrap());
    assert!(!state.exists_and_not_null(&a).unwrap());
}

#[test]
fn remove_from_database() {
    let a = Address::zero();
    let (root, db) = {
        let mut state = get_temp_state();
        state.inc_nonce(&a).unwrap();
        state.commit().unwrap();
        assert_eq!(state.exists(&a).unwrap(), true);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
        state.drop()
    };

    let (root, db) = {
        let mut state = State::from_existing(
            db,
            root,
            U256::from(0u8),
            Default::default(),
            Arc::new(MockDbRepository::init(vec![])),
        )
        .unwrap();
        assert_eq!(state.exists(&a).unwrap(), true);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
        state.kill_account(&a);
        state.commit().unwrap();
        assert_eq!(state.exists(&a).unwrap(), false);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
        state.drop()
    };

    let state = State::from_existing(
        db,
        root,
        U256::from(0u8),
        Default::default(),
        Arc::new(MockDbRepository::init(vec![])),
    )
    .unwrap();
    assert_eq!(state.exists(&a).unwrap(), false);
    assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
}

#[test]
fn alter_balance() {
    let mut state = get_temp_state();
    let a = Address::zero();
    let b = 1u64.into();
    state
        .add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty)
        .unwrap();
    assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    state.commit().unwrap();
    assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    state
        .sub_balance(&a, &U256::from(42u64), &mut CleanupMode::NoEmpty)
        .unwrap();
    assert_eq!(state.balance(&a).unwrap(), U256::from(27u64));
    state.commit().unwrap();
    assert_eq!(state.balance(&a).unwrap(), U256::from(27u64));
    state
        .transfer_balance(&a, &b, &U256::from(18u64), CleanupMode::NoEmpty)
        .unwrap();
    assert_eq!(state.balance(&a).unwrap(), U256::from(9u64));
    assert_eq!(state.balance(&b).unwrap(), U256::from(18u64));
    state.commit().unwrap();
    assert_eq!(state.balance(&a).unwrap(), U256::from(9u64));
    assert_eq!(state.balance(&b).unwrap(), U256::from(18u64));
}

#[test]
fn alter_nonce() {
    let mut state = get_temp_state();
    let a = Address::zero();
    state.inc_nonce(&a).unwrap();
    assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
    state.inc_nonce(&a).unwrap();
    assert_eq!(state.nonce(&a).unwrap(), U256::from(2u64));
    state.commit().unwrap();
    assert_eq!(state.nonce(&a).unwrap(), U256::from(2u64));
    state.inc_nonce(&a).unwrap();
    assert_eq!(state.nonce(&a).unwrap(), U256::from(3u64));
    state.commit().unwrap();
    assert_eq!(state.nonce(&a).unwrap(), U256::from(3u64));
}

#[test]
fn balance_nonce() {
    let mut state = get_temp_state();
    let a = Address::zero();
    assert_eq!(state.balance(&a).unwrap(), U256::from(0u64));
    assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
    state.commit().unwrap();
    assert_eq!(state.balance(&a).unwrap(), U256::from(0u64));
    assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
}

#[test]
fn checkpoint_basic() {
    let mut state = get_temp_state();
    let a = Address::zero();
    state.checkpoint();
    state
        .add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty)
        .unwrap();
    assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    state.discard_checkpoint();
    assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    state.checkpoint();
    state
        .add_balance(&a, &U256::from(1u64), CleanupMode::NoEmpty)
        .unwrap();
    assert_eq!(state.balance(&a).unwrap(), U256::from(70u64));
    state.revert_to_checkpoint();
    assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
}

#[test]
fn checkpoint_nested() {
    let mut state = get_temp_state();
    let a = Address::zero();
    state.checkpoint();
    state.checkpoint();
    state
        .add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty)
        .unwrap();
    assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    state.discard_checkpoint();
    assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    state.revert_to_checkpoint();
    assert_eq!(state.balance(&a).unwrap(), U256::from(0));
}

#[test]
fn create_empty() {
    let mut state = get_temp_state();
    state.commit().unwrap();
    assert_eq!(
        *state.root(),
        "45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0".into()
    );
}

#[test]
fn should_not_panic_on_state_diff_with_storage() {
    let mut state = get_temp_state();

    let a: Address = 0xa.into();
    state.init_code(&a, b"abcdefg".to_vec()).unwrap();;
    state
        .add_balance(&a, &256.into(), CleanupMode::NoEmpty)
        .unwrap();
    state.set_storage(&a, vec![0x0b], vec![0x0c]).unwrap();

    let mut new_state = state.clone();
    new_state.set_storage(&a, vec![0x0b], vec![0x0d]).unwrap();

    new_state.diff_from(state).unwrap();
}
