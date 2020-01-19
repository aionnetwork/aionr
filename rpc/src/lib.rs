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
extern crate jsonrpc_http_server as http;
extern crate jsonrpc_ipc_server as ipc;
extern crate jsonrpc_ws_server as ws;
extern crate acore_bytes as bytes;
extern crate aion_version as version;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

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
pub use ws::{Server as WsServer, Error as WsError};

pub use crate::helpers::dispatch;
pub use metadata::Metadata;
pub use crate::types::Origin;

mod server_http;
mod server_ipc;
mod server_ws;
pub use crate::server_http::{RpcExtractor, start_http};
pub use crate::server_ipc::start_ipc;
pub use crate::server_ws::start_ws;
