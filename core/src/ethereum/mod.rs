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

//! Ethereum protocol module.
//!
//! Contains all Ethereum network specific stuff, such as denominations and
//! consensus specifications.

/// Export the denominations module.
pub mod denominations;

pub use self::denominations::*;

#[cfg(test)]
use machine::EthereumMachine;

use super::spec::*;

fn load<'a, T: Into<Option<SpecParams<'a>>>>(params: T, b: &[u8]) -> Spec {
    match params.into() {
        Some(params) => Spec::load(params, b),
        None => Spec::load(&::std::env::temp_dir(), b),
    }
    .expect("chain spec is invalid")
}

/// Create a new Foundation Mainnet chain spec.
pub fn new_foundation<'a, T: Into<SpecParams<'a>>>(params: T) -> Spec {
    load(params.into(), include_bytes!("../../res/aion/mainnet.json"))
}

#[cfg(test)]
fn load_machine(b: &[u8]) -> EthereumMachine {
    Spec::load_machine(b).expect("chain spec is invalid")
}

// For tests

/// Create a new Foundation Frontier-era chain spec as though it never changes to Homestead.
#[cfg(test)]
pub fn new_frontier_test_machine() -> EthereumMachine {
    load_machine(include_bytes!("../../res/testnet.json"))
}

#[cfg(test)]
pub fn new_aion_test_machine() -> EthereumMachine {
    load_machine(include_bytes!("../../res/testnet.json"))
}
