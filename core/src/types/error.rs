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

//! General error types for use in ethcore.

use std::fmt;
use kvdb;
use aion_types::{H256, U256, Address};
use ethbloom::Bloom;
use util_error::UtilError;
use unexpected::{Mismatch, OutOfBounds};
use trie::TrieError;
use io::*;
use header::BlockNumber;
use client::Error as ClientError;
use engine::EngineError;
use key::Error as EthkeyError;
use account_provider::SignError as AccountsError;
use transaction::Error as TransactionError;

pub use types::executed::{ExecutionError, CallError};

#[derive(Debug, PartialEq, Clone, Copy, Eq)]
/// Errors concerning block processing.
pub enum BlockError {
    /// Solution is invalid.
    InvalidSolution,
    /// result is of an invalid length.
    ResultOutOfBounds(OutOfBounds<U256>),
    /// Extra data is of an invalid length.
    ExtraDataOutOfBounds(OutOfBounds<usize>),
    /// Seal is incorrect format.
    InvalidSealArity(Mismatch<usize>),
    /// Block has too much gas used.
    TooMuchGasUsed(OutOfBounds<U256>),
    /// State root header field is invalid.
    InvalidStateRoot(Mismatch<H256>),
    /// Gas used header field is invalid.
    InvalidGasUsed(Mismatch<U256>),
    /// Transactions root header field is invalid.
    InvalidTransactionsRoot(Mismatch<H256>),
    /// Difficulty is out of range; this can be used as an looser error prior to getting a definitive
    /// value for difficulty. This error needs only provide bounds of which it is out.
    DifficultyOutOfBounds(OutOfBounds<U256>),
    /// Difficulty header field is invalid; this is a strong error used after getting a definitive
    /// value for difficulty (which is provided).
    InvalidDifficulty(Mismatch<U256>),
    /// Seal element of type H256 , but could be something else for
    /// other seal engines) is out of bounds.
    MismatchedH256SealElement(Mismatch<H256>),
    /// Proof-of-work aspect of seal, which we assume is a 256-bit value, is invalid.
    InvalidProofOfWork(OutOfBounds<U256>),
    /// Some low-level aspect of the seal is incorrect.
    InvalidSeal,
    /// Gas limit header field is invalid.
    InvalidGasLimit(OutOfBounds<U256>),
    /// Receipts trie root header field is invalid.
    InvalidReceiptsRoot(Mismatch<H256>),
    /// Timestamp header field is invalid.
    InvalidTimestamp(OutOfBounds<u64>),
    /// Timestamp header field is too far in future.
    TemporarilyInvalid(OutOfBounds<u64>),
    /// Log bloom header field is invalid.
    InvalidLogBloom(Mismatch<Bloom>),
    /// Parent hash field of header is invalid; this is an invalid error indicating a logic flaw in the codebase.
    /// TODO: remove and favour an assert!/panic!.
    InvalidParentHash(Mismatch<H256>),
    /// Number field of header is invalid.
    InvalidNumber(Mismatch<BlockNumber>),
    /// Block number isn't sensible.
    RidiculousNumber(OutOfBounds<BlockNumber>),
    /// Too many transactions from a particular address.
    TooManyTransactions(Address),
    /// Parent given is unknown.
    UnknownParent(H256),
    /// No transition to epoch number.
    UnknownEpochTransition(u64),
    /// Invalid pos timestamp
    InvalidPoSTimestamp(u64, u64, u64),
    /// PoS block producer's stake is null or 0
    NullStake,
    /// Invalid PoS block number before the Unity hard fork point
    InvalidPoSBlockNumber,
    /// Invalid PoS block seal type
    InvalidPoSSealType,
    /// Invalid PoS block seed
    InvalidPoSSeed,
    /// Invalid PoS block signature
    InvalidPoSSignature,
    /// Invalid PoS block author
    InvalidPoSAuthor,
    /// Invalid future time stamp
    InvalidFutureTimestamp(OutOfBounds<u64>),
    /// Invalid beacon hash
    InvalidBeaconHash(H256),
    /// Beacon hash is banned
    BeaconHashBanned,
    /// Branch is incomplete
    IncompleteBranch,
    /// Invalid seal type
    InvalidSealType,
}

