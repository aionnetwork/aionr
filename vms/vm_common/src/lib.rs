extern crate aion_types;
extern crate rlp;
extern crate blake2b as hash;
extern crate common_types as types;
extern crate ajson;

use aion_types::{U256, H256, Address};
use rlp::{Encodable, Decodable, DecoderError, RlpStream, UntrustedRlp};

// return type definition
/// Return data buffer. Holds memory from a previous call and a slice into that memory.
#[derive(Debug, PartialEq, Clone)]
pub struct ReturnData {
    pub mem: Vec<u8>,
    pub offset: usize,
    pub size: usize,
}

impl ::std::ops::Deref for ReturnData {
    type Target = [u8];
    fn deref(&self) -> &[u8] { &self.mem[self.offset..self.offset + self.size] }
}

impl ReturnData {
    /// Create empty `ReturnData`.
    pub fn empty() -> Self {
        ReturnData {
            mem: Vec::new(),
            offset: 0,
            size: 0,
        }
    }
    /// Create `ReturnData` from give buffer and slice.
    pub fn new(mem: Vec<u8>, offset: usize, size: usize) -> Self {
        ReturnData {
            mem: mem,
            offset: offset,
            size: size,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum ExecStatus {
    Success,
    OutOfGas,
    Revert,
    Failure,
    Rejected,
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
    // storage root : AVM
    //pub storage_root: u32,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        ExecutionResult {
            gas_left: 0.into(),
            status_code: ExecStatus::Success,
            return_data: ReturnData::empty(),
            exception: String::new(),
            //storage_root: 0,
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
    /// Avm balance transfer
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
            // this is for avm
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
                //TODO: conflicted, so cannot get BulkBalance from decoder
                4 => CallType::StaticCall,
                _ => return Err(DecoderError::Custom("Invalid value of CallType item")),
            })
        })
    }
}

use std::cmp;
use std::sync::Arc;
use hash::blake2b;
use types::BlockNumber;

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

impl From<ajson::vm::Env> for EnvInfo {
    fn from(e: ajson::vm::Env) -> Self {
        let number = e.number.into();
        EnvInfo {
            number: number,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_can_be_created_as_default() {
        let default_env_info = EnvInfo::default();

        assert_eq!(default_env_info.difficulty, 0.into());
    }
}
