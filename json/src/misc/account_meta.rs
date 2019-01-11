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

//! Misc deserialization.

use hash;

/// Collected account metadata
#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccountMeta {
    /// The name of the account.
    pub name: String,
    /// The rest of the metadata of the account.
    pub meta: String,
    /// The 128-bit Uuid of the account, if it has one (brain-wallets don't).
    pub uuid: Option<String>,
}

impl_serialization!(hash::Address => AccountMeta);