impl fmt::Display for BlockError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::BlockError::*;

        let msg = match *self {
            InvalidSolution => format!("Invalid solution. "),
            ResultOutOfBounds(ref oob) => {
                format!("Computed output violates boundary condition. {}", oob)
            }
            ExtraDataOutOfBounds(ref oob) => format!("Extra block data too long. {}", oob),
            InvalidSealArity(ref mis) => format!("Block seal in incorrect format: {}", mis),
            TooMuchGasUsed(ref oob) => format!("Block has too much gas used. {}", oob),
            InvalidStateRoot(ref mis) => format!("Invalid state root in header: {}", mis),
            InvalidGasUsed(ref mis) => format!("Invalid gas used in header: {}", mis),
            InvalidTransactionsRoot(ref mis) => {
                format!("Invalid transactions root in header: {}", mis)
            }
            DifficultyOutOfBounds(ref oob) => format!("Invalid block difficulty: {}", oob),
            InvalidDifficulty(ref mis) => format!("Invalid block difficulty: {}", mis),
            MismatchedH256SealElement(ref mis) => format!("Seal element out of bounds: {}", mis),
            InvalidProofOfWork(ref oob) => format!("Block has invalid PoW: {}", oob),
            InvalidSeal => "Block has invalid seal.".into(),
            InvalidGasLimit(ref oob) => format!("Invalid gas limit: {}", oob),
            InvalidReceiptsRoot(ref mis) => {
                format!("Invalid receipts trie root in header: {}", mis)
            }
            InvalidTimestamp(ref oob) => format!("Invalid timestamp in header: {}", oob),
            TemporarilyInvalid(ref oob) => format!("Future timestamp in header: {}", oob),
            InvalidLogBloom(ref oob) => format!("Invalid log bloom in header: {}", oob),
            InvalidParentHash(ref mis) => format!("Invalid parent hash: {}", mis),
            InvalidNumber(ref mis) => format!("Invalid number in header: {}", mis),
            RidiculousNumber(ref oob) => format!("Implausible block number. {}", oob),
            UnknownParent(ref hash) => format!("Unknown parent: {}", hash),
            UnknownEpochTransition(ref num) => {
                format!("Unknown transition to epoch number: {}", num)
            }
            TooManyTransactions(ref address) => format!("Too many transactions from: {}", address),
            InvalidPoSTimestamp(ref timestamp, ref parent_timestamp, ref delta) => {
                format!(
                    "Invalid pos block timestamp {}, parent timestamp {}, expected delta: {}",
                    timestamp, parent_timestamp, delta
                )
            }
            NullStake => "PoS block producer's stake is null or 0.".into(),
            InvalidPoSBlockNumber => "PoS block number is before the unity hard fork point.".into(),
            InvalidPoSSealType => "PoS block's seal type is not pos.".into(),
            InvalidPoSSeed => "PoS block's seed verification failed.".into(),
            InvalidPoSSignature => "PoS block's signature verification failed.".into(),
            InvalidPoSAuthor => {
                "PoS block's author does not match the public key provided in the seal.".into()
            }
            InvalidFutureTimestamp(ref oob) => {
                format!(
                    "Block timestamp is greater than local system time plus tolerance threshold. \
                     {}",
                    oob
                )
            }
            InvalidBeaconHash(ref hash) => {
                format!("Block with invalid transaction beacon hash: {}", hash)
            }
            BeaconHashBanned => "Not yet forked, beacon hash is banned".into(),
            IncompleteBranch => "Cannot trace back beacon hash on an incomplete branch".into(),
            InvalidSealType => {
                "Block's seal type is the same as its parent after Unity fork.".into()
            }
        };

        f.write_fmt(format_args!("Block error ({})", msg))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Import to the block queue result
pub enum ImportError {
    /// Already in the block chain.
    AlreadyInChain,
    /// Already in the block queue.
    AlreadyQueued,
    /// Already marked as bad from a previous import (could mean parent is bad).
    KnownBad,
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match *self {
            ImportError::AlreadyInChain => "block already in chain",
            ImportError::AlreadyQueued => "block already in the block queue",
            ImportError::KnownBad => "block known to be bad",
        };

        f.write_fmt(format_args!("Block import error ({})", msg))
    }
}
/// Error dedicated to import block function
#[derive(Debug)]
pub enum BlockImportError {
    /// Import error
    Import(ImportError),
    /// Block error
    Block(BlockError),
    /// Other error
    Other(String),
}

impl From<Error> for BlockImportError {
    fn from(e: Error) -> Self {
        match e {
            Error::Block(block_error) => BlockImportError::Block(block_error),
            Error::Import(import_error) => BlockImportError::Import(import_error),
            _ => BlockImportError::Other(format!("other block import error: {:?}", e)),
        }
    }
}

/// Api-level error for transaction import
#[derive(Debug, Clone)]
pub enum TransactionImportError {
    /// Transaction error
    Transaction(TransactionError),
    /// Other error
    Other(String),
}

impl From<Error> for TransactionImportError {
    fn from(e: Error) -> Self {
        match e {
            Error::Transaction(transaction_error) => {
                TransactionImportError::Transaction(transaction_error)
            }
            _ => TransactionImportError::Other(format!("other block import error: {:?}", e)),
        }
    }
}

