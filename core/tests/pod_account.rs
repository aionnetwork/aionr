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

#![warn(unused_extern_crates)]

#[macro_use]
extern crate macros;
extern crate acore;
extern crate common_types;

use std::collections::BTreeMap;
use common_types::account_diff::{ AccountDiff, Diff };
use acore::pod_account::{PodAccount, diff_pod};

#[test]
fn existence() {
    let a = PodAccount {
        balance: 69.into(),
        nonce: 0.into(),
        code: Some(vec![]),
        storage: map![],
        storage_dword: map![],
    };
    assert_eq!(diff_pod(Some(&a), Some(&a)), None);
    assert_eq!(
        diff_pod(None, Some(&a)),
        Some(AccountDiff {
            balance: Diff::Born(69.into()),
            nonce: Diff::Born(0.into()),
            code: Diff::Born(vec![]),
            storage: map![],
            storage_dword: map![],
        })
    );
}

#[test]
fn basic() {
    let a = PodAccount {
        balance: 69.into(),
        nonce: 0.into(),
        code: Some(vec![]),
        storage: map![],
        storage_dword: map![],
    };
    let b = PodAccount {
        balance: 42.into(),
        nonce: 1.into(),
        code: Some(vec![]),
        storage: map![],
        storage_dword: map![],
    };
    assert_eq!(
        diff_pod(Some(&a), Some(&b)),
        Some(AccountDiff {
            balance: Diff::Changed(69.into(), 42.into()),
            nonce: Diff::Changed(0.into(), 1.into()),
            code: Diff::Same,
            storage: map![],
            storage_dword: map![],
        })
    );
}

#[test]
fn code() {
    let a = PodAccount {
        balance: 0.into(),
        nonce: 0.into(),
        code: Some(vec![]),
        storage: map![],
        storage_dword: map![],
    };
    let b = PodAccount {
        balance: 0.into(),
        nonce: 1.into(),
        code: Some(vec![0]),
        storage: map![],
        storage_dword: map![],
    };
    assert_eq!(
        diff_pod(Some(&a), Some(&b)),
        Some(AccountDiff {
            balance: Diff::Same,
            nonce: Diff::Changed(0.into(), 1.into()),
            code: Diff::Changed(vec![], vec![0]),
            storage: map![],
            storage_dword: map![],
        })
    );
}

#[test]
fn storage() {
    let a = PodAccount {
        balance: 0.into(),
        nonce: 0.into(),
        code: Some(vec![]),
        storage: map_into![1 => 1, 2 => 2, 3 => 3, 4 => 4, 5 => 0, 6 => 0, 7 => 0],
        storage_dword: map_into![1 => 1, 2 => 2, 3 => 3, 4 => 4, 5 => 0, 6 => 0, 7 => 0],
    };
    let b = PodAccount {
        balance: 0.into(),
        nonce: 0.into(),
        code: Some(vec![]),
        storage: map_into![1 => 1, 2 => 3, 3 => 0, 5 => 0, 7 => 7, 8 => 0, 9 => 9],
        storage_dword: map_into![1 => 1, 2 => 3, 3 => 0, 5 => 0, 7 => 7, 8 => 0, 9 => 9],
    };
    assert_eq!(
        diff_pod(Some(&a), Some(&b)),
        Some(AccountDiff {
            balance: Diff::Same,
            nonce: Diff::Same,
            code: Diff::Same,
            storage: map![
                    2.into() => Diff::new(2.into(), 3.into()),
                    3.into() => Diff::new(3.into(), 0.into()),
                    4.into() => Diff::new(4.into(), 0.into()),
                    7.into() => Diff::new(0.into(), 7.into()),
                    9.into() => Diff::new(0.into(), 9.into())
                ],
            storage_dword: map![
                    2.into() => Diff::new(2.into(), 3.into()),
                    3.into() => Diff::new(3.into(), 0.into()),
                    4.into() => Diff::new(4.into(), 0.into()),
                    7.into() => Diff::new(0.into(), 7.into()),
                    9.into() => Diff::new(0.into(), 9.into())
                ],
        })
    );
}
