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

//! Transaction data structure.

use super::error;
use aion_types::{to_u256, Address, Ed25519Public, H256, U256};
use ajson;
use blake2b::blake2b;
use heapsize::HeapSizeOf;
use key::{
    self, public_to_address_ed25519, recover_ed25519, sign_ed25519, Ed25519Secret, Ed25519Signature,
};
use rlp::{self, DecoderError, Encodable, RlpStream, UntrustedRlp};
use std::ops::Deref;
use vms::constants::{
    GAS_CALL_MAX, GAS_CALL_MIN, GAS_CREATE_MAX, GAS_CREATE_MIN, GAS_TX_DATA_NONZERO,
    GAS_TX_DATA_ZERO,
};

use bytes::i64_to_bytes;
use trace_time::to_epoch_micro;

type Bytes = Vec<u8>;
type BlockNumber = u64;

/// Fake address for unsigned transactions as defined by EIP-86.
pub const UNSIGNED_SENDER: Address = H256([0xff; 32]);

/// System sender address for internal state updates.
pub const SYSTEM_ADDRESS: Address = H256([
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
]);

pub const DEFAULT_TRANSACTION_TYPE: U256 = U256([1, 0, 0, 0]);
pub const AVM_TRANSACTION_TYPE: U256 = U256([2, 0, 0, 0]);

struct TransactionEnergyRule;
impl TransactionEnergyRule {
    pub fn is_valid_gas_create(gas: U256) -> bool {
        (gas >= GAS_CREATE_MIN + GAS_CALL_MIN) && (gas <= GAS_CREATE_MAX)
    }

    pub fn is_valid_gas_call(gas: U256) -> bool { gas >= GAS_CALL_MIN && gas <= GAS_CALL_MAX }
}

/// Transaction action type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Create creates new contract.
    Create,
    /// Calls contract at given address.
    /// In the case of a transfer, this is the receiver's address.'
    Call(Address),
}

impl Default for Action {
    fn default() -> Action { Action::Create }
}

impl rlp::Decodable for Action {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.is_empty() {
            Ok(Action::Create)
        } else {
            Ok(Action::Call(rlp.as_val()?))
        }
    }
}

impl rlp::Encodable for Action {
    fn rlp_append(&self, s: &mut RlpStream) {
        match *self {
            Action::Create => s.append_internal(&""),
            Action::Call(ref addr) => s.append_internal(addr),
        };
    }
}

/// Transaction activation condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// Valid at this block number or later.
    Number(BlockNumber),
    /// Valid at this unix time or later.
    Timestamp(u64),
}

/// A set of information describing an externally-originating message call
/// or contract creation operation.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Nonce.
    pub nonce: U256,
    pub nonce_bytes: Bytes,
    /// Gas price.
    pub gas_price: U256,
    pub gas_price_bytes: Bytes,
    /// Gas paid up front for transaction execution.
    pub gas: U256,
    pub gas_bytes: Bytes,
    /// Action, can be either call or contract create.
    pub action: Action,
    /// Transfered value.
    pub value: U256,
    pub value_bytes: Bytes,
    /// Transaction data.
    pub data: Bytes,
    /// Transaction Type.
    pub transaction_type: U256,
}

impl Transaction {
    pub fn new(
        nonce: U256,
        gas_price: U256,
        gas: U256,
        action: Action,
        value: U256,
        data: Bytes,
        tx_type: U256,
    ) -> Transaction
    {
        Transaction {
            nonce,
            nonce_bytes: Bytes::new(),
            gas_price,
            gas_price_bytes: Bytes::new(),
            gas,
            gas_bytes: Bytes::new(),
            action,
            value,
            value_bytes: Bytes::new(),
            data,
            transaction_type: tx_type,
        }
    }

    /// Append object with a without signature into RLP stream
    pub fn rlp_append_unsigned_transaction(
        &self,
        s: &mut RlpStream,
        chain_id: Option<u64>,
        timestamp: &Bytes,
    )
    {
        s.begin_list(if chain_id.is_none() { 8 } else { 11 });
        if self.nonce_bytes.is_empty() {
            s.append(&self.nonce);
        } else {
            s.append(&self.nonce_bytes);
        }
        s.append(&self.action);
        if self.value_bytes.is_empty() {
            s.append(&self.value);
        } else {
            s.append(&self.value_bytes);
        }
        s.append(&self.data);
        s.append(timestamp);
        if self.gas_bytes.is_empty() {
            encode_long(&self.gas, s);
        } else {
            s.append(&self.gas_bytes);
        }
        if self.gas_price_bytes.is_empty() {
            encode_long(&self.gas_price, s);
        } else {
            s.append(&self.gas_price_bytes);
        }
        s.append(&self.transaction_type);
        if let Some(n) = chain_id {
            s.append(&n);
            s.append(&0u8);
            s.append(&0u8);
        }
    }
}

impl HeapSizeOf for Transaction {
    fn heap_size_of_children(&self) -> usize { self.data.heap_size_of_children() }
}

