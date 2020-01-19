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

use std::io;
use std::sync::Arc;
use std::path::PathBuf;
use std::collections::HashSet;

use crate::helpers::aion_ipc_path;
use jsonrpc_core::MetaIoHandler;
use aion_rpc::informant::{RpcStats, Middleware};
use aion_rpc::{self as rpc, Metadata, DomainsValidation};
use crate::rpc_apis::{self, ApiSet};
use tokio::runtime::TaskExecutor;
pub use aion_rpc::{IpcServer, HttpServer, WsServer, RequestMiddleware, WsError};

#[derive(Debug, Clone, PartialEq)]
pub struct HttpConfiguration {
    pub enabled: bool,
    pub interface: String,
    pub port: u16,
    pub apis: ApiSet,
    pub cors: Option<Vec<String>>,
    pub hosts: Option<Vec<String>>,
    pub server_threads: usize,
    pub processing_threads: usize,
}

impl Default for HttpConfiguration {
    fn default() -> Self {
        HttpConfiguration {
            enabled: true,
            interface: "127.0.0.1".into(),
            port: 8545,
            apis: ApiSet::PublicContext,
            cors: Some(vec![]),
            hosts: Some(vec![]),
            server_threads: 1,
            processing_threads: 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IpcConfiguration {
    pub enabled: bool,
    pub socket_addr: String,
    pub apis: ApiSet,
}

impl Default for IpcConfiguration {
    fn default() -> Self {
        IpcConfiguration {
            enabled: true,
            socket_addr: if cfg!(windows) {
                r"\\.\pipe\jsonrpc.ipc".into()
            } else {
                let data_dir = ::dir::default_data_path();
                aion_ipc_path(&data_dir, "$BASE/jsonrpc.ipc")
            },
            apis: ApiSet::IpcContext,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WsConfiguration {
    pub enabled: bool,
    pub interface: String,
    pub port: u16,
    pub apis: ApiSet,
    pub origins: Option<Vec<String>>,
    pub hosts: Option<Vec<String>>,
    pub max_connections: usize,
}

impl Default for WsConfiguration {
    fn default() -> Self {
        WsConfiguration {
            enabled: true,
            interface: "127.0.0.1".into(),
            port: 8546,
            apis: ApiSet::PublicContext,
            origins: Some(Vec::new()),
            hosts: Some(Vec::new()),
            max_connections: 100,
        }
    }
}

pub struct Dependencies<D: rpc_apis::Dependencies> {
    pub apis: Arc<D>,
    pub stats: Arc<RpcStats>,
}

pub fn new_ws<D: rpc_apis::Dependencies>(
    conf: WsConfiguration,
    deps: &Dependencies<D>,
    executor: TaskExecutor,
) -> Result<Option<WsServer>, String>
{
    if !conf.enabled {
        return Ok(None);
    }

    let url = format!("{}:{}", conf.interface, conf.port);
    let addr = url
        .parse()
        .map_err(|_| format!("Invalid WebSockets listen host/port given: {}", url))?;

    let handler = setup_apis(conf.apis, deps);

    let allowed_origins = into_domains(with_domain(conf.origins));
    let allowed_hosts = into_domains(with_domain(conf.hosts));
    let start_result = rpc::start_ws(
        &addr,
        handler,
        allowed_origins,
        allowed_hosts,
        rpc::RpcExtractor,
        executor,
        conf.max_connections,
    );

    match start_result {
        Ok(server) => Ok(Some(server)),
        Err(WsError::Io(ref err)) if err.kind() == io::ErrorKind::AddrInUse => {
            Err(format!(
                "WebSockets address {} is already in use, make sure that another instance of an \
                 Aion client is not running or change the address using the --ws-port and \
                 --ws-interface options.",
                url
            ))
        }
        Err(e) => Err(format!("WebSockets error: {:?}", e)),
    }
}

pub fn new_http<D: rpc_apis::Dependencies>(
    id: &str,
    options: &str,
    conf: HttpConfiguration,
    deps: &Dependencies<D>,
    executor: TaskExecutor,
) -> Result<Option<HttpServer>, String>
{
    if !conf.enabled {
        return Ok(None);
    }

    let url = format!("{}:{}", conf.interface, conf.port);
    let addr = url
        .parse()
        .map_err(|_| format!("Invalid {} listen host/port given: {}", id, url))?;
    let handler = setup_apis(conf.apis, deps);

    let cors_domains = into_domains(conf.cors);
    let allowed_hosts = into_domains(with_domain(conf.hosts));

    let start_result = rpc::start_http(
        &addr,
        cors_domains,
        allowed_hosts,
        handler,
        rpc::RpcExtractor,
        conf.server_threads,
        executor,
    );

    match start_result {
        Ok(server) => Ok(Some(server)),
        Err(ref err) if err.kind() == io::ErrorKind::AddrInUse => {
            Err(format!(
                "{} address {} is already in use, make sure that another instance of an Aion \
                 client is not running or change the address using the --{}-port and \
                 --{}-interface options.",
                id, url, options, options
            ))
        }
        Err(e) => Err(format!("{} error: {:?}", id, e)),
    }
}

pub fn new_ipc<D: rpc_apis::Dependencies>(
    conf: IpcConfiguration,
    dependencies: &Dependencies<D>,
    executor: TaskExecutor,
) -> Result<Option<IpcServer>, String>
{
    if !conf.enabled {
        return Ok(None);
    }

    let handler = setup_apis(conf.apis, dependencies);
    let path = PathBuf::from(&conf.socket_addr);
    // Make sure socket file can be created on unix-like OS.
    // Windows pipe paths are not on the FS.
    if !cfg!(windows) {
        if let Some(dir) = path.parent() {
            ::std::fs::create_dir_all(&dir).map_err(|err| {
                format!(
                    "Unable to create IPC directory at {}: {}",
                    dir.display(),
                    err
                )
            })?;
        }
    }

    match rpc::start_ipc(&conf.socket_addr, handler, rpc::RpcExtractor, executor) {
        Ok(server) => Ok(Some(server)),
        Err(io_error) => Err(format!("IPC error: {}", io_error)),
    }
}

fn into_domains<T: From<String>>(items: Option<Vec<String>>) -> DomainsValidation<T> {
    items
        .map(|vals| vals.into_iter().map(T::from).collect())
        .into()
}

fn with_domain(items: Option<Vec<String>>) -> Option<Vec<String>> {
    items.map(move |items| {
        let items = items.into_iter().collect::<HashSet<_>>();
        /*{
            |address: &Option<rpc::Host>| {
                if let Some(host) = address.clone() {
                    items.insert(host.to_string());
                    items.insert(host.replace("127.0.0.1", "localhost"));
                }
            };
        }*/
        items.into_iter().collect()
    })
}

fn setup_apis<D>(
    apis: ApiSet,
    deps: &Dependencies<D>,
) -> MetaIoHandler<Metadata, Middleware<D::Notifier>>
where
    D: rpc_apis::Dependencies,
{
    let mut handler = MetaIoHandler::with_middleware(Middleware::new(
        deps.stats.clone(),
        deps.apis.activity_notifier(),
    ));
    let apis = apis.list_apis();
    deps.apis.extend_with_set(&mut handler, &apis);

    handler
}
