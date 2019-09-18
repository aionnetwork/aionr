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

//! Utilities and helpers for transaction dispatch.

use std::fmt::Debug;
use std::ops::Deref;
use std::sync::Arc;

use acore::account_provider::{AccountProvider, AccountToken};
use acore::client::MiningBlockChainClient;
use acore::miner::MinerService;
use acore::transaction::{Action, PendingTransaction, SignedTransaction, Transaction};
use aion_types::{Address, H256, H768, U256};
use blake2b::blake2b;
use bytes::Bytes;
use key::Ed25519Signature;
use parking_lot::Mutex;

use helpers::{errors, nonce, ConfirmationPayload, FilledTransactionRequest, TransactionRequest};
use jsonrpc_core::futures::{future, Async, Future, Poll};
use jsonrpc_core::{BoxFuture, Error, Result};
use types::{
    ConfirmationPayload as RpcConfirmationPayload, ConfirmationResponse,
    RichRawTransaction as RpcRichRawTransaction, SignRequest as RpcSignRequest,
};

pub use self::nonce::{Ready as NonceReady, Reservations};

use bytes::i64_to_bytes;
use trace_time::to_epoch_micro;
type SignResult = (WithToken<SignedTransaction>, Option<NonceReady>);
/// Has the capability to dispatch, sign, and decrypt.
///
/// Requires a clone implementation, with the implication that it be cheap;
/// usually just bumping a reference count or two.
pub trait Dispatcher: Send + Sync + Clone {
    // TODO: when ATC exist, use zero-cost
    // type Out<T>: IntoFuture<T, Error>

    /// Fill optional fields of a transaction request, fetching gas price but not nonce.
    fn fill_optional_fields(
        &self,
        request: TransactionRequest,
        force_nonce: bool,
    ) -> BoxFuture<FilledTransactionRequest>;

    /// Sign the given transaction request without dispatching, fetching appropriate nonce.
    fn sign(
        &self,
        accounts: Arc<AccountProvider>,
        filled: FilledTransactionRequest,
        password: SignWith,
    ) -> BoxFuture<SignResult>;

    /// Converts a `SignedTransaction` into `RichRawTransaction`
    fn enrich(&self, SignedTransaction) -> RpcRichRawTransaction;

    /// "Dispatch" a local transaction.
    fn dispatch_transaction(&self, signed_transaction: PendingTransaction) -> Result<H256>;
}

/// A dispatcher which uses references to a client and miner in order to sign
/// requests locally.
#[derive(Debug)]
pub struct FullDispatcher<C, M> {
    client: Arc<C>,
    miner: Arc<M>,
    nonces: Arc<Mutex<nonce::Reservations>>,
    dynamic_gas_price: Option<DynamicGasPrice>,
}

impl<C, M> FullDispatcher<C, M> {
    /// Create a `FullDispatcher` from Arc references to a client and miner.
    pub fn new(
        client: Arc<C>,
        miner: Arc<M>,
        nonces: Arc<Mutex<nonce::Reservations>>,
        dynamic_gas_price: Option<DynamicGasPrice>,
    ) -> Self
    {
        FullDispatcher {
            client,
            miner,
            nonces,
            dynamic_gas_price,
        }
    }
}

impl<C, M> Clone for FullDispatcher<C, M> {
    fn clone(&self) -> Self {
        FullDispatcher {
            client: self.client.clone(),
            miner: self.miner.clone(),
            nonces: self.nonces.clone(),
            dynamic_gas_price: self.dynamic_gas_price.clone(),
        }
    }
}

impl<C: MiningBlockChainClient, M: MinerService> FullDispatcher<C, M> {
    fn state_nonce(&self, from: &Address) -> U256 {
        self.miner
            .last_nonce(from)
            .map(|nonce| nonce + U256::one())
            .unwrap_or_else(|| self.client.latest_nonce(from))
    }

    /// Imports transaction to the miner's queue.
    pub fn dispatch_transaction(
        client: &C,
        miner: &M,
        signed_transaction: PendingTransaction,
    ) -> Result<H256>
    {
        let hash = signed_transaction.transaction.hash().clone();

        miner
            .import_own_transaction(client, signed_transaction)
            .map_err(errors::transaction)
            .map(|_| hash)
    }
}

