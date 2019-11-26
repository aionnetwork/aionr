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

//! RPC types

mod stratum_header;
mod block;
mod block_number;
mod bytes;
mod call_request;
mod confirmations;
mod contract;
mod filter;
mod index;
mod log;
mod provenance;
mod receipt;
mod sync;
mod transaction;
mod transaction_request;
mod transaction_condition;
mod mining;

pub use self::bytes::Bytes;
pub use self::block::{Block, BlockTransactions, Header};
pub use self::block_number::BlockNumber;
pub use self::stratum_header::{SimpleHeader, StratumHeader};
pub use self::call_request::CallRequest;
pub use self::confirmations::{
    ConfirmationPayload, ConfirmationResponse, SignRequest,
};
pub use self::contract::{Contract, ContractInfo, Abi, AbiIO};
pub use self::filter::{Filter, FilterChanges};
pub use self::index::Index;
pub use self::log::Log;
pub use self::provenance::Origin;
pub use self::receipt::{Receipt};
pub use self::sync::{
SyncStatus, SyncInfo, /* Peers, PeerInfo, PeerNetworkInfo, TransactionStats, ChainStatus,
                      AcitvePeerInfo, PbSyncInfo,*/
};
pub use self::transaction::{Transaction, RichRawTransaction};
pub use self::transaction_request::TransactionRequest;
pub use self::transaction_condition::TransactionCondition;
pub use self::mining::{Work, Info, AddressValidation, MiningInfo, MinerStats};
