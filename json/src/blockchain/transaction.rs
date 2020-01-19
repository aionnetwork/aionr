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

//! Blockchain test transaction deserialization.

use crate::uint::Uint;
use crate::bytes::Bytes;

/// Blockchain test transaction deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Transaction {
    data: Bytes,
    // unused.
    timestamp: Bytes,
    #[serde(rename = "gasLimit")]
    gas_limit: Uint,
    #[serde(rename = "gasPrice")]
    gas_price: Uint,
    nonce: Uint,
    r: Uint,
    s: Uint,
    v: Uint,
    value: Uint,
}
