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
/// tests the operation of docking with the database with `require` or `require_or_from`
/// public function will test in tests dir
use super::super::{State, CleanupMode, AccType, AionVMAccount};
use std::sync::Arc;
use aion_types::{Address, U256};
use helpers::{get_temp_state,get_temp_state_with_nonce};
use kvdb::MockDbRepository;

#[test]
fn balance_from_database() {
    let a = Address::zero();
    let (root, db) = {
        let mut state = get_temp_state();
        state
            .require_or_from(
                &a,
                false,
                || AionVMAccount::new_contract(42.into(), 0.into()),
                |_| {},
            )
            .unwrap();
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), 42.into());
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
    assert_eq!(state.balance(&a).unwrap(), 42.into());
}

#[test]
fn avm_empty_bytes_or_null() {
    let a = Address::zero();
    let mut state = get_temp_state();
    state
        .require_or_from(
            &a,
            false,
            || {
                let mut acc = AionVMAccount::new_contract(42.into(), 0.into());
                acc.account_type = AccType::AVM;
                acc
            },
            |_| {},
        )
        .unwrap();
    let key = vec![0x01];
    let value = vec![];
    state.set_storage(&a, key.clone(), value).unwrap();
    assert_eq!(state.storage_at(&a, &key).unwrap(), Some(vec![]));
    state.commit().unwrap();
    state.remove_storage(&a, key.clone()).unwrap();
    // remove unexisting key
    state.remove_storage(&a, vec![0x02]).unwrap();
    state.commit().unwrap();
    state.set_storage(&a, vec![0x02], vec![0x03]).unwrap();
    // clean local cache
    state.commit().unwrap();
    assert_eq!(state.storage_at(&a, &key).unwrap(), None);
    assert_eq!(state.storage_at(&a, &vec![0x02]).unwrap(), Some(vec![0x03]));
}

#[test]
fn code_from_database() {
    let a = Address::zero();

    let (root, db) = {
        let mut state = get_temp_state();
        state
            .require_or_from(
                &a,
                false,
                || AionVMAccount::new_contract(42.into(), 0.into()),
                |_| {},
            )
            .unwrap();
        state.init_code(&a, vec![1, 2, 3]).unwrap();
        assert_eq!(state.code(&a).unwrap(), Some(Arc::new(vec![1u8, 2, 3])));
        state.commit().unwrap();
        assert_eq!(state.code(&a).unwrap(), Some(Arc::new(vec![1u8, 2, 3])));
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
    assert_eq!(state.code(&a).unwrap(), Some(Arc::new(vec![1u8, 2, 3])));
}

#[test]
fn transformed_code_from_database() {
    let a = Address::zero();
    let kvdb;
    let (root, db) = {
        let mut state = get_temp_state();
        state
            .require_or_from(
                &a,
                false,
                || AionVMAccount::new_contract(42.into(), 0.into()),
                |_| {},
            )
            .unwrap();
        state.init_transformed_code(&a, vec![1, 2, 3]).unwrap();
        assert_eq!(
            state.transformed_code(&a).unwrap(),
            Some(Arc::new(vec![1u8, 2, 3]))
        );
        state.commit().unwrap();
        kvdb = state.export_kvdb();
        assert_eq!(
            state.transformed_code(&a).unwrap(),
            Some(Arc::new(vec![1u8, 2, 3]))
        );
        state.drop()
    };

    let state = State::from_existing(db, root, U256::from(0u8), Default::default(), kvdb).unwrap();
    // let _ = state.set_objectgraph(&a, vec![]);
    assert_eq!(
        state.transformed_code(&a).unwrap(),
        Some(Arc::new(vec![1u8, 2, 3]))
    );
}

#[test]
fn storage_at_from_database() {
    let a = Address::zero();
    let (root, db) = {
        let mut state = get_temp_state_with_nonce();
        state.set_storage(&a, vec![2], vec![69]).unwrap();
        state.commit().unwrap();
        state.drop()
    };

    let s = State::from_existing(
        db,
        root,
        U256::from(0u8),
        Default::default(),
        Arc::new(MockDbRepository::init(vec![])),
    )
    .unwrap();
    assert_eq!(s.storage_at(&a, &vec![2]).unwrap_or(None), Some(vec![69]));
}

#[test]
fn get_from_database() {
    let a = Address::zero();
    let (root, db) = {
        let mut state = get_temp_state();
        state.inc_nonce(&a).unwrap();
        state
            .add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty)
            .unwrap();
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.drop()
    };

    let state = State::from_existing(
        db,
        root,
        U256::from(1u8),
        Default::default(),
        Arc::new(MockDbRepository::init(vec![])),
    )
    .unwrap();
    assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
}

#[test]
fn ensure_cached() {
    let mut state = get_temp_state_with_nonce();
    let a = Address::zero();
    state.require(&a, false).unwrap();
    state.commit().unwrap();
    assert_eq!(
        *state.root(),
        "9d6d4b335038e1ffe0f060c29e52d6eed2aec4a085dfa37afba9d1e10cc7be85".into()
    );
}
