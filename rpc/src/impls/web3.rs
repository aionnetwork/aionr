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

//! Web3 rpc implementation.
use tiny_keccak::keccak256;
use jsonrpc_core::Result;
use version::version;
use traits::Web3;
use aion_types::H256;

use types::Bytes;

/// Web3 rpc implementation.
pub struct Web3Client;

impl Web3Client {
    /// Creates new Web3Client.
    pub fn new() -> Self { Web3Client }
}

impl Web3 for Web3Client {
    fn client_version(&self) -> Result<String> { Ok(version().to_owned()) }

    fn sha3(&self, data: Bytes) -> Result<H256> { Ok(keccak256(&data.0).into()) }
}
