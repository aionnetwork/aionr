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

#![warn(unused_extern_crates)]
#[macro_use]
extern crate futures;
extern crate ansi_term;
extern crate order_stat;
extern crate parking_lot;
extern crate rustc_hex;
extern crate serde;
extern crate serde_json;
extern crate blake2b;
extern crate trace_time;

extern crate tokio;
extern crate transient_hashmap;

extern crate jsonrpc_core;
extern crate jsonrpc_http_server as http;
extern crate jsonrpc_ipc_server as ipc;
extern crate jsonrpc_ws_server as ws;
extern crate jsonrpc_pubsub;

extern crate sync;
extern crate acore;
extern crate acore_bytes as bytes;
extern crate aion_types;
extern crate ethbloom;
extern crate key;
extern crate solidity;
extern crate aion_version as version;
extern crate rlp;
extern crate stats;

extern crate tiny_keccak;

#[macro_use]
extern crate log;
#[macro_use]
extern crate jsonrpc_macros;
#[macro_use]
extern crate serde_derive;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

#[cfg(test)]
#[macro_use]
extern crate macros;

mod helpers;

pub mod types;
pub mod informant;
pub mod metadata;
pub mod traits;
pub mod impls;

pub use jsonrpc_pubsub::Session as PubSubSession;
pub use ipc::{
    Server as IpcServer, MetaExtractor as IpcMetaExtractor, RequestContext as IpcRequestContext,
};
pub use http::{
    Server as HttpServer, hyper, RequestMiddleware, RequestMiddlewareAction,
    AccessControlAllowOrigin, Host, DomainsValidation,
};
pub use ws::{Server as WsServer, Error as WsError, ErrorKind as WsErrorKind};

pub use helpers::{block_import::is_major_importing, dispatch};
pub use metadata::Metadata;
pub use types::Origin;

mod server_http;
mod server_ipc;
mod server_ws;
pub use server_http::{RpcExtractor, start_http};
pub use server_ipc::start_ipc;
pub use server_ws::start_ws;
