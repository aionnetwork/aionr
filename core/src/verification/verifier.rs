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

//! A generic verifier trait.

use engines::EthEngine;
use error::Error;
use header::Header;
use super::verification;

/// Should be used to verify blocks.
pub trait Verifier: Send + Sync {
    /// Verify a block relative to its parent and uncles.
    fn verify_block_family(
        &self,
        header: &Header,
        parent: &Header,
        grant_parent: Option<&Header>,
        engine: &EthEngine,
        do_full: Option<verification::FullFamilyParams>,
    ) -> Result<(), Error>;

    /// Do a final verification check for an enacted header vs its expected counterpart.
    fn verify_block_final(&self, expected: &Header, got: &Header) -> Result<(), Error>;
    /// Verify a block, inspecing external state.
    fn verify_block_external(&self, header: &Header, engine: &EthEngine) -> Result<(), Error>;
}
