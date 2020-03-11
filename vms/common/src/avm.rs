use std::fmt;
use aion_types::{Address,H256, U256};
use types::ReturnData;
use super::ExecStatus;

#[derive(Debug)]
pub struct TransactionContext {
    /// 2 - CREATE; 3 - CALL; 4 - BALANCE_TRANSFER; 5 - GC
    pub transaction_type: u8,
    pub address: Vec<u8>,
    pub caller: Vec<u8>,
    pub origin: Vec<u8>,
    pub nonce: u64,
    pub value: Vec<u8>,
    pub data: Vec<u8>,
    pub energy_limit: u64,
    pub energy_price: u64,
    pub transaction_hash: Vec<u8>,
    pub basic_cost: u32,
    pub transaction_timestamp: u64,
    pub block_timestamp: u64,
    pub block_number: u64,
    pub block_energy_limit: u64,
    pub block_coinbase: Vec<u8>,
    pub block_previous_hash: Vec<u8>,
    pub block_difficulty: Vec<u8>,
    pub internal_call_depth: u32,
}

impl TransactionContext {
    pub fn new(
        tx_hash: Vec<u8>,
        address: Address,
        origin: Address,
        caller: Address,
        nrg_price: u64,
        nrg_limit: u64,
        call_value: Vec<u8>,
        call_data: Vec<u8>,
        depth: i32,
        kind: i32,
        block_coinbase: Address,
        block_number: u64,
        block_timestamp: i64,
        block_nrglimit: u64,
        block_difficulty: Vec<u8>,
        nonce: u64,
    ) -> Self
    {
        TransactionContext {
            transaction_type: kind as u8,
            address: address.to_vec(),
            caller: caller.to_vec(),
            origin: origin.to_vec(),
            nonce,
            value: call_value,
            data: call_data,
            energy_limit: nrg_limit,
            energy_price: nrg_price,
            transaction_hash: tx_hash,
            basic_cost: 0,                   //200_000,
            transaction_timestamp: 0 as u64, //TODO:
            block_timestamp: block_timestamp as u64,
            block_number,
            block_energy_limit: block_nrglimit,
            block_coinbase: block_coinbase.to_vec(),
            block_previous_hash: Address::new().to_vec(), //TODO:
            block_difficulty,
            internal_call_depth: depth as u32,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
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

    pub fn tx_hash(&self) -> &Vec<u8> { return &self.transaction_hash; }
}

#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub status: u32,
    pub return_data: Vec<u8>,
    pub energy_used: u64,
    pub state_root: H256,
    pub invokable_hashes: Vec<H256>,
}

impl TransactionResult {
    pub fn new(bytes: Vec<u8>, state_root: Vec<u8>) -> Result<TransactionResult, &'static str> {
        let mut decoder = NativeDecoder::new(&bytes);
        let status = decoder.decode_int()?;
        let return_data = decoder.decode_bytes()?;
        let energy_used = decoder.decode_long()?;
        let mut invokable_hashes = Vec::new();
        loop {
            match decoder.decode_bytes() {
                Ok(hash) => invokable_hashes.push(hash[..].into()),
                Err(_) => break,
            }
        }
        Ok(TransactionResult {
            status,
            return_data,
            energy_used,
            state_root: state_root.as_slice().into(),
            invokable_hashes,
        })
    }
}

pub struct NativeEncoder {
    buffer: Vec<u8>,
}

impl NativeEncoder {
    pub fn new() -> NativeEncoder {
        let buffer: Vec<u8> = Vec::new();

        NativeEncoder {
            buffer,
        }
    }

    pub fn encode_byte(&mut self, n: u8) { self.buffer.push(n); }

    pub fn encode_short(&mut self, n: u16) {
        self.buffer.push((n >> 8) as u8);
        self.buffer.push(n as u8);
    }

    pub fn encode_int(&mut self, n: u32) {
        self.buffer.push((n >> 24) as u8);
        self.buffer.push((n >> 16) as u8);
        self.buffer.push((n >> 8) as u8);
        self.buffer.push(n as u8);
    }

    pub fn encode_long(&mut self, n: u64) {
        self.buffer.push((n >> 56) as u8);
        self.buffer.push((n >> 48) as u8);
        self.buffer.push((n >> 40) as u8);
        self.buffer.push((n >> 32) as u8);
        self.buffer.push((n >> 24) as u8);
        self.buffer.push((n >> 16) as u8);
        self.buffer.push((n >> 8) as u8);
        self.buffer.push(n as u8);
    }

