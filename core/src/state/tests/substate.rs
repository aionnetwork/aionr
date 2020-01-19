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
use crate::state::substate::Substate;
use crate::log_entry::LogEntry;

#[test]
fn created() {
    let sub_state = Substate::new();
    assert_eq!(sub_state.suicides.len(), 0);
}

#[test]
fn accrue() {
    let mut sub_state = Substate::new();
    sub_state.contracts_created.push(1u64.into());
    sub_state.logs.push(LogEntry {
        address: 1u64.into(),
        topics: vec![],
        data: vec![],
    });
    sub_state.sstore_clears_count = 5.into();
    sub_state.suicides.insert(10u64.into());

    let mut sub_state_2 = Substate::new();
    sub_state_2.contracts_created.push(2u64.into());
    sub_state_2.logs.push(LogEntry {
        address: 1u64.into(),
        topics: vec![],
        data: vec![],
    });
    sub_state_2.sstore_clears_count = 7.into();

    sub_state.accrue(sub_state_2);
    assert_eq!(sub_state.contracts_created.len(), 2);
    assert_eq!(sub_state.sstore_clears_count, 12.into());
    assert_eq!(sub_state.suicides.len(), 1);
}
