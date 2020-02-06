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

//! Aion RPC requests Metadata.
use std::sync::Arc;

use jsonrpc_core;
use jsonrpc_pubsub::{Session, PubSubMetadata};

use types::Origin;

/// RPC methods metadata.
#[derive(Clone, Default, Debug)]
pub struct Metadata {
    /// Request origin
    pub origin: Origin,
    /// Request PubSub Session
    pub session: Option<Arc<Session>>,
}

impl jsonrpc_core::Metadata for Metadata {}
impl PubSubMetadata for Metadata {
    fn session(&self) -> Option<Arc<Session>> { self.session.clone() }
}