    pub fn encode_bytes(&mut self, bytes: &Vec<u8>) {
        self.encode_int(bytes.len() as u32);
        self.buffer.append(&mut bytes.clone());
    }

    pub fn to_bytes(&self) -> Vec<u8> { self.buffer.clone() }
}

pub struct NativeDecoder {
    bytes: Vec<u8>,
    index: usize,
}

impl NativeDecoder {
    pub fn new(bytes: &Vec<u8>) -> NativeDecoder {
        NativeDecoder {
            bytes: bytes.clone(),
            index: 0,
        }
    }

    pub fn decode_byte(&mut self) -> Result<u8, &'static str> {
        match self.require(1) {
            true => {
                let ret = self.bytes[self.index];
                self.index = self.index + 1;
                Ok(ret)
            }
            false => Err("Index out of bounds"),
        }
    }

    pub fn decode_short(&mut self) -> Result<u16, &'static str> {
        match self.require(2) {
            true => {
                let ret =
                    ((self.bytes[self.index] as u16) << 8) | (self.bytes[self.index + 1] as u16);
                self.index = self.index + 2;
                Ok(ret)
            }
            false => Err("Index out of bounds"),
        }
    }

    pub fn decode_int(&mut self) -> Result<u32, &'static str> {
        match self.require(4) {
            true => {
                let ret = ((self.bytes[self.index] as u32) << 24)
                    | ((self.bytes[self.index + 1] as u32) << 16)
                    | ((self.bytes[self.index + 2] as u32) << 8)
                    | (self.bytes[self.index + 3] as u32);
                self.index = self.index + 4;
                Ok(ret)
            }
            false => Err("Index out of bounds"),
        }
    }

    pub fn decode_long(&mut self) -> Result<u64, &'static str> {
        match self.require(8) {
            true => {
                let ret = ((self.bytes[self.index] as u64) << 56)
                    | ((self.bytes[self.index + 1] as u64) << 48)
                    | ((self.bytes[self.index + 2] as u64) << 40)
                    | ((self.bytes[self.index + 3] as u64) << 32)
                    | ((self.bytes[self.index + 4] as u64) << 24)
                    | ((self.bytes[self.index + 5] as u64) << 16)
                    | ((self.bytes[self.index + 6] as u64) << 8)
                    | (self.bytes[self.index + 7] as u64);
                self.index = self.index + 8;
                Ok(ret)
            }
            false => Err("Index out of bounds"),
        }
    }

    pub fn decode_bytes(&mut self) -> Result<Vec<u8>, &'static str> {
        let size = self.decode_int()? as usize;
        match self.require(size) {
            true => {
                let slice = self.bytes.as_slice();
                let ret = slice[self.index..self.index + size].to_vec();
                self.index = self.index + size;
                Ok(ret)
            }
            false => Err("Index out of bounds"),
        }
    }

    pub fn require(&self, n: usize) -> bool { self.bytes.len() - self.index >= n }
}

#[derive(Debug, PartialEq, Clone)]
#[repr(C)]
pub enum AvmStatusCode {
    //Camus: Revert status should be dealed within avm.
    Success,
    Rejected,
    Failure,
    Fatal,
}

impl AvmStatusCode {
    pub fn is_fatal(&self) -> bool { *self == AvmStatusCode::Fatal }

    pub fn is_success(&self) -> bool { *self == AvmStatusCode::Success }

    pub fn is_reject(&self) -> bool { *self == AvmStatusCode::Rejected }

    pub fn is_failure(&self) -> bool { *self == AvmStatusCode::Failure }
}

impl From<i32> for AvmStatusCode {
    fn from(code: i32) -> Self {
        match code {
            0 => AvmStatusCode::Success,
            1 => AvmStatusCode::Rejected,
            2 => AvmStatusCode::Failure,
            _ => AvmStatusCode::Fatal,
        }
    }
}

impl Into<i32> for AvmStatusCode {
    fn into(self) -> i32 {
        match self {
            AvmStatusCode::Success => 0,
            AvmStatusCode::Rejected => 1,
            AvmStatusCode::Failure => 2,
            AvmStatusCode::Fatal => -1,
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
            AvmStatusCode::Fatal => write!(f, "AvmFatal"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Final amount of gas left.
    pub gas_left: U256,
    /// Status code returned from VM
    pub status_code: AvmStatusCode,
    /// Return data buffer.
    pub return_data: ReturnData,
    /// exception / error message (empty if success)
    pub exception: String,
    /// state root from avm
    pub state_root: H256,
    /// invokable transaction hashes from avm
    pub invokable_hashes: Vec<(H256, H256)>,
}