impl From<ajson::transaction::Transaction> for UnverifiedTransaction {
    fn from(t: ajson::transaction::Transaction) -> Self {
        let to: Option<ajson::hash::Address> = t.to.into();
        UnverifiedTransaction {
            unsigned: Transaction {
                nonce: t.nonce.into(),
                nonce_bytes: Vec::new(),
                gas_price: t.gas_price.into(),
                gas_price_bytes: Vec::new(),
                gas: t.gas_limit.into(),
                gas_bytes: Vec::new(),
                action: match to {
                    Some(to) => Action::Call(to.into()),
                    None => Action::Create,
                },
                value: t.value.into(),
                value_bytes: Vec::new(),
                data: t.data.into(),
                transaction_type: t.transaction_type.into(),
            },
            timestamp: t.timestamp.into(),
            sig: t.sig.into(),
            hash: 0.into(),
        }
        .compute_hash()
    }
}

impl Transaction {
    /// The message hash of the transaction.
    pub fn hash(&self, chain_id: Option<u64>, timestamp: &Bytes) -> H256 {
        let mut stream = RlpStream::new();
        self.rlp_append_unsigned_transaction(&mut stream, chain_id, timestamp);
        blake2b(stream.as_raw())
    }

    pub fn sign(self, key: &[u8], chain_id: Option<u64>) -> SignedTransaction {
        let timestamp = i64_to_bytes(to_epoch_micro());
        //        let sig = sign_with_secret(secret_from_slice(key), &self.hash(chain_id, &timestamp))
        //            .expect("data is valid and context has signing capabilities; qed");
        let key = Ed25519Secret::from_slice(key)
            .expect("key is valid and context has signing capabilities; qed");
        let sig = sign_ed25519(&key, &self.hash(chain_id, &timestamp))
            .expect("data is valid and context has signing capabilities; qed");
        SignedTransaction::new(self.with_signature(sig, chain_id, timestamp.to_vec()))
            .expect("secret is valid so it's recoverable")
    }

    pub fn with_signature(
        self,
        sig: Ed25519Signature,
        _chain_id: Option<u64>,
        timestamp: Bytes,
    ) -> UnverifiedTransaction
    {
        UnverifiedTransaction {
            unsigned: self,
            timestamp,
            sig: sig.to_vec(),
            hash: 0.into(),
        }
        .compute_hash()
    }

    /// Useful for test incorrectly signed transactions.
    #[cfg(test)]
    pub fn invalid_sign(self) -> UnverifiedTransaction {
        UnverifiedTransaction {
            unsigned: self,
            timestamp: vec![0x00; 8],
            sig: vec![0u8; 96],
            hash: 0.into(),
        }
        .compute_hash()
    }

    /// Specify the sender; this won't survive the serialize/deserialize process, but can be cloned.
    pub fn fake_sign(self, from: Address) -> SignedTransaction {
        SignedTransaction {
            transaction: UnverifiedTransaction {
                unsigned: self,
                timestamp: vec![0x00; 8],
                sig: vec![1u8; 96],
                hash: 0.into(),
            }
            .compute_hash(),
            sender: from,
            public: None,
        }
    }

    /// Add EIP-86 compatible empty signature.
    pub fn null_sign(self, _chain_id: u64) -> SignedTransaction {
        SignedTransaction {
            transaction: UnverifiedTransaction {
                unsigned: self,
                timestamp: vec![0x00; 8],
                sig: vec![0u8; 96],
                hash: 0.into(),
            }
            .compute_hash(),
            sender: UNSIGNED_SENDER,
            public: None,
        }
    }

    /// Get the transaction cost in gas for the given params.
    pub fn gas_required(&self) -> U256 {
        self.data.iter().fold(
            match self.action {
                Action::Create => GAS_CREATE_MIN,
                Action::Call(_) => 0.into(),
            } + GAS_CALL_MIN,
            |g, b| {
                g + match *b {
                    0 => GAS_TX_DATA_ZERO,
                    _ => GAS_TX_DATA_NONZERO,
                }
            },
        )
    }
}

/// Signed transaction information without verified signature.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnverifiedTransaction {
    /// Plain Transaction.
    unsigned: Transaction,
    /// Signature
    sig: Bytes,
    /// Hash of the transaction
    hash: H256,
    /// Timestamp.
    /// It is a 8-bytes array shown the time of the transaction signed by the kernel, the unit is nanosecond.
    timestamp: Bytes,
}

impl Deref for UnverifiedTransaction {
    type Target = Transaction;

    fn deref(&self) -> &Self::Target { &self.unsigned }
}

impl rlp::Decodable for UnverifiedTransaction {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        if d.item_count()? != 9 {
            return Err(DecoderError::RlpIncorrectListLen);
        }
        let hash = blake2b(d.as_raw());
        Ok(UnverifiedTransaction {
            unsigned: Transaction {
                nonce: {
                    let raw = d.val_at::<Vec<u8>>(0)?;
                    if raw.len() > 32 {
                        return Err(DecoderError::Custom("transaction nonce bytes > 32"));
                    } else {
                        U256::from_big_endian(raw.as_slice())
                    }
                },
                nonce_bytes: d.val_at(0)?,
                action: d.val_at(1)?,
                value: {
                    let raw = d.val_at::<Vec<u8>>(2)?;
                    if raw.len() > 32 {
                        return Err(DecoderError::Custom("transaction value bytes > 32"));
                    } else {
                        U256::from_big_endian(raw.as_slice())
                    }
                },
                value_bytes: d.val_at(2)?,
                data: d.val_at(3)?,
                gas: to_u256(d.val_at::<Vec<u8>>(5)?, 8),
                gas_bytes: d.val_at(5)?,
                gas_price: to_u256(d.val_at::<Vec<u8>>(6)?, 8),
                gas_price_bytes: d.val_at(6)?,
                transaction_type: {
                    let transaction_type_vec = d.val_at::<Vec<u8>>(7)?;
                    if transaction_type_vec.len() == 0 {
                        0u8.into()
                    } else {
                        transaction_type_vec[0].into()
                    }
                },
            },
            timestamp: d.val_at(4)?,
            sig: d.val_at(8)?,
            hash: hash,
        })
    }
}

