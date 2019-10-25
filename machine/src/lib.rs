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

#![warn(unused_extern_crates)]
extern crate aion_types;

use aion_types::{H256, U256, Address};

/// A header. This contains important metadata about the block, as well as a
/// "seal" that indicates validity to a consensus engine.
pub trait Header {
    /// Cryptographic hash of the header, excluding the seal.
    fn bare_hash(&self) -> H256;

    /// Cryptographic hash of the header, including the seal.
    fn hash(&self) -> H256;

    /// Get a reference to the seal fields.
    fn seal(&self) -> &[Vec<u8>];

    /// The author of the header.
    fn author(&self) -> &Address;

    /// The number of the header.
    fn number(&self) -> u64;
}

/// A "live" block is one which is in the process of the transition.
/// The state of this block can be mutated by arbitrary rules of the
/// state transition function.
pub trait LiveBlock: 'static {
    /// The block header type;
    type Header: Header;

    /// Get a reference to the header.
    fn header(&self) -> &Self::Header;
}

/// Generalization of types surrounding blockchain-suitable state machines.
pub trait Machine: for<'a> LocalizedMachine<'a> {
    /// The block header type.
    type Header: Header;
    /// The live block type.
    type LiveBlock: LiveBlock<Header = Self::Header>;
    /// A handle to a blockchain client for this machine.
    type EngineClient: ?Sized;
    /// A description of needed auxiliary data.
    type AuxiliaryRequest;
    /// Errors which can occur when querying or interacting with the machine.
    type Error;
}

/// Machine-related types localized to a specific lifetime.
// TODO: this is a workaround for a lack of associated type constructors in the language.
pub trait LocalizedMachine<'a>: Sync + Send {
    /// Definition of auxiliary data associated to a specific block.
    type AuxiliaryData: 'a;
    /// A context providing access to the state in a controlled capacity.
    /// Generally also provides verifiable proofs.
    type StateContext: ?Sized + 'a;
}

/// A state machine that uses balances.
pub trait WithBalances: Machine {
    /// Get the balance, in base units, associated with an account.
    /// Extracts data from the live block.
    fn balance(&self, live: &Self::LiveBlock, address: &Address) -> Result<U256, Self::Error>;

    /// Increment the balance of an account in the state of the live block.
    fn add_balance(
        &self,
        live: &mut Self::LiveBlock,
        address: &Address,
        amount: &U256,
    ) -> Result<(), Self::Error>;

    /// Note block rewards. "direct" rewards are for authors, "indirect" are for e.g. uncles.
    fn note_rewards(
        &self,
        _live: &mut Self::LiveBlock,
        _direct: &[(Address, U256)],
    ) -> Result<(), Self::Error>
    {
        Ok(())
    }
}
