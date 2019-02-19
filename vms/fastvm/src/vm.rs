/*******************************************************************************
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

use aion_types::{U256, H256, H128, U128, U512, Address};
use bytes::Bytes;
use hash::{blake2b, BLAKE2B_EMPTY};
use ffi::EvmStatusCode;
use std::{fmt, ops, cmp};
use vm_common::{ReturnData, ExecutionResult, CallType, EnvInfo};

// result definition
/// VM errors. from vm/src/errors.rs
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// `OutOfGas` is returned when transaction execution runs out of gas.
    /// The state should be reverted to the state from before the
    /// transaction execution. But it does not mean that transaction
    /// was invalid. Balance still should be transfered and nonce
    /// should be increased.
    OutOfGas,
    /// `BadJumpDestination` is returned when execution tried to move
    /// to position that wasn't marked with JUMPDEST instruction
    BadJumpDestination {
        /// Position the code tried to jump to.
        destination: usize,
    },
    /// `BadInstructions` is returned when given instruction is not supported
    BadInstruction {
        /// Unrecognized opcode
        instruction: u8,
    },
    /// `StackUnderflow` when there is not enough stack elements to execute instruction
    StackUnderflow {
        /// Invoked instruction
        instruction: &'static str,
        /// How many stack elements was requested by instruction
        wanted: usize,
        /// How many elements were on stack
        on_stack: usize,
    },
    /// When execution would exceed defined Stack Limit
    OutOfStack {
        /// Invoked instruction
        instruction: &'static str,
        /// How many stack elements instruction wanted to push
        wanted: usize,
        /// What was the stack limit
        limit: usize,
    },
    /// Built-in contract failed on given input
    BuiltIn(String),
    /// Likely to cause consensus issues.
    Internal(String),
    /// Out of bounds access in RETURNDATACOPY.
    OutOfBounds,
    /// Execution has been reverted with REVERT.
    Reverted,
}

impl From<Box<::trie::TrieError>> for Error {
    fn from(err: Box<::trie::TrieError>) -> Self {
        Error::Internal(format!("Internal error: {}", err))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        match *self {
            OutOfGas => write!(f, "Out of gas"),
            BadJumpDestination {
                destination,
            } => write!(f, "Bad jump destination {:x}", destination),
            BadInstruction {
                instruction,
            } => write!(f, "Bad instruction {:x}", instruction),
            StackUnderflow {
                instruction,
                wanted,
                on_stack,
            } => write!(f, "Stack underflow {} {}/{}", instruction, wanted, on_stack),
            OutOfStack {
                instruction,
                wanted,
                limit,
            } => write!(f, "Out of stack {} {}/{}", instruction, wanted, limit),
            BuiltIn(ref name) => write!(f, "Built-in failed: {}", name),
            Internal(ref msg) => write!(f, "Internal error: {}", msg),
            OutOfBounds => write!(f, "Out of bounds"),
            Reverted => write!(f, "Reverted"),
        }
    }
}

use std::sync::Arc;

/// Transaction value
#[derive(Clone, Debug)]
pub enum ActionValue {
    /// Value that should be transfered
    Transfer(U256),
    /// Apparent value for transaction (not transfered)
    Apparent(U256),
}

/// Type of the way parameters encoded
#[derive(Clone, Debug)]
pub enum ParamsType {
    /// Parameters are included in code
    Embedded,
    /// Parameters are passed in data section
    Separate,
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

// TODO: should be a trait, possible to avoid cloning everything from a Transaction(/View).
/// Action (call/create) input params. Everything else should be specified in Externalities.
#[derive(Clone, Debug)]
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
}

impl Default for ActionParams {
    /// Returns default ActionParams initialized with zeros
    fn default() -> ActionParams {
        ActionParams {
            code_address: Address::new(),
            code_hash: Some(BLAKE2B_EMPTY),
            address: Address::new(),
            sender: Address::new(),
            origin: Address::new(),
            gas: U256::zero(),
            gas_price: U256::zero(),
            value: ActionValue::Transfer(U256::zero()),
            code: None,
            data: None,
            call_type: CallType::None,
            static_flag: false,
            params_type: ParamsType::Separate,
            transaction_hash: H256::default(),
            original_transaction_hash: H256::default(),
        }
    }
}

impl From<::ajson::vm::Transaction> for ActionParams {
    fn from(t: ::ajson::vm::Transaction) -> Self {
        let address: Address = t.address.into();
        ActionParams {
            code_address: Address::new(),
            code_hash: Some(blake2b(&*t.code)),
            address: address,
            sender: t.sender.into(),
            origin: t.origin.into(),
            code: Some(Arc::new(t.code.into())),
            data: Some(t.data.into()),
            gas: t.gas.into(),
            gas_price: t.gas_price.into(),
            value: ActionValue::Transfer(t.value.into()),
            call_type: match address.is_zero() {
                true => CallType::None,
                false => CallType::Call,
            },
            static_flag: false,
            params_type: ParamsType::Separate,
            transaction_hash: H256::default(),
            original_transaction_hash: H256::default(),
        }
    }
}

#[derive(Debug)]
pub struct FvmExecutionResult {
    /// Final amount of gas left.
    pub gas_left: U256,
    /// Status code returned from VM
    pub status_code: EvmStatusCode,
    /// Return data buffer.
    pub return_data: ReturnData,
    /// exception / error message (empty if success)
    pub exception: String,
}

/// Externalities interface for EVMs
pub trait Ext {
    /// Returns a value for given key.
    fn storage_at(&self, key: &H128) -> H128;

    /// Stores a value for given key.
    fn set_storage(&mut self, key: H128, value: H128);

    /// Returns a value for given key.
    fn storage_at_dword(&self, key: &H128) -> H256;

    /// Stores a value for given key.
    fn set_storage_dword(&mut self, key: H128, value: H256);

    /// Determine whether an account exists.
    fn exists(&self, address: &Address) -> bool;

    /// Determine whether an account exists and is not null (zero balance/nonce, no code).
    fn exists_and_not_null(&self, address: &Address) -> bool;

    /// Balance of the origin account.
    fn origin_balance(&self) -> U256;

    /// Returns address balance.
    fn balance(&self, address: &Address) -> U256;

    /// Returns the hash of one of the 256 most recent complete blocks.
    fn blockhash(&mut self, number: &U256) -> H256;

    /// Creates new contract.
    ///
    /// Returns gas_left and contract address if contract creation was succesfull.
    fn create(&mut self, gas: &U256, value: &U256, code: &[u8]) -> ExecutionResult;

    /// Message call.
    ///
    /// Returns Err, if we run out of gas.
    /// Otherwise returns call_result which contains gas left
    /// and true if subcall was successfull.
    fn call(
        &mut self,
        gas: &U256,
        sender_address: &Address,
        receive_address: &Address,
        value: Option<U256>,
        data: &[u8],
        code_address: &Address,
        call_type: CallType,
        static_flag: bool,
    ) -> ExecutionResult;

    /// Returns code at given address
    fn extcode(&self, address: &Address) -> Arc<Bytes>;

    /// Returns code size at given address
    fn extcodesize(&self, address: &Address) -> usize;

    /// Creates log entry with given topics and data
    fn log(&mut self, topics: Vec<H256>, data: &[u8]);

    /// Should be called when contract commits suicide.
    /// Address to which funds should be refunded.
    fn suicide(&mut self, refund_address: &Address);

    /// Returns environment info.
    fn env_info(&self) -> &EnvInfo;

    /// Returns current depth of execution.
    ///
    /// If contract A calls contract B, and contract B calls C,
    /// then A depth is 0, B is 1, C is 2 and so on.
    fn depth(&self) -> usize;

    /// Increments sstore refunds count by 1.
    fn inc_sstore_clears(&mut self);

    /// Decide if any more operations should be traced. Passthrough for the VM trace.
    fn trace_next_instruction(&mut self, _pc: usize, _instruction: u8, _current_gas: U256) -> bool {
        false
    }

    /// Prepare to trace an operation. Passthrough for the VM trace.
    fn trace_prepare_execute(&mut self, _pc: usize, _instruction: u8, _gas_cost: U256) {}

    /// Trace the finalised execution of a single instruction.
    fn trace_executed(
        &mut self,
        _gas_used: U256,
        _stack_push: &[U256],
        _mem_diff: Option<(usize, &[u8])>,
        _store_diff: Option<(U256, U256)>,
    )
    {
    }

    /// Save code to newly created contract.
    fn save_code(&mut self, code: Bytes);

    fn set_special_empty_flag(&mut self);
}

/// Virtual Machine interface
pub trait Vm {
    /// This function should be used to execute transaction.
    /// It returns either an error, a known amount of gas left, or parameters to be used
    /// to compute the final gas left.
    fn exec(&mut self, params: ActionParams, ext: &mut Ext) -> Result<FvmExecutionResult, Error>;
}

/// Cost calculation type. For low-gas usage we calculate costs using usize instead of U256
pub trait CostType:
    Sized
    + From<usize>
    + Copy
    + ops::Mul<Output = Self>
    + ops::Div<Output = Self>
    + ops::Add<Output = Self>
    + ops::Sub<Output = Self>
    + ops::Shr<usize, Output = Self>
    + ops::Shl<usize, Output = Self>
    + cmp::Ord
    + fmt::Debug
{
    /// Converts this cost into `U256`
    fn as_u256(&self) -> U256;
    /// Tries to fit `U256` into this `Cost` type
    fn from_u256(val: U256) -> Result<Self, Error>;
    /// Convert to usize (may panic)
    fn as_usize(&self) -> usize;
    /// Add with overflow
    fn overflow_add(self, other: Self) -> (Self, bool);
    /// Multiple with overflow
    fn overflow_mul(self, other: Self) -> (Self, bool);
    /// Single-step full multiplication and shift: `(self*other) >> shr`
    /// Should not overflow on intermediate steps
    fn overflow_mul_shr(self, other: Self, shr: usize) -> (Self, bool);
}

impl CostType for U256 {
    fn as_u256(&self) -> U256 { *self }

    fn from_u256(val: U256) -> Result<Self, Error> { Ok(val) }

    fn as_usize(&self) -> usize { self.as_u64() as usize }

    fn overflow_add(self, other: Self) -> (Self, bool) { self.overflowing_add(other) }

    fn overflow_mul(self, other: Self) -> (Self, bool) { self.overflowing_mul(other) }

    fn overflow_mul_shr(self, other: Self, shr: usize) -> (Self, bool) {
        let x = self.full_mul(other);
        let U512(parts) = x;
        let overflow = (parts[4] | parts[5] | parts[6] | parts[7]) > 0;
        let U512(parts) = x >> shr;
        (U256([parts[0], parts[1], parts[2], parts[3]]), overflow)
    }
}

impl CostType for usize {
    fn as_u256(&self) -> U256 { U256::from(*self) }

    fn from_u256(val: U256) -> Result<Self, Error> {
        let res = val.low_u64() as usize;

        // validate if value fits into usize
        if U256::from(res) != val {
            return Err(Error::OutOfGas);
        }

        Ok(res)
    }

    fn as_usize(&self) -> usize { *self }

    fn overflow_add(self, other: Self) -> (Self, bool) { self.overflowing_add(other) }

    fn overflow_mul(self, other: Self) -> (Self, bool) { self.overflowing_mul(other) }

    fn overflow_mul_shr(self, other: Self, shr: usize) -> (Self, bool) {
        let (c, o) = U128::from(self).overflowing_mul(U128::from(other));
        let U128(parts) = c;
        let overflow = o | (parts[1] > 0);
        let U128(parts) = c >> shr;
        let result = parts[0] as usize;
        let overflow = overflow | (parts[0] > result as u64);
        (result, overflow)
    }
}

#[cfg(test)]
mod tests {
    use aion_types::U256;
    use rlp::RlpStream;
    use super::*;

    #[test]
    fn encode_calltype() {
        let mut s = RlpStream::new();
        s.append(&CallType::None); // 0
        assert_eq!(s.as_raw(), [0x80]);
        s.append(&CallType::Call); // 1
        assert_eq!(s.as_raw(), [0x80, 1]);
        s.append(&CallType::CallCode); // 2
        assert_eq!(s.as_raw(), [0x80, 1, 2]);
        s.append(&CallType::DelegateCall); // 3
        assert_eq!(s.as_raw(), [0x80, 1, 2, 3]);
        s.append(&CallType::StaticCall); // 4

        assert_eq!(s.as_raw(), [0x80u8, 1, 2, 3, 4]);
    }

    #[test]
    fn overflowing_add() {
        let left: U256 = U256::max_value();

        let res = left.overflowing_add(0.into());
        assert_eq!(res.1, false);
        let res = left.overflowing_add(1.into());
        assert_eq!(res.1, true);
    }

    #[test]
    fn should_calculate_overflow_mul_shr_without_overflow() {
        // given
        let num = 1048576;

        // when
        let (res1, o1) = U256::from(num).overflow_mul_shr(U256::from(num), 20);
        let (res2, o2) = num.overflow_mul_shr(num, 20);

        // then
        assert_eq!(res1, U256::from(num));
        assert!(!o1);
        assert_eq!(res2, num);
        assert!(!o2);
    }

    #[test]
    fn should_calculate_overflow_mul_shr_with_overflow() {
        // given
        let max = u64::max_value();
        let num1 = U256([max, max, max, max]);
        let num2 = usize::max_value();

        // when
        let (res1, o1) = num1.overflow_mul_shr(num1, 256);
        let (res2, o2) = num2.overflow_mul_shr(num2, 64);

        // then
        assert_eq!(res2, num2 - 1);
        assert!(o2);

        assert_eq!(res1, U256::max_value() - U256::one());
        assert!(o1);
    }

    #[test]
    fn should_validate_u256_to_usize_conversion() {
        // given
        let v = U256::from(usize::max_value()) + U256::from(1);

        // when
        let res = usize::from_u256(v);

        // then
        assert!(res.is_err());
    }
}