impl rlp::Encodable for UnverifiedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) { self.rlp_append_sealed_transaction(s) }
}

impl UnverifiedTransaction {
    /// Used to compute hash of created transactions
    fn compute_hash(mut self) -> UnverifiedTransaction {
        let hash = blake2b(&*self.rlp_bytes());
        self.hash = hash;
        self
    }

    /// Checks is signature is empty.
    pub fn is_unsigned(&self) -> bool {
        for i in &self.sig {
            if *i != 0 {
                return false;
            }
        }
        true
    }

    /// Append object with a signature into RLP stream
    fn rlp_append_sealed_transaction(&self, s: &mut RlpStream) {
        s.begin_list(9);
        if self.nonce_bytes.is_empty() {
            s.append(&self.nonce);
        } else {
            s.append(&self.nonce_bytes);
        }
        s.append(&self.action);
        if self.value_bytes.is_empty() {
            s.append(&self.value);
        } else {
            s.append(&self.value_bytes);
        }
        s.append(&self.data);
        s.append(&self.timestamp);
        if self.gas_bytes.is_empty() {
            encode_long(&self.gas, s);
        } else {
            s.append(&self.gas_bytes);
        }
        if self.gas_price_bytes.is_empty() {
            encode_long(&self.gas_price, s);
        } else {
            s.append(&self.gas_price_bytes);
        }
        s.append(&self.transaction_type);
        s.append(&self.sig);
    }

    ///    Reference to unsigned part of this transaction.
    pub fn as_unsigned(&self) -> &Transaction { &self.unsigned }

    pub fn standard_v(&self) -> u8 { 0 }

    /// The chain ID, or `None` if this is a global transaction.
    pub fn chain_id(&self) -> Option<u64> { None }

    /// Construct a signature object from the sig.
    pub fn signature(&self) -> Ed25519Signature { Ed25519Signature::from(self.sig.clone()) }

    /// Get the hash of this header (blake2b of the RLP).
    pub fn hash(&self) -> H256 { self.hash }

    /// Get the timestamp
    pub fn timestamp(&self) -> &Bytes { &self.timestamp }

    /// Recovers the public key of the sender.
    pub fn recover_public(&self) -> Result<Ed25519Public, key::Error> {
        recover_ed25519(
            &self.signature(),
            &self.unsigned.hash(self.chain_id(), &self.timestamp),
        )
    }

    /// Do basic validation, checking for valid signature and minimum gas,
    // TODO: consider use in block validation.
    // TODO-aion: add other validation as java version does.
    #[cfg(feature = "json-tests")]
    pub fn validate(
        self,
        allow_chain_id_of_one: bool,
        allow_empty_signature: bool,
    ) -> Result<UnverifiedTransaction, error::Error>
    {
        let chain_id = if allow_chain_id_of_one { Some(1) } else { None };
        self.verify_basic(chain_id)?;
        if !allow_empty_signature || !self.is_unsigned() {
            self.recover_public()?;
        }
        if self.gas < self.gas_required() {
            return Err(error::Error::InvalidGasLimit(::unexpected::OutOfBounds {
                min: Some(self.gas_required()),
                max: None,
                found: self.gas,
            })
            .into());
        }
        Ok(self)
    }

    /// Verify basic signature params. Does not attempt sender recovery.
    pub fn verify_basic(&self, _chain_id: Option<u64>) -> Result<(), error::Error> {
        // verify nonce length
        if self.unsigned.nonce.leading_zeros() / 8 < 16 {
            return Err(error::Error::InvalidNonceLength);
        }

        // verify timestamp length
        if self.timestamp.len() > 8 {
            return Err(error::Error::InvalidTimestampLength);
        }

        // verify value length
        if self.unsigned.value.leading_zeros() / 8 < 16 {
            return Err(error::Error::InvalidValueLength);
        }

        // verify energy
        match &self.unsigned.action {
            Action::Create => {
                if !TransactionEnergyRule::is_valid_gas_create(self.gas) {
                    return Err(error::Error::InvalidContractCreateGas {
                        minimal: GAS_CREATE_MIN + GAS_CALL_MIN,
                        maximal: GAS_CREATE_MAX,
                        got: self.gas,
                    });
                }
            }
            Action::Call(_) => {
                if !TransactionEnergyRule::is_valid_gas_call(self.gas) {
                    return Err(error::Error::InvalidTransactionGas {
                        minimal: GAS_CALL_MIN,
                        maximal: GAS_CALL_MAX,
                        got: self.gas,
                    });
                }
            }
        }

        // verify energy price
        if self.gas_price < U256::zero() || self.gas_price.leading_zeros() / 8 < 16 {
            return Err(error::Error::InvalidGasPrice);
        }

        // verify sig length
        if self.sig.len() != 96 {
            return Err(error::Error::InvalidSignature(format!(
                "signature length is invalid: {}",
                self.sig.len()
            )));
        }

        // signature is verified in SignedTransaction.

        Ok(())
    }
}

/// A `UnverifiedTransaction` with successfully recovered `sender`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignedTransaction {
    transaction: UnverifiedTransaction,
    sender: Address,
    public: Option<Ed25519Public>,
}

