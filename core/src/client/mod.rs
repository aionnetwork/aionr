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

//! Blockchain database client.

mod ancient_import;
mod config;
mod error;
mod test_client;
mod client;

pub use self::client::*;
pub use self::config::{ClientConfig, DatabaseCompactionProfile, BlockChainConfig, VMType};
pub use self::error::Error;
pub use self::test_client::{TestBlockChainClient, EachBlockWith};
pub use self::chain_notify::ChainNotify;
pub use self::traits::{BlockChainClient, MiningBlockChainClient, EngineClient};

pub use types::ids::*;
pub use types::trace_filter::Filter as TraceFilter;
pub use types::pruning_info::PruningInfo;
pub use types::call_analytics::CallAnalytics;

pub use executive::{Executed, Executive};
pub use vms::{EnvInfo, LastHashes};

pub use error::{BlockImportError, TransactionImportError};
pub use verification::VerifierType;

mod traits;

mod chain_notify;
