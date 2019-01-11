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

//! No-op verifier.

use engines::EthEngine;
use error::Error;
use header::Header;
use super::{verification, Verifier};

/// A no-op verifier -- this will verify everything it's given immediately.
#[allow(dead_code)]
pub struct NoopVerifier;

impl Verifier for NoopVerifier {
    fn verify_block_family(
        &self,
        _: &Header,
        _t: &Header,
        _: Option<&Header>,
        _: &EthEngine,
        _: Option<verification::FullFamilyParams>,
    ) -> Result<(), Error>
    {
        Ok(())
    }

    fn verify_block_final(&self, _expected: &Header, _got: &Header) -> Result<(), Error> { Ok(()) }

    fn verify_block_external(&self, _header: &Header, _engine: &EthEngine) -> Result<(), Error> {
        Ok(())
    }
}