impl HeapSizeOf for SignedTransaction {
    fn heap_size_of_children(&self) -> usize { self.transaction.unsigned.heap_size_of_children() }
}

impl rlp::Encodable for SignedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) { self.transaction.rlp_append_sealed_transaction(s) }
}

impl Deref for SignedTransaction {
    type Target = UnverifiedTransaction;
    fn deref(&self) -> &Self::Target { &self.transaction }
}

impl From<SignedTransaction> for UnverifiedTransaction {
    fn from(tx: SignedTransaction) -> Self { tx.transaction }
}

impl SignedTransaction {
    /// Try to verify transaction and recover sender.
    pub fn new(transaction: UnverifiedTransaction) -> Result<Self, key::Error> {
        if transaction.is_unsigned() {
            Ok(SignedTransaction {
                transaction: transaction,
                sender: UNSIGNED_SENDER,
                public: None,
            })
        } else {
            let public = transaction.recover_public()?;
            let sender = public_to_address_ed25519(&public);
            Ok(SignedTransaction {
                transaction: transaction,
                sender: sender,
                public: Some(H256::from_slice(&public.0)),
            })
        }
    }

    /// Returns transaction type.
    pub fn tx_type(&self) -> U256 { self.transaction.unsigned.transaction_type }

    /// Returns transaction sender.
    pub fn sender(&self) -> Address { self.sender }

    /// Returns a public key of the sender.
    pub fn public_key(&self) -> Option<Ed25519Public> { self.public }

    /// Checks is signature is empty.
    pub fn is_unsigned(&self) -> bool { self.transaction.is_unsigned() }

    /// Deconstructs this transaction back into `UnverifiedTransaction`
    pub fn deconstruct(self) -> (UnverifiedTransaction, Address, Option<Ed25519Public>) {
        (self.transaction, self.sender, self.public)
    }
}

/// Signed Transaction that is a part of canon blockchain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedTransaction {
    /// Signed part.
    pub signed: UnverifiedTransaction,
    /// Block number.
    pub block_number: BlockNumber,
    /// Block hash.
    pub block_hash: H256,
    /// Transaction index within block.
    pub transaction_index: usize,
    /// Cached sender
    pub cached_sender: Option<Address>,
}

impl LocalizedTransaction {
    /// Returns transaction sender.
    /// Panics if `LocalizedTransaction` is constructed using invalid `UnverifiedTransaction`.
    pub fn sender(&mut self) -> Address {
        if let Some(sender) = self.cached_sender {
            return sender;
        }
        if self.is_unsigned() {
            return UNSIGNED_SENDER.clone();
        }
        let sender = public_to_address_ed25519(&self.recover_public().expect(
            "LocalizedTransaction is always constructed from transaction from blockchain; \
             Blockchain only stores verified transactions; qed",
        ));
        self.cached_sender = Some(sender);
        sender
    }
}

impl Deref for LocalizedTransaction {
    type Target = UnverifiedTransaction;

    fn deref(&self) -> &Self::Target { &self.signed }
}

/// Queued transaction with additional information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTransaction {
    /// Signed transaction data.
    pub transaction: SignedTransaction,
    /// To be activated at this condition. `None` for immediately.
    pub condition: Option<Condition>,
}

impl PendingTransaction {
    /// Create a new pending transaction from signed transaction.
    pub fn new(signed: SignedTransaction, condition: Option<Condition>) -> Self {
        PendingTransaction {
            transaction: signed,
            condition: condition,
        }
    }
}

impl Deref for PendingTransaction {
    type Target = SignedTransaction;

    fn deref(&self) -> &SignedTransaction { &self.transaction }
}

impl From<SignedTransaction> for PendingTransaction {
    fn from(t: SignedTransaction) -> Self {
        PendingTransaction {
            transaction: t,
            condition: None,
        }
    }
}

