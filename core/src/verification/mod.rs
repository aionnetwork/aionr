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

//! Block verification utilities.

pub mod verification;
pub mod verifier;
pub mod queue;
mod canon_verifier;
mod noop_verifier;

pub use self::verification::*;
pub use self::verifier::Verifier;
pub use self::canon_verifier::CanonVerifier;
pub use self::noop_verifier::NoopVerifier;
pub use self::queue::{BlockQueue, Config as QueueConfig, VerificationQueue, QueueInfo};

/// Verifier type.
#[derive(Debug, PartialEq, Clone)]
pub enum VerifierType {
    /// Verifies block normally.
    Canon,
    /// Verifies block normallly, but skips seal verification.
    CanonNoSeal,
    /// Does not verify block at all.
    /// Used in tests.
    Noop,
}

impl Default for VerifierType {
    fn default() -> Self { VerifierType::Canon }
}

/// Create a new verifier based on type.
pub fn new(v: VerifierType) -> Box<Verifier> {
    match v {
        VerifierType::Canon | VerifierType::CanonNoSeal => Box::new(CanonVerifier),
        VerifierType::Noop => Box::new(NoopVerifier),
    }
}

impl VerifierType {
    /// Check if seal verification is enabled for this verifier type.
    pub fn verifying_seal(&self) -> bool {
        match *self {
            VerifierType::Canon => true,
            VerifierType::Noop | VerifierType::CanonNoSeal => false,
        }
    }
}
