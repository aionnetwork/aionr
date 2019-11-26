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

//! Types used in Confirmations queue (Trusted Signer)

use std::fmt;
use serde::{Serialize, Serializer};
use ansi_term::Colour;
use bytes::ToPretty;
use aion_types::{H256, H520, H768, Address};

use types::{
    TransactionRequest, RichRawTransaction, Bytes,
};
use helpers;

impl fmt::Display for ConfirmationPayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ConfirmationPayload::SendTransaction(ref transaction) => write!(f, "{}", transaction),
            ConfirmationPayload::SignTransaction(ref transaction) => {
                write!(f, "(Sign only) {}", transaction)
            }
            ConfirmationPayload::EthSignMessage(ref sign) => write!(f, "{}", sign),
            //ConfirmationPayload::Decrypt(ref decrypt) => write!(f, "{}", decrypt),
        }
    }
}

/// Sign request
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignRequest {
    /// Address
    pub address: Address,
    /// Hash to sign
    pub data: Bytes,
}

impl From<(Address, Bytes)> for SignRequest {
    fn from(tuple: (Address, Bytes)) -> Self {
        SignRequest {
            address: tuple.0,
            data: tuple.1,
        }
    }
}

impl fmt::Display for SignRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "sign 0x{} with {}",
            self.data.0.pretty(),
            Colour::White.bold().paint(format!("0x{:?}", self.address)),
        )
    }
}

/// Confirmation response for particular payload
#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmationResponse {
    /// Transaction Hash
    SendTransaction(H256),
    /// Transaction RLP
    SignTransaction(RichRawTransaction),
    /// Signature (encoded as VRS)
    Signature(H520),
    SignatureEd25519(H768),
    /// Decrypted data
    Decrypt(Bytes),
}

impl Serialize for ConfirmationResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        match *self {
            ConfirmationResponse::SendTransaction(ref hash) => hash.serialize(serializer),
            ConfirmationResponse::SignTransaction(ref rlp) => rlp.serialize(serializer),
            ConfirmationResponse::Signature(ref signature) => signature.serialize(serializer),
            ConfirmationResponse::Decrypt(ref data) => data.serialize(serializer),
            ConfirmationResponse::SignatureEd25519(ref signature) => {
                signature.serialize(serializer)
            }
        }
    }
}

/// Confirmation payload, i.e. the thing to be confirmed
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ConfirmationPayload {
    /// Send Transaction
    #[serde(rename = "sendTransaction")]
    SendTransaction(TransactionRequest),
    /// Sign Transaction
    #[serde(rename = "signTransaction")]
    SignTransaction(TransactionRequest),
    /// Signature
    #[serde(rename = "sign")]
    EthSignMessage(SignRequest),
    //    /// Decryption
    //    #[serde(rename = "decrypt")]
    //    Decrypt(DecryptRequest),
}

impl From<helpers::ConfirmationPayload> for ConfirmationPayload {
    fn from(c: helpers::ConfirmationPayload) -> Self {
        match c {
            helpers::ConfirmationPayload::SendTransaction(t) => {
                ConfirmationPayload::SendTransaction(t.into())
            }
            helpers::ConfirmationPayload::SignTransaction(t) => {
                ConfirmationPayload::SignTransaction(t.into())
            }
            helpers::ConfirmationPayload::EthSignMessage(address, data) => {
                ConfirmationPayload::EthSignMessage(SignRequest {
                    address: address.into(),
                    data: data.into(),
                })
            } //            helpers::ConfirmationPayload::Decrypt(address, msg) => {
              //                ConfirmationPayload::Decrypt(DecryptRequest {
              //                    address: address.into(),
              //                    msg: msg.into(),
              //                })
              //            }
        }
    }
}
