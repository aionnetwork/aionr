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

//! Blockchain test state deserializer.

use std::collections::BTreeMap;
use crate::hash::Address;
use crate::blockchain::account::Account;

/// Blockchain test state deserializer.
#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct State(BTreeMap<Address, Account>);

impl IntoIterator for State {
    type Item = <BTreeMap<Address, Account> as IntoIterator>::Item;
    type IntoIter = <BTreeMap<Address, Account> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}
