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

//! Helper type with all filter state data.

use std::collections::HashSet;
use aion_types::H256;
use types::{Filter, Log};

pub type BlockNumber = u64;

/// default logs limit.
const DEFAULT_LIMIT: usize = 1000;

/// Filter state.
#[derive(Clone)]
pub enum PollFilter {
    /// Number of last block which client was notified about.
    Block(BlockNumber),
    /// Hashes of all transactions which client was notified about.
    PendingTransaction(Vec<H256>),
    /// Number of From block number, pending logs and log filter itself.
    Logs(BlockNumber, HashSet<Log>, Filter),
}

/// Returns only last `n` logs
pub fn limit_logs(mut logs: Vec<Log>, limit: Option<usize>) -> Vec<Log> {
    let len = logs.len();
    let limit_to_apply: usize = limit.unwrap_or(DEFAULT_LIMIT);

    if len >= limit_to_apply {
        logs.split_off(len - limit_to_apply)
    } else {
        logs
    }
}
