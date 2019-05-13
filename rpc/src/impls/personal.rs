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

//! Account management (personal) rpc implementation
use std::sync::Arc;
use bytes::ToPretty;
use acore::account_provider::AccountProvider;
use acore::transaction::PendingTransaction;
use aion_types::Address;
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_core::futures::Future;
use helpers::errors;
use helpers::dispatch::{self, Dispatcher, SignWith};
use helpers::accounts::unwrap_provider;
use traits::Personal;
use types::{
    H256 as RpcH256, H768 as RpcH768, Bytes as RpcBytes,
    ConfirmationPayload as RpcConfirmationPayload, ConfirmationResponse as RpcConfirmationResponse,
    TransactionRequest, RichRawTransaction as RpcRichRawTransaction,
};
use helpers::nonce::Ready as NonceReady;
/// Account management (personal) rpc implementation.
pub struct PersonalClient<D: Dispatcher> {
    accounts: Option<Arc<AccountProvider>>,
    dispatcher: D,
    allow_perm_unlock: bool,
}

impl<D: Dispatcher> PersonalClient<D> {
    /// Creates new PersonalClient
    pub fn new(
        accounts: Option<Arc<AccountProvider>>,
        dispatcher: D,
        allow_perm_unlock: bool,
    ) -> Self
    {
        PersonalClient {
            accounts,
            dispatcher,
            allow_perm_unlock,
        }
    }

    fn account_provider(&self) -> Result<Arc<AccountProvider>> { unwrap_provider(&self.accounts) }
}

impl<D: Dispatcher + 'static> PersonalClient<D> {
    fn do_sign_transaction(
        &self,
        request: TransactionRequest,
        password: String,
    ) -> BoxFuture<(PendingTransaction, D, Option<NonceReady>)>
    {
        let dispatcher = self.dispatcher.clone();
        let accounts = try_bf!(self.account_provider());
        Box::new(
            dispatcher
                .fill_optional_fields(request.into(), false)
                .and_then(move |filled| {
                    let condition = filled.condition.clone().map(Into::into);
                    dispatcher
                        .sign(accounts, filled, SignWith::Password(password))
                        .map(|(tx, nonce)| (tx.into_value(), nonce))
                        .map(move |(tx, nonce)| (PendingTransaction::new(tx, condition), nonce))
                        .map(move |(tx, nonce)| (tx, dispatcher, nonce))
                }),
        )
    }
}

impl<D: Dispatcher + 'static> Personal for PersonalClient<D> {
    fn accounts(&self) -> Result<Vec<RpcH256>> {
        let store = self.account_provider()?;
        let accounts = store
            .accounts()
            .map_err(|e| errors::account("Could not fetch accounts.", e))?;
        Ok(accounts
            .into_iter()
            .map(Into::into)
            .collect::<Vec<RpcH256>>())
    }

    fn new_account(&self, pass: String) -> Result<RpcH256> {
        let store = self.account_provider()?;

        store
            .new_account_ed25519(&pass)
            .map(Into::into)
            .map_err(|e| errors::account("Could not create account.", e))
    }

    fn unlock_account(
        &self,
        account: RpcH256,
        account_pass: String,
        duration: Option<u64>,
    ) -> Result<bool>
    {
        let account: Address = account.into();
        let store = self.account_provider()?;
        let r = match (self.allow_perm_unlock, duration) {
            (true, Some(0)) => store.unlock_account_permanently(account, account_pass),
            (_, Some(d)) => store.unlock_account_timed(account, account_pass, d * 1000),
            (_, None) => store.unlock_account_timed(account, account_pass, 300_000),
            // Temporarily unlock is for one time use (lock after once used). Disabled in official release to be align with Aion Java kernel.
            // (_, None) => store.unlock_account_temporarily(account, account_pass),
        };
        match r {
            Ok(_) => Ok(true),
            Err(err) => {
                error!(target:"account","Unable to unlock the account: {}", err);
                Ok(false)
            }
        }
    }

    fn lock_account(&self, account: RpcH256, account_pass: String) -> Result<bool> {
        let account: Address = account.into();
        let store = self.account_provider()?;
        let r = store.lock_account(account, account_pass);
        match r {
            Ok(_) => Ok(true),
            Err(err) => Err(errors::account("Unable to lock the account", err)),
        }
    }

    fn is_account_unlocked(&self, account: RpcH256) -> Result<bool> {
        let account: Address = account.into();
        let store = self.account_provider()?;
        Ok(store.is_unlocked_generic(&account))
    }

    fn sign(&self, data: RpcBytes, account: RpcH256, password: String) -> BoxFuture<RpcH768> {
        let dispatcher = self.dispatcher.clone();
        let accounts = try_bf!(self.account_provider());

        let payload = RpcConfirmationPayload::EthSignMessage((account.clone(), data).into());

        Box::new(
            dispatch::from_rpc(payload, &dispatcher)
                .and_then(|payload| {
                    dispatch::execute(
                        dispatcher,
                        accounts,
                        payload,
                        dispatch::SignWith::Password(password),
                    )
                })
                .map(|v| v.into_value())
                .then(|res| {
                    match res {
                        Ok(RpcConfirmationResponse::SignatureEd25519(signature)) => Ok(signature),
                        Err(e) => Err(e),
                        e => Err(errors::internal("Unexpected result", e)),
                    }
                }),
        )
    }

    fn sign_transaction(
        &self,
        request: TransactionRequest,
        password: String,
    ) -> BoxFuture<RpcRichRawTransaction>
    {
        Box::new(self.do_sign_transaction(request, password).map(
            |(pending_tx, dispatcher, nonce)| {
                nonce.map(move |nonce| nonce.mark_used());
                dispatcher.enrich(pending_tx.transaction)
            },
        ))
    }

    fn send_transaction(
        &self,
        request: TransactionRequest,
        password: String,
    ) -> BoxFuture<RpcH256>
    {
        Box::new(self.do_sign_transaction(request, password).and_then(
            |(pending_tx, dispatcher, nonce)| {
                let chain_id = pending_tx.chain_id();
                trace!(target: "miner", "send_transaction: dispatching tx: {} for chain ID {:?}",
                    ::rlp::encode(&*pending_tx).into_vec().pretty(), chain_id);

                dispatcher
                    .dispatch_transaction(pending_tx)
                    .map(move |res| {
                        nonce.map(move |nonce| nonce.mark_used());
                        res
                    })
                    .map(Into::into)
            },
        ))
    }

    fn sign_and_send_transaction(
        &self,
        request: TransactionRequest,
        password: String,
    ) -> BoxFuture<RpcH256>
    {
        warn!(
            target:"personal",
            "Using deprecated personal_signAndSendTransaction, use personal_sendTransaction \
             instead."
        );
        self.send_transaction(request, password)
    }
}
