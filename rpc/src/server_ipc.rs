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

use std::sync::Arc;
use jsonrpc_core;
use ipc;
use Metadata;
use types::Origin;
use jsonrpc_pubsub::Session;
use server_http::RpcExtractor;
use tokio::runtime::TaskExecutor;
impl ipc::MetaExtractor<Metadata> for RpcExtractor {
    fn extract(&self, req: &ipc::RequestContext) -> Metadata {
        Metadata {
            origin: Origin::Ipc(req.session_id.into()),
            session: Some(Arc::new(Session::new(req.sender.clone()))),
        }
    }
}

/// Start ipc server asynchronously and returns result with `Server` handle on success or an error.
pub fn start_ipc<M, S, H, T>(
    addr: &str,
    handler: H,
    extractor: T,
    executor: TaskExecutor,
) -> ::std::io::Result<ipc::Server>
where
    M: jsonrpc_core::Metadata,
    S: jsonrpc_core::Middleware<M>,
    H: Into<jsonrpc_core::MetaIoHandler<M, S>>,
    T: ipc::MetaExtractor<M>,
{
    ipc::ServerBuilder::with_meta_extractor(handler, extractor)
        .event_loop_executor(executor)
        .start(addr)
}
