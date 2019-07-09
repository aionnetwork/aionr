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

pub mod call_analytics;
pub mod filter;
pub mod ids;
pub mod pruning_info;
pub mod verification_queue_info;

// pub mod block_info;
// pub mod block_extra_update;
// pub mod blockchain_best_block;
// pub mod blockchain_cache;
// pub mod blockchain_config;
// pub mod blockchain_extra;
// pub mod blockchain_import_route;

pub mod account;
pub mod block;
pub mod blockchain;
pub mod state;

#[cfg(test)]
mod test;

/// Type for block number.
pub type BlockNumber = u64;