impl<C: MiningBlockChainClient, M: MinerService> Dispatcher for FullDispatcher<C, M> {
    fn fill_optional_fields(
        &self,
        request: TransactionRequest,
        force_nonce: bool,
    ) -> BoxFuture<FilledTransactionRequest>
    {
        let request = request;
        let from = request.from.unwrap_or(Address::from(0));
        let nonce = if force_nonce {
            request.nonce.or_else(|| Some(self.state_nonce(&from)))
        } else {
            request.nonce
        };

        Box::new(future::ok(FilledTransactionRequest {
            from,
            to: request.to,
            nonce,
            gas_price: request.gas_price.unwrap_or_else(|| {
                default_gas_price(&*self.client, &*self.miner, self.dynamic_gas_price.clone())
            }),
            gas: request
                .gas
                .unwrap_or_else(|| self.miner.default_gas_limit()),
            value: request.value.unwrap_or_else(|| 0.into()),
            data: request.data.unwrap_or_else(Vec::new),
            tx_type: request.tx_type.unwrap_or_else(|| 0x01.into()),
            condition: request.condition,
        }))
    }

    fn sign(
        &self,
        accounts: Arc<AccountProvider>,
        filled: FilledTransactionRequest,
        password: SignWith,
    ) -> BoxFuture<SignResult>
    {
        if let Some(nonce) = filled.nonce {
            return Box::new(future::done(sign_transaction(
                &*accounts, filled, nonce, password,
            )));
        }

        let state_nonce = self.state_nonce(&filled.from);
        let reserved = self.nonces.lock().reserve(filled.from, state_nonce);

        Box::new(ProspectiveSigner::new(accounts, filled, reserved, password))
    }

    fn enrich(&self, signed_transaction: SignedTransaction) -> RpcRichRawTransaction {
        RpcRichRawTransaction::from_signed(signed_transaction)
    }

    fn dispatch_transaction(&self, signed_transaction: PendingTransaction) -> Result<H256> {
        Self::dispatch_transaction(&*self.client, &*self.miner, signed_transaction)
    }
}

/// Returns a eth_sign-compatible hash of data to sign.
/// The data is prepended with special message to prevent
/// chosen-plaintext attacks.
pub fn eth_data_hash(mut data: Bytes) -> H256 {
    let mut message_data = format!("\x19Ethereum Signed Message:\n{}", data.len()).into_bytes();
    message_data.append(&mut data);
    blake2b(message_data)
}

fn sign_transaction(
    accounts: &AccountProvider,
    filled: FilledTransactionRequest,
    nonce: U256,
    password: SignWith,
) -> Result<SignResult>
{
    let t = Transaction::new(
        nonce,
        filled.gas_price,
        filled.gas,
        filled.to.map_or(Action::Create, Action::Call),
        filled.value,
        filled.data,
        filled.tx_type,
    );

    let timestamp = i64_to_bytes(to_epoch_micro());
    let hash = t.hash(&timestamp);
    let signature = signature(accounts, filled.from, hash, password)?;

    Ok((
        signature.map(|sig| {
            SignedTransaction::new(t.with_signature(sig, timestamp.to_vec())).expect(
                "Transaction was signed by AccountsProvider; it never produces invalid \
                 signatures; qed",
            )
        }),
        None,
    ))
}

#[derive(Debug, Clone, Copy)]
enum ProspectiveSignerState {
    TryProspectiveSign,
    WaitForNonce,
    Finish,
}

struct ProspectiveSigner {
    accounts: Arc<AccountProvider>,
    filled: FilledTransactionRequest,
    reserved: nonce::Reserved,
    password: SignWith,
    state: ProspectiveSignerState,
    prospective: Option<Result<WithToken<SignedTransaction>>>,
    ready: Option<nonce::Ready>,
}

impl ProspectiveSigner {
    pub fn new(
        accounts: Arc<AccountProvider>,
        filled: FilledTransactionRequest,
        reserved: nonce::Reserved,
        password: SignWith,
    ) -> Self
    {
        // If the account is permanently unlocked we can try to sign
        // using prospective nonce. This should speed up sending
        // multiple subsequent transactions in multi-threaded RPC environment.
        let is_unlocked_permanently = accounts.is_unlocked_permanently(&filled.from);
        let has_password = password.is_password();

        ProspectiveSigner {
            accounts,
            filled,
            reserved,
            password,
            state: if is_unlocked_permanently || has_password {
                ProspectiveSignerState::TryProspectiveSign
            } else {
                ProspectiveSignerState::WaitForNonce
            },
            prospective: None,
            ready: None,
        }
    }

    fn sign(&self, nonce: &U256) -> Result<WithToken<SignedTransaction>> {
        sign_transaction(
            &*self.accounts,
            self.filled.clone(),
            *nonce,
            self.password.clone(),
        )
        .map(move |(res, _)| res)
    }

