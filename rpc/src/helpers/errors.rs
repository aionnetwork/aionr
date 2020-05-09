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

//! RPC Error codes and error objects
//[FZH] TOREMOVE
#![allow(dead_code)]
use std::fmt;

use acore::account_provider::{SignError as AccountError};
use acore::{Error as EthcoreError, CallError};
use jsonrpc_core::{futures, Error, ErrorCode, Value};
use rlp::DecoderError;
use acore::transaction::Error as TransactionError;

mod codes {
    // NOTE [ToDr] Codes from [-32099, -32000]
    pub const UNSUPPORTED_REQUEST: i64 = -32000;
    pub const NO_WORK: i64 = -32001;
    pub const NO_AUTHOR: i64 = -32002;
    pub const NO_NEW_WORK: i64 = -32003;
    pub const NO_WORK_REQUIRED: i64 = -32004;
    pub const POW_NOT_ALLOWED: i64 = -32005;
    pub const POS_NOT_ALLOWED: i64 = -32006;
    pub const UNKNOWN_ERROR: i64 = -32009;
    pub const TRANSACTION_ERROR: i64 = -32010;
    pub const EXECUTION_ERROR: i64 = -32015;
    pub const EXCEPTION_ERROR: i64 = -32016;
    pub const DATABASE_ERROR: i64 = -32017;
    pub const ACCOUNT_LOCKED: i64 = -32020;
    pub const PASSWORD_INVALID: i64 = -32021;
    pub const FILTER_INVALID: i64 = -32022;
    pub const ACCOUNT_ERROR: i64 = -32023;
    pub const REQUEST_REJECTED: i64 = -32040;
    pub const REQUEST_REJECTED_LIMIT: i64 = -32041;
    pub const REQUEST_NOT_FOUND: i64 = -32042;
    pub const ENCRYPTION_ERROR: i64 = -32055;
    //    pub const ENCODING_ERROR: i64 = -32058;
    pub const FETCH_ERROR: i64 = -32060;
    pub const NO_LIGHT_PEERS: i64 = -32065;
    pub const DEPRECATED: i64 = -32070;
    pub const COMPILATION_FAILED: i64 = -32080;
}

pub fn unimplemented(details: Option<String>) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNSUPPORTED_REQUEST),
        message: "This request is not implemented yet. Please create an issue on Github repo."
            .into(),
        data: details.map(Value::String),
    }
}

pub fn light_unimplemented(details: Option<String>) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNSUPPORTED_REQUEST),
        message: "This request is unsupported for light clients.".into(),
        data: details.map(Value::String),
    }
}

pub fn public_unsupported(details: Option<String>) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNSUPPORTED_REQUEST),
        message: "Method disallowed when running aion as a public node.".into(),
        data: details.map(Value::String),
    }
}

pub fn unsupported<T: Into<String>>(msg: T, details: Option<T>) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNSUPPORTED_REQUEST),
        message: msg.into(),
        data: details.map(Into::into).map(Value::String),
    }
}

pub fn request_not_found() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::REQUEST_NOT_FOUND),
        message: "Request not found.".into(),
        data: None,
    }
}

pub fn request_rejected() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::REQUEST_REJECTED),
        message: "Request has been rejected.".into(),
        data: None,
    }
}

pub fn request_rejected_limit() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::REQUEST_REJECTED_LIMIT),
        message: "Request has been rejected because of queue limit.".into(),
        data: None,
    }
}

pub fn account<T: fmt::Debug>(error: &str, details: T) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::ACCOUNT_ERROR),
        message: error.into(),
        data: Some(Value::String(format!("{:?}", details))),
    }
}

/// Internal error signifying a logic error in code.
/// Should not be used when function can just fail
/// because of invalid parameters or incomplete node state.
pub fn internal<T: fmt::Debug>(error: &str, data: T) -> Error {
    Error {
        code: ErrorCode::InternalError,
        message: format!("Internal error occurred: {}", error),
        data: Some(Value::String(format!("{:?}", data))),
    }
}

pub fn invalid_params<T: fmt::Debug>(param: &str, details: T) -> Error {
    Error {
        code: ErrorCode::InvalidParams,
        message: format!("Couldn't parse parameters: {}", param),
        data: Some(Value::String(format!("{:?}", details))),
    }
}

pub fn execution<T: fmt::Debug>(data: T) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::EXECUTION_ERROR),
        message: "Transaction execution error.".into(),
        data: Some(Value::String(format!("{:?}", data))),
    }
}

