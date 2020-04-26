use std::convert::Into;
use std::sync::Arc;
use std::cmp;
use bytes::Bytes;
use aion_types::{ U256, H256, Address };
use ajson::vm::Env;
use rlp::{ Encodable, Decodable, RlpStream, UntrustedRlp, DecoderError };
use blake2b::blake2b;

use types::ReturnData;

type BlockNumber = u64;

#[derive(Debug, PartialEq, Clone)]
/// fastvm uses ExecStatus directly
pub enum ExecStatus {
    Success,
    OutOfGas,
    Revert,
    Failure,
    Rejected,
    Fatal,
}

/// Finalization result. Gas Left: either it is a known value, or it needs to be computed by processing
/// a return instruction.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Final amount of gas left.
    pub gas_left: U256,
    /// Status code returned from VM
    pub status_code: ExecStatus,
    /// Return data buffer.
    pub return_data: ReturnData,
    /// exception / error message (empty if success)
    pub exception: String,
    /// state root from avm
    pub state_root: H256,
    /// invokable transaction hashes from avm
    pub invokable_hashes: Vec<(H256, H256)>,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        ExecutionResult {
            gas_left: 0.into(),
            status_code: ExecStatus::Success,
            return_data: ReturnData::empty(),
            exception: String::new(),
            state_root: H256::default(),
            invokable_hashes: Vec::new(),
        }
    }
}

/// The type of the call-like instruction.
#[derive(Debug, PartialEq, Clone)]
pub enum CallType {
    /// Not a CALL.
    None,
    /// CALL.
    Call,
    /// CALLCODE.
    CallCode,
    /// DELEGATECALL.
    DelegateCall,
    /// STATICCALL
    StaticCall,
    /// avm balance transfer
    BulkBalance,
}

impl Encodable for CallType {
    fn rlp_append(&self, s: &mut RlpStream) {
        let v = match *self {
            CallType::None => 0u32,
            CallType::Call => 1,
            CallType::CallCode => 2,
            CallType::DelegateCall => 3,
            CallType::StaticCall => 4,
            // conflicted with StaticCall, may cause decode error
            CallType::BulkBalance => 4,
        };
        Encodable::rlp_append(&v, s);
    }
}

impl Decodable for CallType {
    fn decode(rlp: &UntrustedRlp) -> ::std::result::Result<Self, DecoderError> {
        rlp.as_val().and_then(|v| {
            Ok(match v {
                0u32 => CallType::None,
                1 => CallType::Call,
                2 => CallType::CallCode,
                3 => CallType::DelegateCall,
                4 => CallType::StaticCall,
                // avm bulk balance transfer is missing
                _ => return Err(DecoderError::Custom("Invalid value of CallType item")),
            })
        })
    }
}

/// Simple vector of hashes, should be at most 256 items large, can be smaller if being used
/// for a block whose number is less than 257.
pub type LastHashes = Vec<H256>;

/// Information concerning the execution environment for a message-call/contract-creation.
#[derive(Debug, Clone)]
pub struct EnvInfo {
    /// The block number.
    pub number: BlockNumber,
    /// The block author.
    pub author: Address,
    /// The block timestamp.
    pub timestamp: u64,
    /// The block difficulty.
    pub difficulty: U256,
    /// The block gas limit.
    pub gas_limit: U256,
    /// The last 256 block hashes.
    pub last_hashes: Arc<LastHashes>,
    /// The gas used.
    pub gas_used: U256,
}

impl Default for EnvInfo {
    fn default() -> Self {
        EnvInfo {
            number: 0,
            author: Address::default(),
            timestamp: 0,
            difficulty: 0.into(),
            gas_limit: 0.into(),
            last_hashes: Arc::new(vec![]),
            gas_used: 0.into(),
        }
    }
}

impl From<Env> for EnvInfo {
    fn from(e: Env) -> Self {
        let number = e.number.into();
        EnvInfo {
            number,
            author: e.author.into(),
            difficulty: e.difficulty.into(),
            gas_limit: e.gas_limit.into(),
            timestamp: e.timestamp.into(),
            last_hashes: Arc::new(
                (1..cmp::min(number + 1, 257))
                    .map(|i| blake2b(format!("{}", number - i).as_bytes()))
                    .collect(),
            ),
            gas_used: U256::default(),
        }
    }
}

/// Transaction value
#[derive(Clone, Debug)]
pub enum ActionValue {
    /// Value that should be transfered
    Transfer(U256),
    /// Apparent value for transaction (not transfered)
    Apparent(U256),
}

impl Into<[u8; 32]> for ActionValue {
    fn into(self) -> [u8; 32] {
        match self {
            ActionValue::Transfer(val) => (U256::from(val)).into(),
            ActionValue::Apparent(val) => (U256::from(val)).into(),
        }
    }
}

impl ActionValue {
    /// Returns action value as U256.
    pub fn value(&self) -> U256 {
        match *self {
            ActionValue::Transfer(x) | ActionValue::Apparent(x) => x,
        }
    }

    /// Returns the transfer action value of the U256-convertable raw value
    pub fn transfer<T: Into<U256>>(transfer_value: T) -> ActionValue {
        ActionValue::Transfer(transfer_value.into())
    }

    /// Returns the apparent action value of the U256-convertable raw value
    pub fn apparent<T: Into<U256>>(apparent_value: T) -> ActionValue {
        ActionValue::Apparent(apparent_value.into())
    }
}

/// Type of the way parameters encoded
#[derive(Clone, Debug)]
pub enum ParamsType {
    /// Parameters are included in code
    Embedded,
    /// Parameters are passed in data section
    Separate,
}

#[derive(Debug, Clone)]
pub struct ActionParams {
    /// Address of currently executed code.
    pub code_address: Address,
    /// Hash of currently executed code.
    pub code_hash: Option<H256>,
    /// Receive address. Usually equal to code_address,
    /// except when called using CALLCODE.
    pub address: Address,
    /// Sender of current part of the transaction.
    pub sender: Address,
    /// Transaction initiator.
    pub origin: Address,
    /// Gas paid up front for transaction execution
    pub gas: U256,
    /// Gas price.
    pub gas_price: U256,
    /// Transaction value.
    pub value: ActionValue,
    /// Code being executed.
    pub code: Option<Arc<Bytes>>,
    /// Input data.
    pub data: Option<Bytes>,
    /// Type of call
    pub call_type: CallType,
    /// Flag to indicate if the call is static
    pub static_flag: bool,
    /// Param types encoding
    pub params_type: ParamsType,
    /// transaction hash
    pub transaction_hash: H256,
    /// original transaction hash
    pub original_transaction_hash: H256,
    /// Nonce
    pub nonce: u64,
}

impl Default for ActionParams {
    fn default() -> Self {
        ActionParams {
            code_address: Address::default(),
            code_hash: None,
            address: Address::default(),
            sender: Address::default(),
            origin: Address::default(),
            gas: U256::default(),
            gas_price: U256::default(),
            value: ActionValue::Transfer(0.into()),
            code: None,
            data: None,
            call_type: CallType::None,
            static_flag: false,
            params_type: ParamsType::Embedded,
            transaction_hash: H256::default(),
            original_transaction_hash: H256::default(),
            nonce: 0,
        }
    }
}
