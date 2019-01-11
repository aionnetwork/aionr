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

//! Signing RPC implementation.

use std::sync::Arc;

use acore::account_provider::AccountProvider;

use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_core::futures::Future;
use helpers::errors;
use helpers::dispatch::{self, Dispatcher};
use helpers::accounts::unwrap_provider;
use traits::{EthSigning};
use types::{
    H256 as RpcH256, H768 as RpcH768, Bytes as RpcBytes,
    RichRawTransaction as RpcRichRawTransaction,
    TransactionRequest as RpcTransactionRequest,
    ConfirmationPayload as RpcConfirmationPayload,
    ConfirmationResponse as RpcConfirmationResponse,
};

/// Implementation of functions that require signing when no trusted signer is used.
pub struct SigningClient<D> {
    accounts: Option<Arc<AccountProvider>>,
    dispatcher: D,
}

impl<D: Dispatcher + 'static> SigningClient<D> {
    /// Creates new SigningClient.
    pub fn new(accounts: &Option<Arc<AccountProvider>>, dispatcher: D) -> Self {
        SigningClient {
            accounts: accounts.clone(),
            dispatcher: dispatcher,
        }
    }

    fn account_provider(&self) -> Result<Arc<AccountProvider>> { unwrap_provider(&self.accounts) }

    fn handle(&self, payload: RpcConfirmationPayload) -> BoxFuture<RpcConfirmationResponse> {
        let accounts = try_bf!(self.account_provider());
        let dis = self.dispatcher.clone();
        Box::new(
            dispatch::from_rpc(payload, &dis)
                .and_then(move |payload| {
                    dispatch::execute(dis, accounts, payload, dispatch::SignWith::Nothing)
                })
                .map(|v| v.into_value()),
        )
    }
}

impl<D: Dispatcher + 'static> EthSigning for SigningClient<D> {
    fn sign(&self, address: RpcH256, data: RpcBytes) -> BoxFuture<RpcH768> {
        Box::new(
            self.handle(RpcConfirmationPayload::EthSignMessage(
                (address.clone(), data).into(),
            ))
            .then(|res| {
                match res {
                    Ok(RpcConfirmationResponse::SignatureEd25519(signature)) => Ok(signature),
                    Err(e) => Err(e),
                    e => Err(errors::internal("Unexpected result", e)),
                }
            }),
        )
    }

    fn send_transaction(&self, request: RpcTransactionRequest) -> BoxFuture<RpcH256> {
        Box::new(
            self.handle(RpcConfirmationPayload::SendTransaction(request))
                .then(|res| {
                    match res {
                        Ok(RpcConfirmationResponse::SendTransaction(hash)) => Ok(hash),
                        Err(e) => Err(e),
                        e => Err(errors::internal("Unexpected result", e)),
                    }
                }),
        )
    }

    fn sign_transaction(&self, request: RpcTransactionRequest) -> BoxFuture<RpcRichRawTransaction> {
        Box::new(
            self.handle(RpcConfirmationPayload::SignTransaction(request))
                .then(|res| {
                    match res {
                        Ok(RpcConfirmationResponse::SignTransaction(tx)) => Ok(tx),
                        Err(e) => Err(e),
                        e => Err(errors::internal("Unexpected result", e)),
                    }
                }),
        )
    }
}
