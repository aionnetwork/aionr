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

use std::sync::Arc;

use sync::p2p::NetworkConfig;
use sync::sync::{Sync, SyncConfig, NetworkManager, Params, SyncProvider};
use sync::sync::error::SyncError;
use acore::client::BlockChainClient;

pub use acore::client::ChainNotify;

pub type SyncModules = (Arc<SyncProvider>, Arc<NetworkManager>, Arc<ChainNotify>);

pub fn sync(
    sync_cfg: SyncConfig,
    net_cfg: NetworkConfig,
    cli: Arc<BlockChainClient>,
) -> Result<SyncModules, SyncError>
{
    let sync = Sync::get_instance(Params {
        config: sync_cfg,
        client: cli,
        network_config: net_cfg,
    });

    Ok((
        sync.clone() as Arc<SyncProvider>,
        sync.clone() as Arc<NetworkManager>,
        sync.clone() as Arc<ChainNotify>,
    ))
}
