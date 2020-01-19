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

//! Eth rpc interface.

use jsonrpc_core::BoxFuture;
use aion_types::{H256, H768, Address};
use jsonrpc_derive::rpc;

use crate::types::{Bytes as RpcBytes, TransactionRequest, RichRawTransaction};

/// Signing methods implementation relying on unlocked accounts.
#[rpc(server)]
pub trait EthSigning {
    type Metadata;

    /// Signs the hash of data with given address signature.
    #[rpc(name = "eth_sign")]
    fn sign(&self, address: Address, pass: RpcBytes) -> BoxFuture<H768>;

    /// Sends transaction; will block waiting for signer to return the
    /// transaction hash.
    /// If Signer is disable it will require the account to be unlocked.
    #[rpc(name = "eth_sendTransaction")]
    fn send_transaction(&self, request: TransactionRequest) -> BoxFuture<H256>;

    /// Signs transactions without dispatching it to the network.
    /// Returns signed transaction RLP representation and the transaction itself.
    /// It can be later submitted using `eth_sendRawTransaction/eth_submitTransaction`.
    #[rpc(name = "eth_signTransaction")]
    fn sign_transaction(&self, request: TransactionRequest) -> BoxFuture<RichRawTransaction>;
}