pub fn state_pruned() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNSUPPORTED_REQUEST),
        message: "This request is not supported because your node is running with state pruning. \
                  Run with --pruning=archive."
            .into(),
        data: None,
    }
}

pub fn state_corrupt() -> Error { internal("State corrupt", "") }

pub fn exceptional() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::EXCEPTION_ERROR),
        message: "The execution failed due to an exception.".into(),
        data: None,
    }
}

pub fn no_work() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::NO_WORK),
        message: "Still syncing.".into(),
        data: None,
    }
}

pub fn pow_not_allowed() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::POW_NOT_ALLOWED),
        message: "PoW mining not allowed.".into(),
        data: None,
    }
}

pub fn pos_not_allowed() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::POS_NOT_ALLOWED),
        message: "PoS mining not allowed.".into(),
        data: None,
    }
}

pub fn no_new_work() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::NO_NEW_WORK),
        message: "Work has not changed.".into(),
        data: None,
    }
}

pub fn no_author() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::NO_AUTHOR),
        message: "Author not configured. Run Aion with --author to configure.".into(),
        data: None,
    }
}

pub fn no_work_required() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::NO_WORK_REQUIRED),
        message: "External work is only required for Proof of Work engines.".into(),
        data: None,
    }
}

pub fn not_enough_data() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNSUPPORTED_REQUEST),
        message: "The node does not have enough data to compute the given statistic.".into(),
        data: None,
    }
}

pub fn token(e: String) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNKNOWN_ERROR),
        message: "There was an error when saving your authorization tokens.".into(),
        data: Some(Value::String(e)),
    }
}

pub fn signer_disabled() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNSUPPORTED_REQUEST),
        message: "Trusted Signer is disabled. This API is not available.".into(),
        data: None,
    }
}

pub fn dapps_disabled() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNSUPPORTED_REQUEST),
        message: "Dapps Server is disabled. This API is not available.".into(),
        data: None,
    }
}

pub fn ws_disabled() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNSUPPORTED_REQUEST),
        message: "WebSockets Server is disabled. This API is not available.".into(),
        data: None,
    }
}

pub fn network_disabled() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNSUPPORTED_REQUEST),
        message: "Network is disabled or not yet up.".into(),
        data: None,
    }
}

pub fn encryption<T: fmt::Debug>(error: T) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::ENCRYPTION_ERROR),
        message: "Encryption error.".into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

//pub fn encoding<T: fmt::Debug>(error: T) -> Error {
//    Error {
//        code: ErrorCode::ServerError(codes::ENCODING_ERROR),
//        message: "Encoding error.".into(),
//        data: Some(Value::String(format!("{:?}", error))),
//    }
//}

pub fn database<T: fmt::Debug>(error: T) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::DATABASE_ERROR),
        message: "Database error.".into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

pub fn fetch<T: fmt::Debug>(error: T) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::FETCH_ERROR),
        message: "Error while fetching content.".into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

pub fn signing(error: AccountError) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::ACCOUNT_LOCKED),
        message: "Your account is locked. Unlock the account via CLI, personal_unlockAccount or \
                  use Trusted Signer."
            .into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

pub fn password(error: AccountError) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::PASSWORD_INVALID),
        message: "Account password is invalid or account does not exist.".into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

pub fn filter(error: &str) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::FILTER_INVALID),
        message: format!("Filter error occurred: {}", error),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

