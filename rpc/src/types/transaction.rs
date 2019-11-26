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

use acore::contract_address;
use acore::transaction::{LocalizedTransaction, Action, PendingTransaction, SignedTransaction};
use aion_types::{H256, U256};
use bytes::u64_to_bytes;
use serde::ser::{Serialize, Serializer, SerializeStruct};

use types::{Bytes, TransactionCondition};

/// Transaction
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Transaction {
    /// Hash
    pub hash: H256,
    /// Nonce
    pub nonce: U256,
    /// Block hash
    pub block_hash: Option<H256>,
    /// Block number
    pub block_number: Option<U256>,
    /// Transaction Index
    pub transaction_index: Option<U256>,
    /// Sender
    pub from: H256,
    /// Recipient
    pub to: Option<H256>,
    /// Transfered value
    pub value: U256,
    /// Gas Price
    pub gas_price: U256,
    /// Gas
    pub gas: U256,
    /// Data
    pub input: Bytes,
    /// Creates contract
    pub creates: Option<H256>,
    /// Raw transaction data
    pub raw: Bytes,
    /// Public key of the signer.
    pub public_key: Option<H256>,
    /// The standardised V field of the signature (0 or 1).
    pub standard_v: U256,
    /// Signature.
    pub sig: Bytes,
    /// Transaction activates at specified block.
    pub condition: Option<TransactionCondition>,
    /// Timestamp
    pub timestamp: Bytes,
    /// beacon
    pub beacon: Option<H256>,
}

impl Serialize for Transaction {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let mut transaction = serializer.serialize_struct("Transaction", 15)?;
        transaction.serialize_field("hash", &self.hash)?;
        transaction.serialize_field("nonce", &(self.nonce.low_u64()))?;
        transaction.serialize_field("blockHash", &self.block_hash)?;
        transaction.serialize_field("blockNumber", &self.block_number)?;
        if self.transaction_index.is_some() {
            transaction.serialize_field(
                "transactionIndex",
                &self.transaction_index.unwrap().low_u64(),
            )?;
        } else {
            transaction.serialize_field("transactionIndex", &self.transaction_index)?;
        }
        transaction.serialize_field("to", &self.to)?;
        transaction.serialize_field("from", &self.from)?;
        transaction.serialize_field("value", &self.value)?;
        transaction.serialize_field("gasPrice", &self.gas_price)?;
        transaction.serialize_field("gas", &self.gas.low_u64())?;
        transaction.serialize_field("nrgPrice", &self.gas_price)?;
        transaction.serialize_field("nrg", &self.gas.low_u64())?;
        transaction.serialize_field("input", &self.input)?;
        transaction.serialize_field("contractAddress", &self.creates)?;
        // do not serialize raw
        // do not serialize public key
        // do not serialize chain id
        // do not serialize standard v
        // do not serialize sig
        // do not serialize condition
        transaction.serialize_field(
            "timestamp",
            &U256::from_big_endian(self.timestamp.0.as_slice()).low_u64(),
        )?;
        if let Some(beacon) = &self.beacon {
            transaction.serialize_field("beacon", beacon)?;
        }
        transaction.end()
    }
}

/// Geth-compatible output for eth_signTransaction method
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct RichRawTransaction {
    /// Raw transaction RLP
    pub raw: Bytes,
    /// Transaction details
    #[serde(rename = "tx")]
    pub transaction: Transaction,
}

impl RichRawTransaction {
    /// Creates new `RichRawTransaction` from `SignedTransaction`.
    pub fn from_signed(tx: SignedTransaction) -> Self {
        let tx = Transaction::from_signed(tx);
        RichRawTransaction {
            raw: tx.raw.clone(),
            transaction: tx,
        }
    }
}

impl Transaction {
    /// Convert `LocalizedTransaction` into RPC Transaction.
    pub fn from_localized(mut t: LocalizedTransaction, timestamp: u64) -> Transaction {
        let signature = t.signature();
        Transaction {
            hash: t.hash().clone(),
            nonce: t.nonce,
            block_hash: Some(t.block_hash.clone()),
            block_number: Some(t.block_number.into()),
            transaction_index: Some(t.transaction_index.into()),
            from: t.sender().clone(),
            to: match t.action {
                Action::Create => None,
                Action::Call(ref address) => Some(address.clone()),
            },
            value: t.value,
            gas_price: t.gas_price,
            gas: t.gas,
            input: Bytes::new(t.data.clone()),
            creates: match t.action {
                Action::Create => Some(contract_address(&t.sender(), &t.nonce).0),
                Action::Call(_) => None,
            },
            raw: ::rlp::encode(&t.signed).into_vec().into(),
            public_key: {
                let mut pk = [0u8; 32];
                // since local Transaction has been verified, it must be able to recover public key.
                pk.copy_from_slice(&t.recover_public().unwrap().0);
                Some(H256(pk))
            },
            standard_v: t.standard_v().into(),
            sig: Bytes::new(signature.to_vec()),
            condition: None,
            timestamp: Bytes::new(u64_to_bytes(timestamp)),
            beacon: t.beacon.clone(),
        }
    }

    /// Convert `SignedTransaction` into RPC Transaction.
    pub fn from_signed(t: SignedTransaction) -> Transaction {
        let signature = t.signature();
        Transaction {
            hash: t.hash().clone(),
            nonce: t.nonce,
            block_hash: None,
            block_number: None,
            transaction_index: None,
            from: t.sender().clone(),
            to: match t.action {
                Action::Create => None,
                Action::Call(ref address) => Some(address.clone()),
            },
            value: t.value,
            gas_price: t.gas_price,
            gas: t.gas,
            input: Bytes::new(t.data.clone()),
            creates: match t.action {
                Action::Create => Some(contract_address(&t.sender(), &t.nonce).0),
                Action::Call(_) => None,
            },
            raw: ::rlp::encode(&t).into_vec().into(),
            public_key: t.public_key(),

            standard_v: t.standard_v().into(),
            sig: Bytes::new(signature.clone().to_vec()),
            condition: None,
            timestamp: Bytes::new(t.timestamp().clone()),
            beacon: t.beacon,
        }
    }

    /// Convert `PendingTransaction` into RPC Transaction.
    pub fn from_pending(t: PendingTransaction) -> Transaction {
        let mut r = Transaction::from_signed(t.transaction);
        r.condition = t.condition.map(|b| b.into());
        r
    }
}

#[cfg(test)]
mod tests {
    use super::Transaction;
    use serde_json;

    #[test]
    fn test_transaction_serialize() {
        let t = Transaction::default();
        let serialized = serde_json::to_string(&t).unwrap();
        assert_eq!(serialized, r#"{"hash":"0x0000000000000000000000000000000000000000000000000000000000000000","nonce":0,"blockHash":null,"blockNumber":null,"transactionIndex":null,"to":null,"from":"0x0000000000000000000000000000000000000000000000000000000000000000","value":"0x0","gasPrice":"0x0","gas":0,"nrgPrice":"0x0","nrg":0,"input":"0x","contractAddress":null,"timestamp":0}"#);
    }

    #[test]
    fn test_transaction_serialize2() {
        let mut t = Transaction::default();
        t.to = None;
        let serialized = serde_json::to_string(&t).unwrap();
        println!("value: {} ", serialized);
    }
}
