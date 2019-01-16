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

//! Ethereum rpc interface implementation.
macro_rules! try_bf {
    ($res: expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => return Box::new(::jsonrpc_core::futures::future::err(e.into())),
        }
    };
}

#[macro_use]
mod eth;
mod eth_filter;
mod eth_pubsub;
mod net;
mod personal;
mod signing;
mod rpc;
mod stratum;
mod web3;
mod ping;

pub use self::eth::EthClient;
pub use self::eth_filter::EthFilterClient;
pub use self::eth_pubsub::EthPubSubClient;
pub use self::net::NetClient;
pub use self::personal::PersonalClient;
pub use self::signing::SigningClient;
pub use self::web3::Web3Client;
pub use self::rpc::RpcClient;
pub use self::stratum::StratumClient;
pub use self::ping::PingClient;
