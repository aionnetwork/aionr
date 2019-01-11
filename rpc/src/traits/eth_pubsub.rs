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

//! Eth PUB-SUB rpc interface.

use jsonrpc_core::Result;
use jsonrpc_macros::Trailing;
use jsonrpc_macros::pubsub::Subscriber;
use jsonrpc_pubsub::SubscriptionId;

use types::pubsub;

build_rpc_trait! {
    /// Eth PUB-SUB rpc interface.
    pub trait EthPubSub {
        type Metadata;

        #[pubsub(name = "eth_subscription")] {
            /// Subscribe to Eth subscription.
            #[rpc(name = "eth_subscribe")]
            fn subscribe(&self, Self::Metadata, Subscriber<pubsub::Result>, pubsub::Kind, Trailing<pubsub::Params>);

            /// Unsubscribe from existing Eth subscription.
            #[rpc(name = "eth_unsubscribe")]
            fn unsubscribe(&self, SubscriptionId) -> Result<bool>;
        }
    }
}