fn encode_long(value: &U256, s: &mut RlpStream) {
    let mut val_bytes = Vec::new();
    if value > &U256::from(0xFFFFFFFFu64) {
        let val = H256::from(value);
        for i in 24..32 {
            val_bytes.push(val[i]);
        }
        s.append(&val_bytes);
    } else {
        s.append(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aion_types::U256;
    use blake2b::blake2b;
    use rustc_hex::*;

    #[test]
    fn sender_test() {
        let t: UnverifiedTransaction = rlp::decode(&::rustc_hex::FromHex::from_hex("f89480a0a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e6340a80880005748d2d576db18252080101b8608bc5c4e5599afac7cb0efcb0010540017dda3e80870bb543b356867b2a8cacbf7447205c145d35c2a4e6bd58e58e5894b37416247ed0330f4bb114984f869aa3ed914130be741856a664439cbd64d5583e85dd470ea448c8fc9102b2116c2a0a").unwrap());
        assert_eq!(t.data, b"");
        assert_eq!(t.gas, U256::from(0x5208u64));
        assert_eq!(t.gas_price, U256::from(0x01u64));
        assert_eq!(t.nonce, U256::from(0x00u64));

        if let Action::Call(ref to) = t.action {
            assert_eq!(
                *to,
                "a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e634".into()
            );
        } else {
            panic!();
        }
        assert_eq!(t.value, U256::from(0x0au64));
        assert_eq!(
            public_to_address_ed25519(&t.recover_public().unwrap()),
            "a00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2c".into()
        );
        assert_eq!(t.chain_id(), None);
    }

    // verify_basic() tests

    #[test]
    fn test_verify_basic_success() {
        let mut t: UnverifiedTransaction = rlp::decode(&::rustc_hex::FromHex::from_hex("f87c80800184646174618800057a9d04e38ebe83030d408398968001b860fdc74311a02604a1171e984d64363a5c6073b7dff9e063d1c2eee84f7364021bbea5d2484bc0adc48f4eff40d2d41ab142f38cc66c7df9051792f03196042dd4d8d200f595f5775bd66417d26ef79e2d2bd8ec6faed5f35f28422c9b3c36f700").unwrap());
        // assert!(t.verify_basic(Some(0)).is_err());
        t.unsigned.gas = U256::from(221000);
        assert!(t.verify_basic(Some(0)).is_ok());
    }

    #[test]
    fn test_verify_basic_invalid_signature() {
        let t = Transaction {
            action: Action::Call(SYSTEM_ADDRESS),
            nonce: U256::from(1),
            nonce_bytes: Vec::new(),
            gas_price: U256::from(20000),
            gas_bytes: Vec::new(),
            gas: U256::from(22000),
            gas_price_bytes: Vec::new(),
            value: U256::from(0),
            value_bytes: Vec::new(),
            data: ::rustc_hex::FromHex::from_hex("26121ff0").unwrap(),
            transaction_type: U256::from(1),
        };

        let ut = UnverifiedTransaction {
            unsigned: t,
            sig: Vec::new(),
            hash: H256::zero(),
            timestamp: Vec::new(),
        };
        let r = ut.verify_basic(Some(0));
        assert!(r.is_err());
        let e = r.err().unwrap();
        match e {
            error::Error::InvalidSignature(_) => {}
            _ => assert!(false),
        }
        println!("test_verify_basic_invalid_signature error={}", e);
    }

    #[test]
    fn test_verify_basic_invalid_gas_price() {
        let t = Transaction {
            action: Action::Call(SYSTEM_ADDRESS),
            nonce: U256::from(1),
            nonce_bytes: Vec::new(),
            gas_price: U256::from("11111111111111111111111111111111111111111111111111111111111111"),
            gas_bytes: Vec::new(),
            gas: U256::from(22000),
            gas_price_bytes: Vec::new(),
            value: U256::from(0),
            value_bytes: Vec::new(),
            data: ::rustc_hex::FromHex::from_hex("26121ff0").unwrap(),
            transaction_type: U256::from(1),
        };

        let ut = UnverifiedTransaction {
            unsigned: t,
            sig: Vec::new(),
            hash: H256::zero(),
            timestamp: Vec::new(),
        };
        let r = ut.verify_basic(Some(0));
        assert!(r.is_err());
        let e = r.err().unwrap();
        assert_eq!(e, error::Error::InvalidGasPrice);
        println!("test_verify_basic_gas_price_min error={}", e);
    }

    #[test]
    fn test_verify_basic_gas_min() {
        //        let t = Transaction {
        //            action: Action::Call(SYSTEM_ADDRESS),
        //            nonce: U256::from(1),
        //            nonce_bytes: Vec::new(),
        //            gas_price: U256::from(3000),
        //            gas_bytes: Vec::new(),
        //            gas: U256::from(20999),
        //            gas_price_bytes: Vec::new(),
        //            value: U256::from(0),
        //            value_bytes: Vec::new(),
        //            data: ::rustc_hex::FromHex::from_hex("26121ff0").unwrap(),
        //            transaction_type: 1,
        //        };
        //
        //        let ut = UnverifiedTransaction {
        //                unsigned: t,
        //                sig: Vec::new(),
        //                hash: H256::zero(),
        //                timestamp: Vec::new(),
        //        };
        //        println!("{}", rlp::encode(&ut).to_hex());
        let t: UnverifiedTransaction = rlp::decode(&::rustc_hex::FromHex::from_hex("f101a0fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe808426121ff080825207820bb80180").unwrap());
        let r = t.verify_basic(Some(0));
        assert!(r.is_err());
        let e = r.err().unwrap();
        assert_eq!(
            e,
            error::Error::InvalidTransactionGas {
                minimal: GAS_CALL_MIN,
                maximal: GAS_CALL_MAX,
                got: t.gas,
            }
        );
        println!("test_verify_basic_gas_min error={}", e);
    }

    #[test]
    fn test_verify_basic_gas_max() {
        //        let t = Transaction {
        //            action: Action::Call(SYSTEM_ADDRESS),
        //            nonce: U256::from(1),
        //            nonce_bytes: Vec::new(),
        //            gas_price: U256::from(3000),
        //            gas_bytes: Vec::new(),
        //            gas: U256::from(2000001),
        //            gas_price_bytes: Vec::new(),
        //            value: U256::from(0),
        //            value_bytes: Vec::new(),
        //            data: ::rustc_hex::FromHex::from_hex("26121ff0").unwrap(),
        //            transaction_type: 1,
        //        };
        //
        //        let ut = UnverifiedTransaction {
        //                unsigned: t,
        //                sig: Vec::new(),
        //                hash: H256::zero(),
        //                timestamp: Vec::new(),
        //        };
        let t: UnverifiedTransaction = rlp::decode(&::rustc_hex::FromHex::from_hex("f201a0fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe808426121ff080831e8481820bb80180").unwrap());
        let r = t.verify_basic(Some(0));
        assert!(r.is_err());
        let e = r.err().unwrap();
        assert_eq!(
            e,
            error::Error::InvalidTransactionGas {
                minimal: GAS_CALL_MIN,
                maximal: GAS_CALL_MAX,
                got: t.gas,
            }
        );
        println!("test_verify_basic_gas_min error={}", e);
    }

    #[test]
    fn test_verify_basic_gas_contract_create_min() {
        //        let t = Transaction {
        //            action: Action::Create,
        //            nonce: U256::from(1),
        //            nonce_bytes: Vec::new(),
        //            gas_price: U256::from(3000),
        //            gas_bytes: Vec::new(),
        //            gas: U256::from(199999),
        //            gas_price_bytes: Vec::new(),
        //            value: U256::from(0),
        //            value_bytes: Vec::new(),
        //            data: ::rustc_hex::FromHex::from_hex("26121ff0").unwrap(),
        //            transaction_type: 1,
        //        };
        //
        //        let ut = UnverifiedTransaction {
        //                unsigned: t,
        //                sig: Vec::new(),
        //                hash: H256::zero(),
        //                timestamp: Vec::new(),
        //        };
        let t: UnverifiedTransaction = rlp::decode(
            &::rustc_hex::FromHex::from_hex("d20180808426121ff08083030d3f820bb80180").unwrap(),
        );
        let r = t.verify_basic(Some(0));
        assert!(r.is_err());
        let e = r.err().unwrap();
        assert_eq!(
            e,
            error::Error::InvalidContractCreateGas {
                minimal: GAS_CREATE_MIN + GAS_CALL_MIN,
                maximal: GAS_CREATE_MAX,
                got: t.gas,
            }
        );
        println!("test_verify_basic_gas_contract_create_min error={}", e);
    }

    #[test]
    fn test_verify_basic_gas_contract_create_max() {
        //        let t = Transaction {
        //            action: Action::Create,
        //            nonce: U256::from(1),
        //            nonce_bytes: Vec::new(),
        //            gas_price: U256::from(3000),
        //            gas_bytes: Vec::new(),
        //            gas: U256::from(5000001),
        //            gas_price_bytes: Vec::new(),
        //            value: U256::from(0),
        //            value_bytes: Vec::new(),
        //            data: ::rustc_hex::FromHex::from_hex("26121ff0").unwrap(),
        //            transaction_type: 1,
        //        };
        //
        //        let ut = UnverifiedTransaction {
        //                unsigned: t,
        //                sig: Vec::new(),
        //                hash: H256::zero(),
        //                timestamp: Vec::new(),
        //        };
        let t: UnverifiedTransaction = rlp::decode(
            &::rustc_hex::FromHex::from_hex("d20180808426121ff080834c4b41820bb80180").unwrap(),
        );
        let r = t.verify_basic(Some(0));
        assert!(r.is_err());
        let e = r.err().unwrap();
        assert_eq!(
            e,
            error::Error::InvalidContractCreateGas {
                minimal: GAS_CREATE_MIN + GAS_CALL_MIN,
                maximal: GAS_CREATE_MAX,
                got: t.gas,
            }
        );
        println!("test_verify_basic_gas_contract_create_max error={}", e);
    }

    #[test]
    fn test_verify_basic_timestamp_fail() {
        let t: UnverifiedTransaction = rlp::decode(&::rustc_hex::FromHex::from_hex("f87e80800184646174618a1122334455667788990083030d408398968001b860fdc74311a02604a1171e984d64363a5c6073b7dff9e063d1c2eee84f7364021b96613583903c150349ab5da3123f528a59b1fb4d1588b4aac46e484759c37fb961419fff8f77d96aaf78c79b00c61ad3519e0cd640a3bf87cf5a5327e5afdd00").unwrap());
        let r = t.verify_basic(Some(0));
        assert!(r.is_err());
        let e = r.err().unwrap();
        assert_eq!(e, error::Error::InvalidTimestampLength);
        println!("test_verify_basic_timestamp_fail error={}", e);
    }

    #[test]
    fn test_verify_basic_invalid_nonce() {
        //        let t = Transaction {
        //            action: Action::Call(Address::default()),
        //            nonce: U256::from("1111111111111111111111111111111111111111111111111111111111111"),
        //            nonce_bytes: Vec::new(),
        //            gas_price: U256::from(3000),
        //            gas_bytes: Vec::new(),
        //            gas: U256::from(1000_000),
        //            gas_price_bytes: Vec::new(),
        //            value: U256::from(0),
        //            value_bytes: Vec::new(),
        //            data: ::rustc_hex::FromHex::from_hex("26121ff0").unwrap(),
        //            transaction_type: 1,
        //        };
        //
        //        let ut = UnverifiedTransaction {
        //                unsigned: t,
        //                sig: Vec::new(),
        //                hash: H256::zero(),
        //                timestamp: Vec::new(),
        //        };
        let t: UnverifiedTransaction = rlp::decode(&::rustc_hex::FromHex::from_hex("f8519f01111111111111111111111111111111111111111111111111111111111111a00000000000000000000000000000000000000000000000000000000000000000808426121ff080830f4240820bb80180").unwrap());
        let r = t.verify_basic(None);
        assert!(r.is_err());
        let e = r.err().unwrap();
        assert_eq!(e, error::Error::InvalidNonceLength);
        println!("test_verify_basic_invalid_nonce error={}", e);
    }

    #[test]
    fn test_verify_basic_invalid_value() {
        //        let t = Transaction {
        //            action: Action::Call(Address::default()),
        //            nonce: U256::from(1),
        //            nonce_bytes: Vec::new(),
        //            gas_price: U256::from(3000),
        //            gas_bytes: Vec::new(),
        //            gas: U256::from(1000_000),
        //            gas_price_bytes: Vec::new(),
        //            value: U256::from("1111111111111111111111111111111111111111111111111111"),
        //            value_bytes: Vec::new(),
        //            data: ::rustc_hex::FromHex::from_hex("26121ff0").unwrap(),
        //            transaction_type: 1,
        //        };
        //
        //        let ut = UnverifiedTransaction {
        //                unsigned: t,
        //                sig: Vec::new(),
        //                hash: H256::zero(),
        //                timestamp: Vec::new(),
        //        };
        let t: UnverifiedTransaction = rlp::decode(&::rustc_hex::FromHex::from_hex("f84c01a000000000000000000000000000000000000000000000000000000000000000009a11111111111111111111111111111111111111111111111111118426121ff080830f4240820bb80180").unwrap());
        let r = t.verify_basic(None);
        assert!(r.is_err());
        let e = r.err().unwrap();
        assert_eq!(e, error::Error::InvalidValueLength);
        println!("test_verify_basic_invalid_value error={}", e);
    }

    #[test]
    fn gas_decode_test() {
        let t: UnverifiedTransaction = rlp::decode(&::rustc_hex::FromHex::from_hex("f8882a80018648656c6c6f218800000000000000008800000066ffffffff8800000088ffffffff01b860010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101").unwrap());

        println!("gas: 0x{:#x}, gas_price: 0x{:#x}", t.gas, t.gas_price);

        assert_eq!(
            t.gas,
            U256::from("00000000000000000000000000000000000000000000000000000066FFFFFFFF")
        );
        assert_eq!(
            t.gas_price,
            U256::from("00000000000000000000000000000000000000000000000000000088FFFFFFFF")
        );
    }

    #[test]
    fn nrg_decode_test() {
        let t: UnverifiedTransaction = rlp::decode(&::rustc_hex::FromHex::from_hex("f89a80a0a054340a3152d10006b66c4248cfa73e5725056294081c476c0e67ef5ad253340180880005797ca23fea7384000e57e0841f38b2e601b8608bc5c4e5599afac7cb0efcb0010540017dda3e80870bb543b356867b2a8cacbf481c59336a60bcc0690121580766ec64593615ead12a1310d2a413afb411537aa1fb37bb5749f2a29ca8ce5e32a7dbaffd242c5da1b287188d39e8d17b95db06").unwrap());

        println!("UnverifiedTransaction: {:?}", t);

        // assert_eq!(
        //     t.gas,
        //     U256::from("00000000000000000000000000000000000000000000000000000066FFFFFFFF")
        // );
        // assert_eq!(
        //     t.gas_price,
        //     U256::from("00000000000000000000000000000000000000000000000000000088FFFFFFFF")
        // );
    }

    #[test]
    // test gas required
    fn gas_required() {
        let t = Transaction {
            action: Action::Call(Address::default()),
            nonce: U256::from(42),
            nonce_bytes: Vec::new(),
            gas_price: U256::from(3000),
            gas_bytes: Vec::new(),
            gas: U256::from(1000_000),
            gas_price_bytes: Vec::new(),
            value: U256::from(0),
            value_bytes: Vec::new(),
            data: ::rustc_hex::FromHex::from_hex("26121ff0").unwrap(),
            transaction_type: U256::from(1),
        };
        println!("data: {:?}", t.data);
        assert_eq!(t.gas_required().low_u64(), 21256);
    }

    #[test]
    fn signing() {
        use key::generate_keypair;

        let key = generate_keypair();
        let t = Transaction {
            action: Action::Create,
            nonce: U256::from(42),
            nonce_bytes: Vec::new(),
            gas_price: U256::from(3000),
            gas_bytes: Vec::new(),
            gas: U256::from(50_000),
            gas_price_bytes: Vec::new(),
            value: U256::from(1),
            value_bytes: Vec::new(),
            data: b"Hello!".to_vec(),
            transaction_type: U256::from(1),
        }
        .sign(&key.secret(), None);
        let mut slice = blake2b(key.public());
        slice[0] = 0xA0;
        assert_eq!(Address::from(slice), t.sender());
        assert_eq!(t.chain_id(), None);
    }

    #[test]
    fn fake_signing() {
        let t = Transaction {
            action: Action::Create,
            nonce: U256::from(42),
            nonce_bytes: Vec::new(),
            gas_price: U256::from(
                "00000000000000000000000000000000000000000000000000000088FFFFFFFF",
            ),
            gas_price_bytes: Vec::new(),
            gas: U256::from("00000000000000000000000000000000000000000000000000000066FFFFFFFF"),
            gas_bytes: Vec::new(),
            value: U256::from(1),
            value_bytes: Vec::new(),
            data: b"Hello!".to_vec(),
            transaction_type: U256::from(1),
        }
        .fake_sign(Address::from(0x69));
        assert_eq!(Address::from(0x69), t.sender());
        assert_eq!(t.chain_id(), None);

        let t = t.clone();
        assert_eq!(Address::from(0x69), t.sender());
        assert_eq!(t.chain_id(), None);

        println!("{:?}", t.rlp_bytes().to_hex());
    }

    #[test]
    fn should_agree_with_vitalik() {
        use rustc_hex::FromHex;

        let test_vector = |tx_data: &str, address: &'static str| {
            let signed = rlp::decode(&FromHex::from_hex(tx_data).unwrap());
            let signed = SignedTransaction::new(signed).unwrap();
            assert_eq!(signed.sender(), address.into());
            println!("chainid: {:?}", signed.chain_id());
        };

        test_vector("f89480a0a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e6340a80880005748d2d576db18252080101b8608bc5c4e5599afac7cb0efcb0010540017dda3e80870bb543b356867b2a8cacbf7447205c145d35c2a4e6bd58e58e5894b37416247ed0330f4bb114984f869aa3ed914130be741856a664439cbd64d5583e85dd470ea448c8fc9102b2116c2a0a", "0xa00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2c");
        test_vector("f89480a0a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e6340a80880005748d37a7c7488252080101b860a3417d533e677c33b5aa5182527792a52a3b7c67f4862e99b0e43bb03a5852f54706ab2e799a4e61a5545b97b70a6009b00524a97b20801283aeb70805303ecea45d60ce6728cb8e3282759634b4c49dc05c6050750820c27a0adec15ba51c04", "0xa0362a7ed604d2e9d641e22726892702230d5cd4e87cf84d74675c465adaa577");
        test_vector("f89480a0a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e6340a80880005748d390e80db8252080101b8603e7012bfc929a0c31b09b6d3e50128e1a3e3350ea9e407a929f7aa967c954102f16f458cb7ce35063f8a320520b145e69a267562f17d34a8ffca68edb7f07931c452e3f630ccad66c52b8a843c83126bd421af135db13a28ff047e073c37ea00", "0xa054340a3152d10006b66c4248cfa73e5725056294081c476c0e67ef5ad25334");
        test_vector("f89480a0a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e6340a80880005748d3adca3638252080101b8601b1c767320a33b0bbfc62114f4a492da1ea75f38b305fd3f7273555dc7f316a5224ebd3b0db5973387b2abb59b3d98a298f435d100838b5a364e9933c76522028c714186224206ab549a31f3e72f4fd69c3d64a4c66ebf59c50e01d575ef6d0c", "0xa05480f0440b5bfbf62dd9a8f9efe83a1dc00e9be3bb569d96e3958d048196cf");
        test_vector("f89480a0a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e6340a80880005748d3c2d156f8252080101b860a6fcfe53896acc3eb28c9cd68dda6ed7666a6e209a644b630ebbc51a2db179048bae8cbc2b58756a50f6a206ac5055fc685b6d87986ef86a2c513dde47d501a26d3883c93265c42e6d4bf69c3fc65853c28d8ad3e073351bd41bc8f26180a501", "0xa05a27f0c1ea16ed4433c9efc86ac08effbf2cd4530d08a2b35393b05e489df5");
        test_vector("f89480a0a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e6340a80880005748d3d71edd68252080101b86003ea58c243f6dde976fed0fce36bfcc61e428ce1c7697e5560ab5122b6de5d6df944f0d79e98b307a47b3e75fc9b88c587805972df2ef69c8a315761498077860d4f147403d8061833fdb9c58935bd7ed85efc9fdfaf352d0b110d27b98c5106", "0xa05ed4fcb3fd1c2b8d65f7a9cbff0e280e53b40e6399f9887c3e28b37b5d09bf");
        test_vector("f89480a0a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e6340a80880005748d3f28f5078252080101b8603b6c306d49bf5753aea6bd3279b96de9a77632a8b24e0b95d21519197e00cb0b3c1b5fef55cb4316b4942b91145168dcda405ff6fb8abce9e70fd3751f5e994405c509c181d9d763c3be9625a7ecdd430fc77f4161dc63d320a79a0710449603", "0xa064f2fbf5d703733d723ba5f08109d96196467331d5b4568835a634e81c5715");
        test_vector("f89480a0a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e6340a80880005748d407ca5228252080101b8600df77c275f7bb54197c31860153af6d6b23762cbda1b30aaf28672a4399170712cf11bb211f9cf5e8d25a99b97bd1c59a7b26bceb726fda9c0ced33b4c8e147116c1bd179f9ea559ac73fba50e4cf78337e1c68198091fccd6c8815a6f40fe02", "0xa07611e8110193fe44a44d249f67f11ff86e2067209a54edccb2ca0f5d8ea3e3");
        test_vector("f89480a0a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e6340a80880005748d423b74d68252080101b86054c0b58818d59cdf27f7aa7b2ae61e62fac7c3c4fadd3fc737dcf256314992f0702f3fd509781e7332c7877adb3b59a3be901995f6019e30cb81729a8417f2bbfe23f00cff118c4e9dd037a21e941d842a186ee4a9f0b9905c1a5b4a93fb1b09", "0xa07e185919beef1e0a79fea78fcfabc24927c5067d758e514ad74b905a2bf137");
        test_vector("f89480a0a02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e6340a80880005748d44454ecf8252080101b8609651eece2224f6841405c5f00cc25223d1cda1fee8fd65bd113d7138f42244a1fa1003774ec12fb36e675c6fa55288d034c0930bb8bc9b5ffba0b0dc69ed57c434be19502da4a6e18f98fdc8c4fba688536e214b27f024cccd4ce8a25ed0e805", "0xa02c25e39471085ff8cae0882132d82c6490eb02f3e6906b303d8f990e86e634");
    }
}
