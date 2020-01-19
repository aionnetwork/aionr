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
use crate::state::account::{AionVMAccount,AccType,VMAccount};
use kvdb::MemoryDB;
use crate::db::AccountDBMut;
use aion_types::Address;

use std::sync::Arc;

#[test]
fn storage_at() {
    let mut db = MemoryDB::new();
    let mut db = AccountDBMut::new(&mut db, &Address::new());
    let rlp = {
        let mut a = AionVMAccount::new_contract(69.into(), 0.into());
        let key = vec![0u8; 16];
        a.set_storage(key, vec![0x12, 0x34]);
        a.commit_storage(&Default::default(), &mut db).unwrap();
        a.init_code(vec![]);
        a.commit_code(&mut db);
        a.rlp()
    };

    let a = AionVMAccount::from_rlp(&rlp);
    assert_eq!(
        *a.storage_root().unwrap(),
        "d2e59a50e7414e56da75917275d1542a13fd345bf88a657a4222a0d50ad58868".into()
    );
    let value = a.storage_at(&db.immutable(), &vec![0x00; 16]).unwrap();
    assert_eq!(value, Some(vec![0x12, 0x34]));
    let value = a.storage_at(&db.immutable(), &vec![0x01]).unwrap();
    assert_eq!(value, Some(Vec::<u8>::new()));
}

#[test]
fn commit_storage() {
    let mut a = AionVMAccount::new_contract(69.into(), 0.into());
    let mut db = MemoryDB::new();
    let mut db = AccountDBMut::new(&mut db, &Address::new());
    a.set_storage(vec![0u8; 16], vec![0x12, 0x34]);
    assert_eq!(a.storage_root(), None);
    a.commit_storage(&Default::default(), &mut db).unwrap();
    assert_eq!(
        *a.storage_root().unwrap(),
        "d2e59a50e7414e56da75917275d1542a13fd345bf88a657a4222a0d50ad58868".into()
    );
}

#[test]
fn note_code() {
    let mut db = MemoryDB::new();
    let mut db = AccountDBMut::new(&mut db, &Address::new());

    let rlp = {
        let mut a = AionVMAccount::new_contract(69.into(), 0.into());
        a.init_code(vec![0x55, 0x44, 0xffu8]);
        a.commit_code(&mut db);
        a.rlp()
    };

    let mut a = AionVMAccount::from_rlp(&rlp);
    assert!(a.cache_code(&db.immutable()).is_some());

    let mut a = AionVMAccount::from_rlp(&rlp);
    assert_eq!(a.note_code(vec![0x55, 0x44, 0xffu8]), Ok(()));
}

#[test]
fn cache_transformed_code() {
    let address = Address::new();
    let mut db = MemoryDB::new();
    let mut db = AccountDBMut::new(&mut db, &address);
    let mut a = AionVMAccount::new_contract(69.into(), 0.into());

    let rlp = {
        a.init_transformed_code(vec![0x55, 0x44, 0xffu8]);
        // update account's address hash
        a.address_hash(&address);
        a.commit_code(&mut db);
        a.rlp()
    };

    let mut a = AionVMAccount::from_rlp(&rlp);
    a.address_hash(&address);
    assert_eq!(a.cache_code(&db.immutable()), Some(Arc::new(vec![])));
    assert_eq!(a.account_type, AccType::FVM);
    assert_eq!(
        a.cache_transformed_code(&db.immutable()),
        Some(Arc::new(vec![0x55, 0x44, 0xffu8]))
    );
    assert_eq!(a.account_type, AccType::AVM);
}

// #[test]
// fn cache_objectgraph() {
//     let address = Address::new();
//     let mut db = MemoryDB::new();
//     let mut db = AccountDBMut::new(&mut db, &address);
//     let mut a = AionVMAccount::new_contract(69.into(), 0.into());
//     let kvdb = Mockkvdb::new_default();

//     let rlp = {
//         a.init_objectgraph(vec![0x55, 0x44, 0xffu8]);
//         a.commit_storage(&Default::default(), &mut db).unwrap();
//         // calculate delta_root and save it in accountDB
//         a.update_root(&address, Arc::new(kvdb));
//         a.rlp()
//     };

//     let mut a = AionVMAccount::from_rlp(&rlp);
//     assert_eq!(
//         a.cache_objectgraph(&address, &db.immutable()),
//         Some(Arc::new(vec![0x55, 0x44, 0xffu8]))
//     );
// }

#[test]
fn cached_storage_at() {
    let mut db = MemoryDB::new();
    let mut db = AccountDBMut::new(&mut db, &Address::new());
    let mut a = AionVMAccount::new_contract(69.into(), 0.into());

    a.set_storage(vec![0x12, 0x34], vec![0x67, 0x78]);
    a.commit_storage(&Default::default(), &mut db).unwrap();

    assert!(a.cached_storage_at(&vec![0x12, 0x34]).is_some());

    a.storage_cache.borrow_mut().clear();
    assert!(a.cached_storage_at(&vec![0x12, 0x34]).is_none());
}