pub fn transaction_message(error: TransactionError) -> String {
    use self::TransactionError::*;

    match error {
        AlreadyImported => "Transaction with the same hash was already imported.".into(),
        Old => "Transaction nonce is too low. Try to increment the nonce.".into(),
        TooCheapToReplace => {
            "Transaction gas price is too low. There is another transaction with same nonce in the \
             queue. Try to increase the gas price or incrementing the nonce."
                .into()
        }
        LimitReached => {
            "There are too many transactions in the queue. Your transaction was dropped due to \
             limit. Try to increase the fee."
                .into()
        }
        InsufficientGas {
            minimal,
            got,
        } => {
            format!(
                "Transaction gas is too low. There is not enough gas to cover minimal cost of the \
                 transaction (minimal: {}, got: {}). Try to increase supplied gas.",
                minimal, got
            )
        }
        InvalidGasPrice => "Invalid gas price (negative or wrong format).".into(),
        InvalidGasPriceRange {
            minimal,
            maximal,
            got,
        } => {
            format!(
                "Transaction gas price is either too low or too high. It does not satisfy your \
                 node's gas price range configuration (minimal: {}, maximal: {}, got: {}). Try to \
                 adjust the gas price.",
                minimal, maximal, got
            )
        }
        InsufficientBalance {
            balance,
            cost,
        } => {
            format!(
                "Insufficient funds. The account you tried to send transaction from does not have \
                 enough funds. Required {} and got: {}.",
                cost, balance
            )
        }
        GasLimitExceeded {
            limit,
            got,
        } => {
            format!(
                "Transaction gas limit exceeded the gas limit set in configuration. Limit: {}, \
                 got: {}. Try to decrease supplied gas or increase gas limit configuration.",
                limit, got
            )
        }
        InvalidSignature(sig) => format!("Invalid signature: {}", sig),
        InvalidChainId => "Invalid chain id.".into(),
        InvalidGasLimit(_) => "Supplied gas is beyond limit.".into(),
        SenderBanned => "Sender is banned in local queue.".into(),
        RecipientBanned => "Recipient is banned in local queue.".into(),
        CodeBanned => "Code is banned in local queue.".into(),
        NotAllowed => "Transaction is not permitted.".into(),
        InvalidNonceLength => "Transaction nonce length is invalid".into(),
        InvalidTimestampLength => "Transaction timestamp length is invalid".into(),
        InvalidValueLength => "Transaction value length is invalid".into(),
        InvalidContractCreateGas {
            minimal,
            maximal,
            got,
        } => {
            format!(
                "Invalid contract creation gas. Min={}, Max={}, Given={}",
                minimal, maximal, got
            )
        }
        InvalidTransactionGas {
            minimal,
            maximal,
            got,
        } => {
            format!(
                "Invalid transaction gas. Min={}, Max={}, Given={}",
                minimal, maximal, got
            )
        }
        InvalidTransactionType => format!("Invalid transaction type. Expected type = 1 or 2"),
        InvalidBeaconHash(ref hash) => {
            format!(
                "Invalid transaction beacon hash :{}, not in canon chain.",
                hash
            )
        }
        BeaconBanned => "Not yet forked, Beacon hash is banned.".into(),
        FvmDeprecated => "Fvm Create is no longer allowed.".into(),
    }
}

pub fn transaction<T: Into<EthcoreError>>(error: T) -> Error {
    let error = error.into();
    if let EthcoreError::Transaction(e) = error {
        Error {
            code: ErrorCode::ServerError(codes::TRANSACTION_ERROR),
            message: transaction_message(e),
            data: None,
        }
    } else {
        Error {
            code: ErrorCode::ServerError(codes::UNKNOWN_ERROR),
            message: "Unknown error when sending transaction.".into(),
            data: Some(Value::String(format!("{:?}", error))),
        }
    }
}

pub fn rlp(error: DecoderError) -> Error {
    Error {
        code: ErrorCode::InvalidParams,
        message: "Invalid RLP.".into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

pub fn call(error: CallError) -> Error {
    match error {
        CallError::StatePruned => state_pruned(),
        CallError::StateCorrupt => state_corrupt(),
        CallError::Exceptional => exceptional(),
        CallError::Execution(e) => execution(e),
        CallError::TransactionNotFound => {
            internal(
                "{}, this should not be the case with eth_call, most likely a bug.",
                CallError::TransactionNotFound,
            )
        }
        CallError::AVMDecoder(e) => execution(e),
    }
}

pub fn unknown_block() -> Error {
    Error {
        code: ErrorCode::InvalidParams,
        message: "Unknown block number".into(),
        data: None,
    }
}

pub fn no_light_peers() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::NO_LIGHT_PEERS),
        message: "No light peers who can serve data".into(),
        data: None,
    }
}

pub fn deprecated<T: Into<Option<String>>>(message: T) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::DEPRECATED),
        message: "Method deprecated".into(),
        data: message.into().map(Value::String),
    }
}

pub fn compilation_failed<T: Into<Option<String>>>(message: T) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::COMPILATION_FAILED),
        message: "Contracts compilation failed".into(),
        data: message.into().map(Value::String),
    }
}

// on-demand sender cancelled.
pub fn on_demand_cancel(_cancel: futures::sync::oneshot::Canceled) -> Error {
    internal("on-demand sender cancelled", "")
}
