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
use types::state_diff::StateDiff;
use types::account_diff::{ AccountDiff, Diff };
use pod_account::{ PodAccount };
use pod_state::{ PodState, diff_pod };

#[test]
fn create_delete() {
    let a = PodState::from(map![
            1.into() => PodAccount {
                balance: 69.into(),
                nonce: 0.into(),
                code: Some(Vec::new()),
                storage: map![],
            }
        ]);
    assert_eq!(
        diff_pod(&a, &PodState::new()),
        StateDiff {
            raw: map![
                    1.into() => AccountDiff{
                        balance: Diff::Died(69.into()),
                        nonce: Diff::Died(0.into()),
                        code: Diff::Died(vec![]),
                        storage: map![],
                    }
                ],
        }
    );
    assert_eq!(
        diff_pod(&PodState::new(), &a),
        StateDiff {
            raw: map![
                    1.into() => AccountDiff{
                        balance: Diff::Born(69.into()),
                        nonce: Diff::Born(0.into()),
                        code: Diff::Born(vec![]),
                        storage: map![],
                    }
                ],
        }
    );
}

#[test]
fn create_delete_with_unchanged() {
    let a = PodState::from(map![
            1.into() => PodAccount {
                balance: 69.into(),
                nonce: 0.into(),
                code: Some(Vec::new()),
                storage: map![],
            }
        ]);
    let b = PodState::from(map![
            1.into() => PodAccount {
                balance: 69.into(),
                nonce: 0.into(),
                code: Some(Vec::new()),
                storage: map![],
            },
            2.into() => PodAccount {
                balance: 69.into(),
                nonce: 0.into(),
                code: Some(Vec::new()),
                storage: map![],
            }
        ]);
    assert_eq!(
        diff_pod(&a, &b),
        StateDiff {
            raw: map![
                    2.into() => AccountDiff{
                        balance: Diff::Born(69.into()),
                        nonce: Diff::Born(0.into()),
                        code: Diff::Born(vec![]),
                        storage: map![],
                    }
                ],
        }
    );
    assert_eq!(
        diff_pod(&b, &a),
        StateDiff {
            raw: map![
                    2.into() => AccountDiff{
                        balance: Diff::Died(69.into()),
                        nonce: Diff::Died(0.into()),
                        code: Diff::Died(vec![]),
                        storage: map![],
                    }
                ],
        }
    );
}

#[test]
fn change_with_unchanged() {
    let a = PodState::from(map![
            1.into() => PodAccount {
                balance: 69.into(),
                nonce: 0.into(),
                code: Some(Vec::new()),
                storage: map![],
            },
            2.into() => PodAccount {
                balance: 69.into(),
                nonce: 0.into(),
                code: Some(Vec::new()),
                storage: map![],
            }
        ]);
    let b = PodState::from(map![
            1.into() => PodAccount {
                balance: 69.into(),
                nonce: 1.into(),
                code: Some(Vec::new()),
                storage: map![],
            },
            2.into() => PodAccount {
                balance: 69.into(),
                nonce: 0.into(),
                code: Some(Vec::new()),
                storage: map![],
            }
        ]);
    assert_eq!(
        diff_pod(&a, &b),
        StateDiff {
            raw: map![
                    1.into() => AccountDiff{
                        balance: Diff::Same,
                        nonce: Diff::Changed(0.into(), 1.into()),
                        code: Diff::Same,
                        storage: map![],
                    }
                ],
        }
    );
}
