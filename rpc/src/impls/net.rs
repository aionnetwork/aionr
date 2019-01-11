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

//! Net rpc implementation.
use std::sync::Arc;
use jsonrpc_core::Result;
use sync::sync::SyncProvider;
use traits::Net;

/// Net rpc implementation.
pub struct NetClient<S: ?Sized> {
    sync: Arc<S>,
}

impl<S: ?Sized> NetClient<S>
where S: SyncProvider
{
    /// Creates new NetClient.
    pub fn new(sync: &Arc<S>) -> Self {
        NetClient {
            sync: sync.clone(),
        }
    }
}

impl<S: ?Sized> Net for NetClient<S>
where S: SyncProvider + 'static
{
    fn version(&self) -> Result<String> {
        Ok(format!("{}", self.sync.status().network_id).to_owned())
    }

    fn peer_count(&self) -> Result<u64> { Ok(self.sync.status().num_peers as u64) }

    fn is_listening(&self) -> Result<bool> {
        // right now (11 march 2016), we are always listening for incoming connections
        //
        // (this may not be true now -- 26 september 2016)
        Ok(true)
    }
}
