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
use hash::Address;
use bytes::Bytes;
use spec::{Account, Builtin};
use aion_types::U256;
use uint::Uint;

/// Blockchain test state deserializer.
#[derive(Debug, PartialEq, Deserialize)]
pub struct State(BTreeMap<Address, Account>);

impl State {
    /// Returns all builtins.
    pub fn builtins(&self) -> BTreeMap<Address, Builtin> {
        self.0
            .iter()
            .filter_map(|(add, ref acc)| {
                acc.builtin.clone().map(|mut b| {
                    // add contract address to builtin's address.
                    b.set_address(add.clone());

                    (add.clone(), b)
                })
            })
            .collect()
    }

    /// Returns all constructors.
    pub fn constructors(&self) -> BTreeMap<Address, Bytes> {
        self.0
            .iter()
            .filter_map(|(add, ref acc)| acc.constructor.clone().map(|b| (add.clone(), b)))
            .collect()
    }

    /// Returns premine number.
    pub fn premine(&self) -> U256 {
        self.0
            .iter()
            .fold(U256::from(0), |sum, (_add, ref acc)| sum + acc.balance.clone().unwrap_or(Uint(U256::from(0))).0)
    }
}

impl IntoIterator for State {
    type Item = <BTreeMap<Address, Account> as IntoIterator>::Item;
    type IntoIter = <BTreeMap<Address, Account> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}
