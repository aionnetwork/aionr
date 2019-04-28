use super::codec::{NativeDecoder, NativeEncoder};
use aion_types::{Address, H256};
use vm_common::ExecStatus;

use std::fmt;

type Bytes = Vec<u8>;

#[derive(Debug, PartialEq, Clone)]
#[repr(C)]
pub enum AvmStatusCode {
    //Camus: Revert status should be dealed within avm.
    Success,
    Rejected,
    Failure,
    Unsupprted,
}

impl From<i32> for AvmStatusCode {
    fn from(code: i32) -> Self {
        match code {
            0 => AvmStatusCode::Success,
            1 => AvmStatusCode::Rejected,
            2 => AvmStatusCode::Failure,
            _ => AvmStatusCode::Unsupprted,
        }
    }
}

impl Into<i32> for AvmStatusCode {
    fn into(self) -> i32 {
        match self {
            AvmStatusCode::Success => 0,
            AvmStatusCode::Rejected => 1,
            AvmStatusCode::Failure => 2,
            _ => -99,
        }
    }
}

impl From<AvmStatusCode> for ExecStatus {
    fn from(status: AvmStatusCode) -> ExecStatus {
        match status {
            AvmStatusCode::Success => ExecStatus::Success,
            AvmStatusCode::Rejected => ExecStatus::Rejected,
            // avm failure does not cost all gas, it is actually Revert.
            //TODO: needs a more detailed definition of avm status code
            AvmStatusCode::Failure => ExecStatus::Revert,
            _ => ExecStatus::Failure,
        }
    }
}

impl fmt::Display for AvmStatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AvmStatusCode::Success => write!(f, "AvmSuccess"),
            AvmStatusCode::Rejected => write!(f, "AvmRejected"),
            AvmStatusCode::Failure => write!(f, "AvmFailure"),
            AvmStatusCode::Unsupprted => write!(f, "AvmUnsupported"),
        }
    }
}

#[derive(Debug)]
pub struct TransactionContext {
    /// 2 - CREATE; 3 - CALL; 4 - BALANCE_TRANSFER; 5 - GC
    pub transaction_type: u8,
    pub address: Bytes,
    pub caller: Bytes,
    pub origin: Bytes,
    pub nonce: u64,
    pub value: Bytes,
    pub data: Bytes,
    pub energy_limit: u64,
    pub energy_price: u64,
    pub transaction_hash: Bytes,
    pub basic_cost: u32,
    pub transaction_timestamp: u64,
    pub block_timestamp: u64,
    pub block_number: u64,
    pub block_energy_limit: u64,
    pub block_coinbase: Bytes,
    pub block_previous_hash: Bytes,
    pub block_difficulty: Bytes,
    pub internal_call_depth: u32,
}

impl TransactionContext {
    pub fn new(
        tx_hash: Bytes,
        address: Address,
        origin: Address,
        caller: Address,
        nrg_price: u64,
        nrg_limit: u64,
        call_value: Bytes,
        call_data: Bytes,
        depth: i32,
        kind: i32,
        block_coinbase: Address,
        block_number: u64,
        block_timestamp: i64,
        block_nrglimit: u64,
        block_difficulty: Bytes,
        nonce: u64,
    ) -> Self
    {
        TransactionContext {
            transaction_type: kind as u8,
            address: address.to_vec(),
            caller: caller.to_vec(),
            origin: origin.to_vec(),
            nonce: nonce,
            value: call_value,
            data: call_data,
            energy_limit: nrg_limit,
            energy_price: nrg_price,
            transaction_hash: tx_hash,
            basic_cost: 0,//200_000,
            transaction_timestamp: 0 as u64, //TODO:
            block_timestamp: block_timestamp as u64,
            block_number: block_number,
            block_energy_limit: block_nrglimit,
            block_coinbase: block_coinbase.to_vec(),
            block_previous_hash: Address::new().to_vec(), //TODO:
            block_difficulty: block_difficulty,
            internal_call_depth: depth as u32,
        }
    }

    pub fn to_bytes(&self) -> Bytes {
        let mut enc = NativeEncoder::new();

        enc.encode_byte(self.transaction_type);
        enc.encode_bytes(&self.address);
        enc.encode_bytes(&self.caller);
        enc.encode_bytes(&self.origin);
        enc.encode_long(self.nonce);
        enc.encode_bytes(&self.value);
        enc.encode_bytes(&self.data);
        enc.encode_long(self.energy_limit);
        enc.encode_long(self.energy_price);
        enc.encode_bytes(&self.transaction_hash);
        enc.encode_int(self.basic_cost);
        enc.encode_long(self.transaction_timestamp);
        enc.encode_long(self.block_timestamp);
        enc.encode_long(self.block_number);
        enc.encode_long(self.block_energy_limit);
        enc.encode_bytes(&self.block_coinbase);
        enc.encode_bytes(&self.block_previous_hash);
        enc.encode_bytes(&self.block_difficulty);
        enc.encode_int(self.internal_call_depth);

        enc.to_bytes()
    }
}

#[derive(Debug, Clone)]
pub struct Log {
    pub address: Bytes,
    pub topics: Vec<Bytes>,
    pub data: Bytes,
}

#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub code: u32,
    pub return_data: Bytes,
    pub energy_used: u64,
    pub state_root: H256,
}

impl TransactionResult {
    pub fn new(bytes: Bytes, state_root: Bytes) -> Result<TransactionResult, &'static str> {
        let mut decoder = NativeDecoder::new(&bytes);
        let code = decoder.decode_int()?;
        let return_data = decoder.decode_bytes()?;
        let energy_used = decoder.decode_long()?;

        Ok(TransactionResult {
            code,
            return_data,
            energy_used,
            state_root: state_root.as_slice().into(),
        })
    }
}
