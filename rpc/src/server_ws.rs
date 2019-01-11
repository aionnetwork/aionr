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
use ws;
use jsonrpc_core as core;
use jsonrpc_pubsub::Session;
use std::net::SocketAddr;
use tokio::runtime::TaskExecutor;
use Metadata;
use types::Origin;

use server_http::RpcExtractor;

impl ws::MetaExtractor<Metadata> for RpcExtractor {
    fn extract(&self, req: &ws::RequestContext) -> Metadata {
        let origin = Origin::Ws {
            origin: req
                .origin
                .as_ref()
                .map(|origin| (&**origin).into())
                .unwrap_or_default(),
            session: req.session_id.into(),
        };
        let session = Some(Arc::new(Session::new(req.sender())));
        Metadata {
            origin,
            session,
        }
    }
}

/// Start WS server and return `Server` handle.
pub fn start_ws<M, S, H, T>(
    addr: &SocketAddr,
    handler: H,
    allowed_origins: ws::DomainsValidation<ws::Origin>,
    allowed_hosts: ws::DomainsValidation<ws::Host>,
    extractor: T,
    executor: TaskExecutor,
    max_connections: usize,
) -> Result<ws::Server, ws::Error>
where
    M: core::Metadata,
    S: core::Middleware<M>,
    H: Into<core::MetaIoHandler<M, S>>,
    T: ws::MetaExtractor<M>,
{
    ws::ServerBuilder::with_meta_extractor(handler, extractor)
        .event_loop_executor(executor)
        .max_connections(max_connections)
        .allowed_origins(allowed_origins)
        .allowed_hosts(allowed_hosts)
        .start(addr)
}