#[derive(Debug)]
/// General error type which should be capable of representing all errors in ethcore.
pub enum Error {
    /// Client configuration error.
    Client(ClientError),
    /// Database error.
    Database(kvdb::Error),
    /// Error concerning a utility.
    Util(UtilError),
    /// Error concerning block processing.
    Block(BlockError),
    /// Unknown engine given.
    UnknownEngineName(String),
    /// Error concerning EVM code execution.
    Execution(ExecutionError),
    /// Error concerning transaction processing.
    Transaction(TransactionError),
    /// Error concerning block import.
    Import(ImportError),
    /// PoW hash is invalid or out of date.
    PowHashInvalid,
    /// The pow seal is invalid.
    PowInvalid,
    /// The pos seal is invalid
    PosInvalid,
    /// Error concerning TrieDBs
    Trie(TrieError),
    /// Io crate error.
    Io(IoError),
    /// Standard io error.
    StdIo(::std::io::Error),
    /// Consensus vote error.
    Engine(EngineError),
    /// Ethkey error.
    Ethkey(EthkeyError),
    /// Account Provider error.
    AccountProvider(AccountsError),
    /// Other error.
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Client(ref err) => err.fmt(f),
            Error::Database(ref err) => err.fmt(f),
            Error::Util(ref err) => err.fmt(f),
            Error::Io(ref err) => err.fmt(f),
            Error::Block(ref err) => err.fmt(f),
            Error::Execution(ref err) => err.fmt(f),
            Error::Transaction(ref err) => err.fmt(f),
            Error::Import(ref err) => err.fmt(f),
            Error::UnknownEngineName(ref name) => {
                f.write_fmt(format_args!("Unknown engine name ({})", name))
            }
            Error::PowHashInvalid => f.write_str("Invalid or out of date PoW hash."),
            Error::PowInvalid => f.write_str("Invalid PoW nonce or mishash"),
            Error::PosInvalid => f.write_str("Invalid PoS seal"),
            Error::Trie(ref err) => err.fmt(f),
            Error::StdIo(ref err) => err.fmt(f),
            Error::Engine(ref err) => err.fmt(f),
            Error::Ethkey(ref err) => err.fmt(f),
            Error::AccountProvider(ref err) => err.fmt(f),
            Error::Other(ref err) => f.write_str(err),
        }
    }
}

/// Result of import block operation.
pub type ImportResult = Result<H256, Error>;

impl From<ClientError> for Error {
    fn from(err: ClientError) -> Error {
        match err {
            ClientError::Trie(err) => Error::Trie(err),
            _ => Error::Client(err),
        }
    }
}

impl From<kvdb::Error> for Error {
    fn from(err: kvdb::Error) -> Error { Error::Database(err) }
}

impl From<TransactionError> for Error {
    fn from(err: TransactionError) -> Error { Error::Transaction(err) }
}

impl From<ImportError> for Error {
    fn from(err: ImportError) -> Error { Error::Import(err) }
}

impl From<BlockError> for Error {
    fn from(err: BlockError) -> Error { Error::Block(err) }
}

impl From<ExecutionError> for Error {
    fn from(err: ExecutionError) -> Error { Error::Execution(err) }
}

impl From<::rlp::DecoderError> for Error {
    fn from(err: ::rlp::DecoderError) -> Error { Error::Util(UtilError::from(err)) }
}

impl From<UtilError> for Error {
    fn from(err: UtilError) -> Error { Error::Util(err) }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Error { Error::Io(err) }
}

impl From<TrieError> for Error {
    fn from(err: TrieError) -> Error { Error::Trie(err) }
}

impl From<::std::io::Error> for Error {
    fn from(err: ::std::io::Error) -> Error { Error::StdIo(err) }
}

impl From<BlockImportError> for Error {
    fn from(err: BlockImportError) -> Error {
        match err {
            BlockImportError::Block(e) => Error::Block(e),
            BlockImportError::Import(e) => Error::Import(e),
            BlockImportError::Other(s) => Error::Other(s),
        }
    }
}

impl From<EngineError> for Error {
    fn from(err: EngineError) -> Error { Error::Engine(err) }
}

impl From<EthkeyError> for Error {
    fn from(err: EthkeyError) -> Error { Error::Ethkey(err) }
}

impl From<AccountsError> for Error {
    fn from(err: AccountsError) -> Error { Error::AccountProvider(err) }
}

impl<E> From<Box<E>> for Error
where Error: From<E>
{
    fn from(err: Box<E>) -> Error { Error::from(*err) }
}
