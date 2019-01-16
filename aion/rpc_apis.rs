/*******************************************************************************
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

use std::cmp::PartialEq;
use std::collections::{BTreeMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;

use sync::sync::{SyncProvider};
use acore::account_provider::AccountProvider;
use acore::client::Client;
use acore::miner::Miner;
use jsonrpc_core::{self as core, MetaIoHandler};
use acore::miner::external::ExternalMiner;
use aion_rpc::dispatch::{FullDispatcher,DynamicGasPrice};
use aion_rpc::informant::{ActivityNotifier, ClientNotifier};
use aion_rpc::{Metadata};
use parking_lot::Mutex;
use tokio::runtime::TaskExecutor;

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum Api {
    /// Web3 (Safe)
    Web3,
    /// Net (Safe)
    Net,
    /// Eth (Safe)
    Eth,
    /// Eth (Safe)
    Stratum,
    /// Eth Pub-Sub (Safe)
    EthPubSub,
    /// "personal" api (All)
    Personal,
    /// Rpc (Safe)
    Rpc,
    /// Ping (Safe)
    Ping,
}

impl FromStr for Api {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::Api::*;

        match s {
            "web3" => Ok(Web3),
            "net" => Ok(Net),
            "eth" => Ok(Eth),
            "stratum" => Ok(Stratum),
            "pubsub" => Ok(EthPubSub),
            "personal" => Ok(Personal),
            "rpc" => Ok(Rpc),
            "ping" => Ok(Ping),
            api => Err(format!("Unknown api: {}", api)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ApiSet {
    // Public context (like public jsonrpc over http)
    PublicContext,
    // All possible APIs
    All,
    // Local "unsafe" context and accounts access
    IpcContext,
    // Fixed list of APis
    List(HashSet<Api>),
}

impl Default for ApiSet {
    fn default() -> Self { ApiSet::PublicContext }
}

impl PartialEq for ApiSet {
    fn eq(&self, other: &Self) -> bool { self.list_apis() == other.list_apis() }
}

impl FromStr for ApiSet {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut apis = HashSet::new();

        for api in s.split(',') {
            match api {
                "all" => {
                    apis.extend(ApiSet::All.list_apis());
                }
                // Remove the API
                api if api.starts_with("-") => {
                    let api = api[1..].parse()?;
                    apis.remove(&api);
                }
                api => {
                    let api = api.parse()?;
                    apis.insert(api);
                }
            }
        }

        Ok(ApiSet::List(apis))
    }
}

fn to_modules(apis: &HashSet<Api>) -> BTreeMap<String, String> {
    let mut modules = BTreeMap::new();
    for api in apis {
        let (name, version) = match *api {
            Api::Web3 => ("web3", "1.0"),
            Api::Net => ("net", "1.0"),
            Api::Eth => ("eth", "1.0"),
            Api::Stratum => ("stratum", "1.0"),
            Api::EthPubSub => ("pubsub", "1.0"),
            Api::Personal => ("personal", "1.0"),
            Api::Rpc => ("rpc", "1.0"),
            Api::Ping => ("ping", "1.0"),
        };
        modules.insert(name.into(), version.into());
    }
    modules
}

/// RPC dependencies can be used to initialize RPC endpoints from APIs.
pub trait Dependencies {
    type Notifier: ActivityNotifier;

    /// Create the activity notifier.
    fn activity_notifier(&self) -> Self::Notifier;

    /// Extend the given I/O handler with endpoints for each API.
    fn extend_with_set<S>(&self, handler: &mut MetaIoHandler<Metadata, S>, apis: &HashSet<Api>)
    where S: core::Middleware<Metadata>;
}

/// RPC dependencies for a full node.
pub struct FullDependencies {
    pub client: Arc<Client>,
    pub sync: Arc<SyncProvider>,
    pub account_store: Option<Arc<AccountProvider>>,
    pub miner: Arc<Miner>,
    pub external_miner: Arc<ExternalMiner>,
    pub dynamic_gas_price: Option<DynamicGasPrice>,
    pub executor: TaskExecutor,
}

impl FullDependencies {
    fn extend_api<S>(
        &self,
        handler: &mut MetaIoHandler<Metadata, S>,
        apis: &HashSet<Api>,
        for_generic_pubsub: bool,
    ) where
        S: core::Middleware<Metadata>,
    {
        use aion_rpc::impls::*;
        use aion_rpc::dispatch;
        use aion_rpc::traits::*;
        macro_rules! add_signing_methods {
            ($namespace:ident, $handler:expr, $deps:expr, $nonces:expr) => {{
                let deps = &$deps;
                let dispatcher = FullDispatcher::new(
                    deps.client.clone(),
                    deps.miner.clone(),
                    $nonces,
                    deps.dynamic_gas_price.clone(),
                );
                $handler.extend_with($namespace::to_delegate(SigningClient::new(
                    &deps.account_store,
                    dispatcher,
                )))
            }};
        }

        let nonces = Arc::new(Mutex::new(dispatch::Reservations::new(
            self.executor.clone(),
        )));
        let dispatcher = FullDispatcher::new(
            self.client.clone(),
            self.miner.clone(),
            nonces.clone(),
            self.dynamic_gas_price.clone(),
        );
        for api in apis {
            match *api {
                Api::Web3 => {
                    handler.extend_with(Web3Client::new().to_delegate());
                }
                Api::Net => {
                    handler.extend_with(NetClient::new(&self.sync).to_delegate());
                }
                Api::Eth => {
                    let client = EthClient::new(
                        &self.client,
                        &self.sync,
                        &self.account_store,
                        &self.miner,
                        &self.external_miner,
                        self.dynamic_gas_price.clone(),
                    );
                    handler.extend_with(client.to_delegate());

                    if !for_generic_pubsub {
                        let filter_client =
                            EthFilterClient::new(self.client.clone(), self.miner.clone());
                        handler.extend_with(filter_client.to_delegate());

                        add_signing_methods!(EthSigning, handler, self, nonces.clone());
                    }
                }
                Api::Stratum => {
                    let client = StratumClient::new(
                        &self.client,
                        &self.sync,
                        &self.miner,
                        &self.account_store,
                    );
                    handler.extend_with(client.to_delegate());
                }
                Api::EthPubSub => {
                    if !for_generic_pubsub {
                        let client =
                            EthPubSubClient::new(self.client.clone(), self.executor.clone());
                        let h = client.handler();
                        self.miner
                            .add_transactions_listener(Box::new(move |hashes| {
                                if let Some(h) = h.upgrade() {
                                    h.new_transactions(hashes);
                                }
                            }));

                        if let Some(h) = client.handler().upgrade() {
                            self.client.add_notify(h);
                        }
                        handler.extend_with(client.to_delegate());
                    }
                }
                Api::Personal => {
                    handler.extend_with(
                        PersonalClient::new(self.account_store.clone(), dispatcher.clone(), true)
                            .to_delegate(),
                    );
                }
                Api::Rpc => {
                    let modules = to_modules(&apis);
                    handler.extend_with(RpcClient::new(modules).to_delegate());
                }
                Api::Ping => {
                    handler.extend_with(PingClient::new().to_delegate());
                }
            }
        }
    }
}

impl Dependencies for FullDependencies {
    type Notifier = ClientNotifier;

    fn activity_notifier(&self) -> ClientNotifier {
        ClientNotifier {
            client: self.client.clone(),
        }
    }

    fn extend_with_set<S>(&self, handler: &mut MetaIoHandler<Metadata, S>, apis: &HashSet<Api>)
    where S: core::Middleware<Metadata> {
        self.extend_api(handler, apis, false)
    }
}

impl ApiSet {
    pub fn list_apis(&self) -> HashSet<Api> {
        let all = [
            Api::Web3,
            Api::Net,
            Api::Eth,
            Api::Stratum,
            Api::Rpc,
            Api::Personal,
            Api::EthPubSub,
            Api::Ping,
        ]
            .into_iter()
            .cloned()
            .collect();

        let public_list = [
            Api::Web3,
            Api::Net,
            Api::Eth,
            Api::Stratum,
            Api::Rpc,
            Api::Personal,
            Api::Ping,
        ]
            .into_iter()
            .cloned()
            .collect();

        match *self {
            ApiSet::List(ref apis) => apis.clone(),
            ApiSet::PublicContext => public_list,
            ApiSet::IpcContext => public_list,
            ApiSet::All => all,
        }
    }
}

#[cfg(test)]
mod test {
    use super::{Api, ApiSet};

    #[test]
    fn test_api_parsing() {
        assert_eq!(Api::Web3, "web3".parse().unwrap());
        assert_eq!(Api::Net, "net".parse().unwrap());
        assert_eq!(Api::Eth, "eth".parse().unwrap());
        assert_eq!(Api::Stratum, "stratum".parse().unwrap());
        assert_eq!(Api::EthPubSub, "pubsub".parse().unwrap());
        assert_eq!(Api::Personal, "personal".parse().unwrap());
        assert_eq!(Api::Rpc, "rpc".parse().unwrap());
        assert!("rp".parse::<Api>().is_err());
    }

    #[test]
    fn test_api_set_parsing() {
        assert_eq!(
            ApiSet::List(vec![Api::Web3, Api::Eth].into_iter().collect()),
            "web3,eth".parse().unwrap()
        );
    }

    #[test]
    fn test_api_set_ipc_context() {
        let expected = vec![
            // safe
            Api::Web3,
            Api::Net,
            Api::Eth,
            Api::Stratum,
            Api::Rpc,
            Api::Personal,
        ]
        .into_iter()
        .collect();
        assert_eq!(ApiSet::IpcContext.list_apis(), expected);
    }

    #[test]
    fn test_all_apis() {
        assert_eq!(
            "all".parse::<ApiSet>().unwrap(),
            ApiSet::List(
                vec![
                    Api::Web3,
                    Api::Net,
                    Api::Eth,
                    Api::Stratum,
                    Api::Rpc,
                    Api::Personal,
                    Api::EthPubSub
                ]
                .into_iter()
                .collect()
            )
        );
    }

    #[test]
    fn test_all_without_personal_apis() {
        assert_eq!(
            "personal,all,-personal".parse::<ApiSet>().unwrap(),
            ApiSet::List(
                vec![
                    Api::Web3,
                    Api::Net,
                    Api::Eth,
                    Api::Stratum,
                    Api::Rpc,
                    Api::EthPubSub
                ]
                .into_iter()
                .collect()
            )
        );
    }
    /*
    #[test]
    fn test_safe_parsing() {
        assert_eq!(
            "safe".parse::<ApiSet>().unwrap(),
            ApiSet::List(
                vec![
                    Api::Web3,
                    Api::Net,
                    Api::Eth,
                    Api::Stratum,
                    Api::EthPubSub,
                    Api::Rpc,
                ].into_iter()
                .collect()
            )
        );
    }*/
}
