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

use std::sync::{Arc, Weak};
use jsonrpc_core::{Result, Error};
use jsonrpc_core::futures::{self, Future, IntoFuture};
use jsonrpc_macros::Trailing;
use jsonrpc_macros::pubsub::{Sink, Subscriber};
use jsonrpc_pubsub::SubscriptionId;
use helpers::{errors, limit_logs, Subscribers};
use metadata::Metadata;
use traits::EthPubSub;
use types::{pubsub, Log};
use acore::filter::Filter as EthFilter;
use acore::client::{BlockChainClient, ChainNotify, BlockId};
use tokio::runtime::TaskExecutor;
use aion_types::H256;
use bytes::Bytes;
use parking_lot::RwLock;

type Client = Sink<pubsub::Result>;

pub struct EthPubSubClient<C> {
    handler: Arc<ChainNotificationHandler<C>>,
    logs_subscribers: Arc<RwLock<Subscribers<(Client, EthFilter)>>>,
    transactions_subscribers: Arc<RwLock<Subscribers<Client>>>,
}

impl<C> EthPubSubClient<C> {
    pub fn new(client: Arc<C>, executor: TaskExecutor) -> Self {
        let logs_subscribers = Arc::new(RwLock::new(Subscribers::default()));
        let transactions_subscribers = Arc::new(RwLock::new(Subscribers::default()));

        EthPubSubClient {
            handler: Arc::new(ChainNotificationHandler {
                client,
                executor,
                logs_subscribers: logs_subscribers.clone(),
                transactions_subscribers: transactions_subscribers.clone(),
            }),
            logs_subscribers,
            transactions_subscribers,
        }
    }

    /// Returns a chain notification handler.
    pub fn handler(&self) -> Weak<ChainNotificationHandler<C>> { Arc::downgrade(&self.handler) }
}

/// PubSub Notification handler.
pub struct ChainNotificationHandler<C> {
    client: Arc<C>,
    executor: TaskExecutor,
    logs_subscribers: Arc<RwLock<Subscribers<(Client, EthFilter)>>>,
    transactions_subscribers: Arc<RwLock<Subscribers<Client>>>,
}

impl<C> ChainNotificationHandler<C> {
    fn notify(executor: &TaskExecutor, subscriber: &Client, result: pubsub::Result) {
        executor.spawn(
            subscriber
                .notify(Ok(result))
                .map(|_| ())
                .map_err(|e| warn!(target: "rpc", "Unable to send notification: {}", e)),
        );
    }

    fn notify_logs<F, T>(&self, enacted: &[H256], logs: F)
    where
        F: Fn(EthFilter) -> T,
        T: IntoFuture<Item = Vec<Log>, Error = Error>,
        T::Future: Send + 'static,
    {
        for &(ref subscriber, ref filter) in self.logs_subscribers.read().values() {
            let logs = futures::future::join_all(
                enacted
                    .iter()
                    .map(|hash| {
                        let mut filter = filter.clone();
                        filter.from_block = BlockId::Hash(*hash);
                        filter.to_block = filter.from_block.clone();
                        logs(filter).into_future()
                    })
                    .collect::<Vec<_>>(),
            );
            let limit = filter.limit;
            let executor = self.executor.clone();
            let subscriber = subscriber.clone();
            self.executor.spawn(
                logs.map(move |logs| {
                    let logs = logs.into_iter().flat_map(|log| log).collect();

                    for log in limit_logs(logs, limit) {
                        Self::notify(&executor, &subscriber, pubsub::Result::Log(log))
                    }
                })
                .map_err(|e| warn!(target:"rpc","Unable to fetch latest logs: {:?}", e)),
            );
        }
    }

    /// Notify all subscribers about new transaction hashes.
    pub fn new_transactions(&self, hashes: &[H256]) {
        for subscriber in self.transactions_subscribers.read().values() {
            for hash in hashes {
                Self::notify(
                    &self.executor,
                    subscriber,
                    pubsub::Result::TransactionHash((*hash).into()),
                );
            }
        }
    }
}

impl<C: BlockChainClient> ChainNotify for ChainNotificationHandler<C> {
    fn new_blocks(
        &self,
        _imported: Vec<H256>,
        _invalid: Vec<H256>,
        enacted: Vec<H256>,
        retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _proposed: Vec<Bytes>,
        _duration: u64,
    )
    {
        // Enacted logs
        self.notify_logs(&enacted, |filter| {
            Ok(self
                .client
                .logs(filter)
                .into_iter()
                .map(Into::into)
                .collect())
        });

        // Retracted logs
        self.notify_logs(&retracted, |filter| {
            Ok(self
                .client
                .logs(filter)
                .into_iter()
                .map(Into::into)
                .map(|mut log: Log| {
                    log.log_type = "removed".into();
                    log
                })
                .collect())
        });
    }
}

impl<C: Send + Sync + 'static> EthPubSub for EthPubSubClient<C> {
    type Metadata = Metadata;

    fn subscribe(
        &self,
        _meta: Metadata,
        subscriber: Subscriber<pubsub::Result>,
        kind: pubsub::Kind,
        params: Trailing<pubsub::Params>,
    )
    {
        let error = match (kind, params.into()) {
            (pubsub::Kind::NewHeads, _) => {
                errors::invalid_params("newHeads", "Expected no parameters.")
            }
            (pubsub::Kind::Logs, Some(pubsub::Params::Logs(filter))) => {
                self.logs_subscribers
                    .write()
                    .push(subscriber, filter.into());
                return;
            }
            (pubsub::Kind::Logs, _) => errors::invalid_params("logs", "Expected a filter object."),
            (pubsub::Kind::NewPendingTransactions, None) => {
                self.transactions_subscribers.write().push(subscriber);
                return;
            }
            (pubsub::Kind::NewPendingTransactions, _) => {
                errors::invalid_params("newPendingTransactions", "Expected no parameters.")
            }
            _ => errors::unimplemented(None),
        };

        let _ = subscriber.reject(error);
    }

    fn unsubscribe(&self, id: SubscriptionId) -> Result<bool> {
        let res_0 = self.logs_subscribers.write().remove(&id).is_some();
        let res_1 = self.transactions_subscribers.write().remove(&id).is_some();
        Ok(res_0 || res_1)
    }
}
