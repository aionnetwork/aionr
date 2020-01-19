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

use jsonrpc_core;
use http;
use hyper;
use tokio::runtime::TaskExecutor;
use crate::Metadata;
use crate::types::Origin;
use std::net::SocketAddr;

/// Common HTTP & IPC & WebSocket metadata extractor.
pub struct RpcExtractor;

impl http::MetaExtractor<Metadata> for RpcExtractor {
    fn read_metadata(&self, req: &hyper::Request<hyper::Body>) -> Metadata {
        let as_string = |header: Option<&hyper::header::HeaderValue>| {
            header.and_then(|val| val.to_str().ok().map(|s| s.to_owned()))
        };
        let user_agent = as_string(req.headers().get("user-agent"));
        Metadata {
            origin: match user_agent {
                Some(service) => Origin::Rpc(service.into()),
                None => Origin::Rpc("unknown".into()),
            },
            session: None,
        }
    }
}

/// Start http server asynchronously and returns result with `Server` handle on success or an error.
pub fn start_http<M, S, H, T>(
    addr: &SocketAddr,
    cors_domains: http::DomainsValidation<http::AccessControlAllowOrigin>,
    allowed_hosts: http::DomainsValidation<http::Host>,
    handler: H,
    extractor: T,
    threads: usize,
    executor: TaskExecutor,
) -> ::std::io::Result<http::Server>
where
    M: jsonrpc_core::Metadata,
    S: jsonrpc_core::Middleware<M>,
    H: Into<jsonrpc_core::MetaIoHandler<M, S>>,
    T: http::MetaExtractor<M>,
{
    let builder = http::ServerBuilder::with_meta_extractor(handler, extractor)
        .threads(threads)
        .event_loop_executor(executor)
        .cors(cors_domains.into())
        .allowed_hosts(allowed_hosts.into())
        .keep_alive(false);

    Ok(builder.start_http(addr)?)
}