    fn poll_reserved(&mut self) -> Poll<nonce::Ready, Error> {
        self.reserved
            .poll()
            .map_err(|_| errors::internal("Nonce reservation failure", ""))
    }
}

impl Future for ProspectiveSigner {
    type Item = SignResult;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        use self::ProspectiveSignerState::*;

        loop {
            match self.state {
                TryProspectiveSign => {
                    // Try to poll reserved, it might be ready.
                    match self.poll_reserved()? {
                        Async::NotReady => {
                            self.state = WaitForNonce;
                            self.prospective = Some(self.sign(self.reserved.prospective_value()));
                        }
                        Async::Ready(nonce) => {
                            self.state = Finish;
                            self.prospective = Some(self.sign(nonce.value()));
                            self.ready = Some(nonce);
                        }
                    }
                }
                WaitForNonce => {
                    let nonce = try_ready!(self.poll_reserved());
                    let result = match (self.prospective.take(), nonce.matches_prospective()) {
                        (Some(prospective), true) => prospective,
                        _ => self.sign(nonce.value()),
                    };
                    self.state = Finish;
                    self.prospective = Some(result);
                    self.ready = Some(nonce);
                }
                Finish => {
                    if let (Some(result), Some(nonce)) =
                        (self.prospective.take(), self.ready.take())
                    {
                        // Mark nonce as used on successful signing
                        return result.map(move |tx| Async::Ready((tx, Some(nonce))));
                    } else {
                        panic!("Poll after ready.");
                    }
                }
            }
        }
    }
}

/// Values used to unlock accounts for signing.
#[derive(Debug, Clone, PartialEq)]
pub enum SignWith {
    /// Nothing -- implies the account is already unlocked.
    Nothing,
    /// Unlock with password.
    Password(String),
    /// Unlock with single-use token.
    Token(AccountToken),
}

impl SignWith {
    fn is_password(&self) -> bool {
        if let SignWith::Password(_) = *self {
            true
        } else {
            false
        }
    }
}

/// A value, potentially accompanied by a signing token.
#[derive(Debug)]
pub enum WithToken<T: Debug> {
    /// No token.
    No(T),
    /// With token.
    Yes(T, AccountToken),
}

impl<T: Debug> Deref for WithToken<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match *self {
            WithToken::No(ref v) => v,
            WithToken::Yes(ref v, _) => v,
        }
    }
}

impl<T: Debug> WithToken<T> {
    /// Map the value with the given closure, preserving the token.
    pub fn map<S, F>(self, f: F) -> WithToken<S>
    where
        S: Debug,
        F: FnOnce(T) -> S,
    {
        match self {
            WithToken::No(v) => WithToken::No(f(v)),
            WithToken::Yes(v, token) => WithToken::Yes(f(v), token),
        }
    }

    /// Convert into inner value, ignoring possible token.
    pub fn into_value(self) -> T {
        match self {
            WithToken::No(v) => v,
            WithToken::Yes(v, _) => v,
        }
    }

    /// Convert the `WithToken` into a tuple.
    pub fn into_tuple(self) -> (T, Option<AccountToken>) {
        match self {
            WithToken::No(v) => (v, None),
            WithToken::Yes(v, token) => (v, Some(token)),
        }
    }
}

impl<T: Debug> From<(T, AccountToken)> for WithToken<T> {
    fn from(tuple: (T, AccountToken)) -> Self { WithToken::Yes(tuple.0, tuple.1) }
}

