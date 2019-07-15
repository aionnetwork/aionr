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

//! Execution environment substate.
use std::collections::HashSet;
use aion_types::{U256, Address};
use log_entry::LogEntry;
use super::CleanupMode;

/// State changes which should be applied in finalize,
/// after transaction is fully executed.
#[derive(Debug, Default, Clone)]
pub struct Substate {
    /// Any accounts that have suicided.
    pub suicides: HashSet<Address>,

    /// Any accounts that are touched.
    // This collection is not in use now. It was used to mark touched addresses to
    // be filtered quickly when cleaning garbage.
    pub touched: HashSet<Address>,

    /// Any logs.
    pub logs: Vec<LogEntry>,

    /// Refund counter of SSTORE nonzero -> zero.
    pub sstore_clears_count: U256,

    /// Created contracts.
    pub contracts_created: Vec<Address>,
}

impl Substate {
    /// Creates new substate.
    pub fn new() -> Self { Substate::default() }

    /// Merge secondary substate `s` into self, accruing each element correspondingly.
    pub fn accrue(&mut self, s: Substate) {
        self.suicides.extend(s.suicides);
        self.touched.extend(s.touched);
        self.logs.extend(s.logs);
        self.sstore_clears_count = self.sstore_clears_count + s.sstore_clears_count;
        self.contracts_created.extend(s.contracts_created);
    }

    /// Get the cleanup mode object from this.
    pub fn to_cleanup_mode(&mut self) -> CleanupMode {
        CleanupMode::TrackTouched(&mut self.touched)
    }
}
