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

use std::collections::BTreeMap;
use types::account::account_diff::{ AccountDiff, Diff };
use pod_account::{PodAccount, diff_pod};

#[test]
fn existence() {
    let a = PodAccount {
        balance: 69.into(),
        nonce: 0.into(),
        code: Some(vec![]),
        storage: map![],
    };
    assert_eq!(diff_pod(Some(&a), Some(&a)), None);
    assert_eq!(
        diff_pod(None, Some(&a)),
        Some(AccountDiff {
            balance: Diff::Born(69.into()),
            nonce: Diff::Born(0.into()),
            code: Diff::Born(vec![]),
            storage: map![],
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
    };
    let b = PodAccount {
        balance: 42.into(),
        nonce: 1.into(),
        code: Some(vec![]),
        storage: map![],
    };
    assert_eq!(
        diff_pod(Some(&a), Some(&b)),
        Some(AccountDiff {
            balance: Diff::Changed(69.into(), 42.into()),
            nonce: Diff::Changed(0.into(), 1.into()),
            code: Diff::Same,
            storage: map![],
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
    };
    let b = PodAccount {
        balance: 0.into(),
        nonce: 1.into(),
        code: Some(vec![0]),
        storage: map![],
    };
    assert_eq!(
        diff_pod(Some(&a), Some(&b)),
        Some(AccountDiff {
            balance: Diff::Same,
            nonce: Diff::Changed(0.into(), 1.into()),
            code: Diff::Changed(vec![], vec![0]),
            storage: map![],
        })
    );
}

#[test]
fn storage() {
    let vec:Vec<Vec<u8>>=vec![vec![],vec![1],vec![2],vec![3],vec![4],vec![5],vec![6],vec![7],vec![8],vec![9]];
    let a = PodAccount {
        balance: 0.into(),
        nonce: 0.into(),
        code: Some(vec![]),
        storage: map_into![vec[1].clone() => vec[1].clone(), vec[2].clone() => vec[2].clone(), vec[3].clone() => vec[3].clone(), vec[4].clone() => vec[4].clone(), vec[5].clone() => vec[0].clone(), vec[6].clone() => vec[0].clone(), vec[7].clone() => vec[0].clone()],
    };
    let b = PodAccount {
        balance: 0.into(),
        nonce: 0.into(),
        code: Some(vec![]),
        storage: map_into![vec[1].clone() => vec[1].clone(), vec[2].clone() => vec[3].clone(), vec[3].clone() => vec[0].clone(), vec[5].clone() => vec[0].clone(), vec[7].clone() => vec[7].clone(), vec[8].clone() => vec[0].clone(), vec[9].clone() => vec[9].clone()],
    };
    assert_eq!(
        diff_pod(Some(&a), Some(&b)),
        Some(AccountDiff {
            balance: Diff::Same,
            nonce: Diff::Same,
            code: Diff::Same,
            storage: map![
                    vec[2].clone() => Diff::new(vec[2].clone(), vec[3].clone()),
                    vec[3].clone() => Diff::new(vec[3].clone(), vec[0].clone()),
                    vec[4].clone() => Diff::new(vec[4].clone(), vec[0].clone()),
                    vec[7].clone() => Diff::new(vec[0].clone(), vec[7].clone()),
                    vec[9].clone() => Diff::new(vec[0].clone(), vec[9].clone())
                ],
        })
    );
}