impl<T: Debug> From<(T, Option<AccountToken>)> for WithToken<T> {
    fn from(tuple: (T, Option<AccountToken>)) -> Self {
        match tuple.1 {
            Some(token) => WithToken::Yes(tuple.0, token),
            None => WithToken::No(tuple.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicGasPrice {
    pub blk_price_window: usize,
    pub max_blk_traverse: usize,
    pub gas_price_percentile: usize,
    pub last_processed: i64,
    pub recommendation: U256,
}

impl Default for DynamicGasPrice {
    fn default() -> DynamicGasPrice {
        DynamicGasPrice {
            blk_price_window: 20,
            max_blk_traverse: 64,
            gas_price_percentile: 60,
            last_processed: -1,
            recommendation: U256::from(10000000000u64),
        }
    }
}

/// Execute a confirmation payload.
pub fn execute<D: Dispatcher + 'static>(
    dispatcher: D,
    accounts: Arc<AccountProvider>,
    payload: ConfirmationPayload,
    pass: SignWith,
) -> BoxFuture<WithToken<ConfirmationResponse>>
{
    match payload {
        ConfirmationPayload::SendTransaction(request) => {
            let condition = request.condition.clone().map(Into::into);
            Box::new(
                dispatcher
                    .sign(accounts, request, pass)
                    .map(move |(v, nonce)| {
                        (
                            v.map(move |tx| PendingTransaction::new(tx, condition)),
                            nonce,
                        )
                    })
                    .map(move |(v, nonce)| {
                        let v = v.into_tuple();
                        (v, nonce)
                    })
                    .map(|((tx, token), nonce)| (tx, token, nonce, dispatcher))
                    .and_then(|(tx, tok, nonce, dispatcher)| {
                        dispatcher
                            .dispatch_transaction(tx)
                            .map(ConfirmationResponse::SendTransaction)
                            .map(move |h| {
                                nonce.map(move |nonce| nonce.mark_used());
                                WithToken::from((h, tok))
                            })
                    }),
            )
        }
        ConfirmationPayload::SignTransaction(request) => {
            Box::new(
                dispatcher
                    .sign(accounts, request, pass)
                    .map(move |(v, nonce)| {
                        nonce.map(move |nonce| nonce.mark_used());
                        v.map(move |tx| dispatcher.enrich(tx))
                            .map(ConfirmationResponse::SignTransaction)
                    }),
            )
        }
        ConfirmationPayload::EthSignMessage(address, data) => {
            let hash = eth_data_hash(data);

            let res = signature(&accounts, address, hash, pass).map(|result| {
                result
                    //.map(|rsv| H768::from(rsv.into()))
                    .map(|rsv| H768(rsv.into()))
                    .map(ConfirmationResponse::SignatureEd25519)
            });
            Box::new(future::done(res))
        }
    }
}

fn signature(
    accounts: &AccountProvider,
    address: Address,
    hash: H256,
    password: SignWith,
) -> Result<WithToken<Ed25519Signature>>
{
    match password.clone() {
        SignWith::Nothing => accounts.sign(address, None, hash).map(WithToken::No),
        SignWith::Password(pass) => accounts.sign(address, Some(pass), hash).map(WithToken::No),
        SignWith::Token(token) => {
            accounts
                .sign_with_token(address, token, hash)
                .map(Into::into)
        }
    }
    .map_err(|e| {
        match password {
            SignWith::Nothing => errors::signing(e),
            _ => errors::password(e),
        }
    })
}

/// Extract the default gas price from a client and miner.
pub fn default_gas_price<C, M>(
    client: &C,
    miner: &M,
    dynamic_gas_price: Option<DynamicGasPrice>,
) -> U256
where
    C: MiningBlockChainClient,
    M: MinerService,
{
    match dynamic_gas_price {
        None => {
            return miner.minimal_gas_price();
        }
        Some(mut dynamic_gas_price) => {
            let blk_now = client.chain_info().best_block_number as i64;
            if blk_now - dynamic_gas_price.last_processed >= 2 {
                dynamic_gas_price.recommendation = client
                    .gas_price_corpus(
                        dynamic_gas_price.blk_price_window,
                        dynamic_gas_price.max_blk_traverse,
                    )
                    .percentile(dynamic_gas_price.gas_price_percentile)
                    .cloned()
                    .map_or(miner.minimal_gas_price(), |gas_price| {
                        ::std::cmp::min(gas_price, miner.local_maximal_gas_price())
                    });
                dynamic_gas_price.last_processed = blk_now;
            }
            return dynamic_gas_price.recommendation;
        }
    }
}

/// Convert RPC confirmation payload to signer confirmation payload.
/// May need to resolve in the future to fetch things like gas price.
pub fn from_rpc<D>(
    payload: RpcConfirmationPayload,
    dispatcher: &D,
) -> BoxFuture<ConfirmationPayload>
where
    D: Dispatcher,
{
    match payload {
        RpcConfirmationPayload::SendTransaction(request) => {
            Box::new(
                dispatcher
                    .fill_optional_fields(request.into(), false)
                    .map(ConfirmationPayload::SendTransaction),
            )
        }
        RpcConfirmationPayload::SignTransaction(request) => {
            Box::new(
                dispatcher
                    .fill_optional_fields(request.into(), false)
                    .map(ConfirmationPayload::SignTransaction),
            )
        }
        RpcConfirmationPayload::EthSignMessage(RpcSignRequest {
            address,
            data,
        }) => {
            Box::new(future::ok(ConfirmationPayload::EthSignMessage(
                address.into(),
                data.into(),
            )))
        }
    }
}
