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
pub mod unity_engine;
pub use self::unity_engine::UnityEngine;

use std::fmt;
use std::sync::Arc;
use types::error::Error;
use spec::CommonParams;
use header::{Header, BlockNumber};
use std::collections::{BTreeMap};
use precompiled::builtin::BuiltinContract;
use transaction::{UnverifiedTransaction, SignedTransaction};
use aion_types::{U256, Address};
use machine::EthereumMachine;
use client::BlockChainClient;

use aion_machine::{Machine, LocalizedMachine as Localized};
use unexpected::{Mismatch, OutOfBounds};
use num_bigint::BigUint;

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
    // TODO: make this into an &<EthereumMachine as Machine>::StateContext
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
    WithState(Arc<dyn StateDependentProof<M>>),
}

/// Common type alias for an engine coupled with an Ethereum-like state machine.
// TODO: make this a _trait_ alias when those exist.
// fortunately the effect is largely the same since engines are mostly used
// via trait objects.
pub trait Engine: Sync + Send {
    /// The name of this engine.
    fn name(&self) -> &str;

    /// Get access to the underlying state machine.
    // TODO: decouple.
    fn machine(&self) -> &EthereumMachine;

    /// Calculate the difficulty of
    fn calculate_difficulty(
        &self,
        _parent: &Header,
        _grand_parent: Option<&Header>,
        _great_grand_parent: Option<&Header>,
        _client: &dyn BlockChainClient,
    ) -> U256
    {
        U256::from(0)
    }

    /// The number of additional header fields required for this engine.
    fn seal_fields(&self, _header: &<EthereumMachine as Machine>::Header) -> usize { 0 }

    /// Block transformation functions, after the transactions.
    fn on_close_block(
        &self,
        _block: &mut <EthereumMachine as Machine>::LiveBlock,
    ) -> Result<(), <EthereumMachine as Machine>::Error>
    {
        Ok(())
    }

    /// Verify a locally-generated seal of a header.
    ///
    /// If this engine seals internally,
    /// no checks have to be done here, since all internally generated seals
    /// should be valid.
    ///
    /// Externally-generated seals (e.g. PoW) will need to be checked for validity.
    ///
    /// It is fine to require access to state or a full client for this function, since
    /// light clients do not generate seals.
    fn verify_local_seal_pow(
        &self,
        header: &<EthereumMachine as Machine>::Header,
    ) -> Result<(), <EthereumMachine as Machine>::Error>;

    fn verify_seal_pos(
        &self,
        header: &<EthereumMachine as Machine>::Header,
        parent: &<EthereumMachine as Machine>::Header,
        grand_parent: Option<&<EthereumMachine as Machine>::Header>,
        stake: Option<BigUint>,
    ) -> Result<(), <EthereumMachine as Machine>::Error>;

    /// Phase 1 quick block verification. Only does checks that are cheap. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_block_basic(
        &self,
        _header: &<EthereumMachine as Machine>::Header,
    ) -> Result<(), <EthereumMachine as Machine>::Error>
    {
        Ok(())
    }

    /// Phase 2 verification. Perform costly checks such as transaction signatures. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_block_unordered(
        &self,
        _header: &<EthereumMachine as Machine>::Header,
    ) -> Result<(), <EthereumMachine as Machine>::Error>
    {
        Ok(())
    }

    /// Phase 3 verification. Check block information against parent. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_block_family(
        &self,
        _header: &<EthereumMachine as Machine>::Header,
        _parent: &<EthereumMachine as Machine>::Header,
        _grand_parent: Option<&<EthereumMachine as Machine>::Header>,
        _great_grand_parent: Option<&<EthereumMachine as Machine>::Header>,
        _client: &dyn BlockChainClient,
    ) -> Result<(), Error>
    {
        Ok(())
    }

    /// Populate a header's difficulty based on its parent's header.
    /// Usually implements the chain scoring rule based on weight.
    fn set_difficulty_from_parent(
        &self,
        _header: &mut <EthereumMachine as Machine>::Header,
        _parent: &<EthereumMachine as Machine>::Header,
        _grand_parent: Option<&<EthereumMachine as Machine>::Header>,
        _great_grand_parent: Option<&<EthereumMachine as Machine>::Header>,
        _client: &dyn BlockChainClient,
    )
    {
    }

    //    /// Trigger next step of the consensus engine.
    //    fn step(&self) {}
    //
    /// Stops any services that the may hold the Engine and makes it safe to drop.
    fn stop(&self) {}

    /// Get the general parameters of the chain.
    fn params(&self) -> &CommonParams { self.machine().params() }

    /// Builtin-contracts for the chain..
    fn builtins(&self) -> &BTreeMap<Address, Box<dyn BuiltinContract>> { self.machine().builtins() }

    /// Attempt to get a handle to a built-in contract.
    /// Only returns references to activated built-ins.
    fn builtin(&self, a: &Address, block_number: BlockNumber) -> Option<&Box<dyn BuiltinContract>> {
        self.machine().builtin(a, block_number)
    }

    /// Some intrinsic operation parameters; by default they take their value from the `spec()`'s `engine_params`.
    fn maximum_extra_data_size(&self) -> usize { self.machine().maximum_extra_data_size() }

    /// The nonce with which accounts begin at given block.
    fn account_start_nonce(&self, block: BlockNumber) -> U256 {
        self.machine().account_start_nonce(block)
    }

    /// Verify a transaction's signature is valid.
    fn verify_transaction_signature(
        &self,
        t: UnverifiedTransaction,
        header: &Header,
    ) -> Result<SignedTransaction, Error>
    {
        self.machine().verify_transaction_signature(t, header)
    }

    /// Additional verification for transactions in blocks.
    // TODO: Add flags for which bits of the transaction to check.
    // TODO: consider including State in the params.
    fn verify_transaction_basic(
        &self,
        t: &UnverifiedTransaction,
        block_num: BlockNumber,
    ) -> Result<(), Error>
    {
        self.machine().verify_transaction_basic(t, Some(block_num))
    }
}
