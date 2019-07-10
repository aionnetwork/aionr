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

use std::sync::Arc;
use aion_types::{U256, H256, Address};
use vms::{EnvInfo, traits::Ext,  CallType};
use state::{State, Substate};
use helpers::{get_temp_state,make_aion_machine};
use kvdb::MemoryDBRepository;
use externalities::{OriginInfo,Externalities};

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
            machine: make_aion_machine(),
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
        vec![OriginInfo::get_test_origin()],
        &mut setup.sub_state,
        Arc::new(MemoryDBRepository::new()),
    );

    assert_eq!(ext.env_info().number, 100);
}

#[test]
fn can_return_block_hash() {
    let test_hash = H256::from("afafafafafafafafafafafbcbcbcbcbcbcbcbcbcbeeeeeeeeeeeeedddddddddd");
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
        vec![OriginInfo::get_test_origin()],
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
        vec![OriginInfo::get_test_origin()],
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
            vec![OriginInfo::get_test_origin()],
            &mut setup.sub_state,
            Arc::new(MemoryDBRepository::new()),
        );
        ext.log(log_topics.clone(), &log_data);
    }

    assert_eq!(setup.sub_state.logs.len(), 1);
    assert_eq!(setup.sub_state.logs[0].topics, log_topics);
    assert_eq!(setup.sub_state.logs[0].data, log_data);
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
            vec![OriginInfo::get_test_origin()],
            &mut setup.sub_state,
            Arc::new(MemoryDBRepository::new()),
        );
        ext.suicide(refund_account);
    }

    assert_eq!(setup.sub_state.suicides.len(), 1);
}
