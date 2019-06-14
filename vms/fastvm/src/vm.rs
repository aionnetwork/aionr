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

use aion_types::{U256, U128, U512};
use ffi::EvmStatusCode;
use std::{fmt, ops, cmp};
use vm_common::{ReturnData};

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

// TODO: should be a trait, possible to avoid cloning everything from a Transaction(/View).
/// Action (call/create) input params. Everything else should be specified in Externalities.
// #[derive(Clone, Debug)]
// pub struct ActionParams {
//     /// Address of currently executed code.
//     pub code_address: Address,
//     /// Hash of currently executed code.
//     pub code_hash: Option<H256>,
//     /// Receive address. Usually equal to code_address,
//     /// except when called using CALLCODE.
//     pub address: Address,
//     /// Sender of current part of the transaction.
//     pub sender: Address,
//     /// Transaction initiator.
//     pub origin: Address,
//     /// Gas paid up front for transaction execution
//     pub gas: U256,
//     /// Gas price.
//     pub gas_price: U256,
//     /// Transaction value.
//     pub value: ActionValue,
//     /// Code being executed.
//     pub code: Option<Arc<Bytes>>,
//     /// Input data.
//     pub data: Option<Bytes>,
//     /// Type of call
//     pub call_type: CallType,
//     /// Flag to indicate if the call is static
//     pub static_flag: bool,
//     /// Param types encoding
//     pub params_type: ParamsType,
//     /// transaction hash
//     pub transaction_hash: H256,
//     /// original transaction hash
//     pub original_transaction_hash: H256,
// }

// impl Default for ActionParams {
//     /// Returns default ActionParams initialized with zeros
//     fn default() -> ActionParams {
//         ActionParams {
//             code_address: Address::new(),
//             code_hash: Some(BLAKE2B_EMPTY),
//             address: Address::new(),
//             sender: Address::new(),
//             origin: Address::new(),
//             gas: U256::zero(),
//             gas_price: U256::zero(),
//             value: ActionValue::Transfer(U256::zero()),
//             code: None,
//             data: None,
//             call_type: CallType::None,
//             static_flag: false,
//             params_type: ParamsType::Separate,
//             transaction_hash: H256::default(),
//             original_transaction_hash: H256::default(),
//         }
//     }
// }

// impl From<::ajson::vm::Transaction> for ActionParams {
//     fn from(t: ::ajson::vm::Transaction) -> Self {
//         let address: Address = t.address.into();
//         ActionParams {
//             code_address: Address::new(),
//             code_hash: Some(blake2b(&*t.code)),
//             address: address,
//             sender: t.sender.into(),
//             origin: t.origin.into(),
//             code: Some(Arc::new(t.code.into())),
//             data: Some(t.data.into()),
//             gas: t.gas.into(),
//             gas_price: t.gas_price.into(),
//             value: ActionValue::Transfer(t.value.into()),
//             call_type: match address.is_zero() {
//                 true => CallType::None,
//                 false => CallType::Call,
//             },
//             static_flag: false,
//             params_type: ParamsType::Separate,
//             transaction_hash: H256::default(),
//             original_transaction_hash: H256::default(),
//         }
//     }
// }

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