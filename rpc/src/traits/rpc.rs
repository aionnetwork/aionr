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

//! RPC interface.

use std::collections::BTreeMap;

use jsonrpc_core::Result;

build_rpc_trait! {
    /// RPC Interface.
    pub trait Rpc {
        /// Returns supported modules for Geth 1.3.6
        /// @ignore
        #[rpc(name = "modules")]
        fn modules(&self) -> Result<BTreeMap<String, String>>;

        /// Returns supported modules for Geth 1.4.0
        /// @ignore
        #[rpc(name = "rpc_modules")]
        fn rpc_modules(&self) -> Result<BTreeMap<String, String>>;
    }
}
