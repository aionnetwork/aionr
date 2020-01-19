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

//! RPC generic methods implementation.
use std::collections::BTreeMap;
use jsonrpc_core::Result;
use crate::traits::Rpc;
use crate::Metadata;

/// RPC generic methods implementation.
pub struct RpcClient {
    modules: BTreeMap<String, String>,
    valid_apis: Vec<String>,
}

impl RpcClient {
    /// Creates new `RpcClient`.
    pub fn new(modules: BTreeMap<String, String>) -> Self {
        // geth 1.3.6 fails upon receiving unknown api
        let valid_apis = vec!["web3", "eth", "net", "personal", "rpc", "stratum"];

        RpcClient {
            modules: modules,
            valid_apis: valid_apis.into_iter().map(|x| x.to_owned()).collect(),
        }
    }
}

impl Rpc for RpcClient {
    type Metadata = Metadata;

    fn rpc_modules(&self) -> Result<BTreeMap<String, String>> {
        let modules = self
            .modules
            .iter()
            .fold(BTreeMap::new(), |mut map, (k, v)| {
                map.insert(k.to_owned(), v.to_owned());
                map
            });

        Ok(modules)
    }

    fn modules(&self) -> Result<BTreeMap<String, String>> {
        let modules = self
            .modules
            .iter()
            .filter(|&(k, _v)| self.valid_apis.contains(k))
            .fold(BTreeMap::new(), |mut map, (k, v)| {
                map.insert(k.to_owned(), v.to_owned());
                map
            });

        Ok(modules)
    }
}
