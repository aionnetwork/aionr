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

//! Consensus engine specification and basic implementations.
pub mod pow_equihash_engine;
pub mod epoch;

pub use self::epoch::{EpochVerifier, Transition as EpochTransition};
pub use self::pow_equihash_engine::POWEquihashEngine;

use std::sync::Arc;
use std::fmt;
use error::Error;

use aion_machine::{Machine, LocalizedMachine as Localized};
use aion_types::{H256, Address};
use unexpected::{Mismatch, OutOfBounds};

/// Voting errors.
#[derive(Debug)]
pub enum EngineError {
    /// Signature or author field does not belong to an authority.
    NotAuthorized(Address),
    /// The same author issued different votes at the same step.
    DoubleVote(Address),
    /// The received block is from an incorrect proposer.
    NotProposer(Mismatch<Address>),
    /// Message was not expected.
    UnexpectedMessage,
    /// Seal field has an unexpected size.
    BadSealFieldSize(OutOfBounds<usize>),
    /// Validation proof insufficient.
    InsufficientProof(String),
    /// Failed system call.
    FailedSystemCall(String),
    /// Malformed consensus message.
    MalformedMessage(String),
    /// Requires client ref, but none registered.
    RequiresClient,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::EngineError::*;
        let msg = match *self {
            DoubleVote(ref address) => format!("Author {} issued too many blocks.", address),
            NotProposer(ref mis) => format!("Author is not a current proposer: {}", mis),
            NotAuthorized(ref address) => format!("Signer {} is not authorized.", address),
            UnexpectedMessage => "This Engine should not be fed messages.".into(),
            BadSealFieldSize(ref oob) => format!("Seal field has an unexpected length: {}", oob),
            InsufficientProof(ref msg) => format!("Insufficient validation proof: {}", msg),
            FailedSystemCall(ref msg) => format!("Failed to make system call: {}", msg),
            MalformedMessage(ref msg) => format!("Received malformed consensus message: {}", msg),
            RequiresClient => format!("Call requires client but none registered"),
        };

        f.write_fmt(format_args!("Engine error ({})", msg))
    }
}

/// Proof dependent on state.
pub trait StateDependentProof<M: Machine>: Send + Sync {
    /// Generate a proof, given the state.
    // TODO: make this into an &M::StateContext
    fn generate_proof<'a>(
        &self,
        state: &<M as Localized<'a>>::StateContext,
    ) -> Result<Vec<u8>, String>;
    /// Check a proof generated elsewhere (potentially by a peer).
    // `engine` needed to check state proofs, while really this should
    // just be state machine params.
    fn check_proof(&self, machine: &M, proof: &[u8]) -> Result<(), String>;
}

/// Proof generated on epoch change.
pub enum Proof<M: Machine> {
    /// Known proof (extracted from signal)
    Known(Vec<u8>),
    /// State dependent proof.
    WithState(Arc<StateDependentProof<M>>),
}

/// Generated epoch verifier.
pub enum ConstructedVerifier<'a, M: Machine> {
    /// Fully trusted verifier.
    Trusted(Box<EpochVerifier<M>>),
    /// Verifier unconfirmed. Check whether given finality proof finalizes given hash
    /// under previous epoch.
    Unconfirmed(Box<EpochVerifier<M>>, &'a [u8], H256),
    /// Error constructing verifier.
    Err(Error),
}

impl<'a, M: Machine> ConstructedVerifier<'a, M> {
    /// Convert to a result, indicating that any necessary confirmation has been done
    /// already.
    pub fn known_confirmed(self) -> Result<Box<EpochVerifier<M>>, Error> {
        match self {
            ConstructedVerifier::Trusted(v) | ConstructedVerifier::Unconfirmed(v, _, _) => Ok(v),
            ConstructedVerifier::Err(e) => Err(e),
        }
    }
}
