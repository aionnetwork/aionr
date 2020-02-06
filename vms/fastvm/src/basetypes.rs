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

#![allow(unused)]

use std::mem::transmute;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub type EvmHash = [u8; 32];
/// Aion address is now the public key which takes 32 bytes
pub type EvmAddress = [u8; 32];
/// Big-endian 128-bit word
pub type EvmWord = [u8; 16];

#[derive(Debug, Clone)]
pub struct DataWord {
    bytes_num: i32,
    bb: ByteBuffer,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ByteBuffer {
    hb: Vec<i8>,
    offset: i32,
    is_readonly: bool,
}

impl ByteBuffer {
    pub fn allocate(capacity: i32) -> ByteBuffer {
        if capacity < 0 {
            panic!("Invalid capacity: {}", capacity);
        }
        ByteBuffer {
            hb: Vec::<i8>::with_capacity(capacity as usize),
            offset: 0,
            is_readonly: false,
        }
    }
}

impl DataWord {
    pub fn new() -> DataWord {
        DataWord {
            bytes_num: 16,
            bb: ByteBuffer::allocate(16),
            data: Vec::<u8>::with_capacity(16),
        }
    }

    pub fn new_with_int(num: i32) -> DataWord {
        let mut dataword = DataWord {
            bytes_num: 16,
            bb: ByteBuffer::allocate(16),
            data: Vec::<u8>::new(),
        };

        let bytes: [u8; 4] = unsafe { transmute(num.to_be()) };
        //dataword.data.extend(&bytes);
        let dummy: [u8; 12] = [0; 12];
        dataword.data.extend(&dummy);
        dataword.data.extend(&bytes);
        dataword
    }

    pub fn new_with_long(num: i64) -> DataWord {
        let mut dataword = DataWord {
            bytes_num: 16,
            bb: ByteBuffer::allocate(16),
            data: Vec::<u8>::new(),
        };

        let bytes: [u8; 8] = unsafe { transmute(num.to_be()) };
        //dataword.data.extend(&bytes);
        let dummy: [u8; 8] = [0; 8];
        dataword.data.extend(&dummy);
        dataword.data.extend(&bytes);
        dataword
    }

    pub fn new_with_array(num: &[u8; 16]) -> DataWord {
        let mut dataword = DataWord {
            bytes_num: 16,
            bb: ByteBuffer::allocate(16),
            data: num.to_vec(),
        };

        dataword
    }

    pub fn one() -> DataWord { DataWord::new_with_int(1) }

    pub fn zero() -> DataWord { DataWord::new_with_int(0) }
}

struct EvmContext {}

use libc;

/// The message describing an EVM call,
/// including a zero-depth calls from a transaction origin.
#[repr(C)]
pub struct EvmMessage {
    pub recv_addr: EvmAddress, //< the receive address that accepts balance
    pub address: EvmAddress,   //< The destination of the message.
    pub caller: EvmAddress,    //< The sender of the message.
    /// The amount of Ether transferred with the message.
    pub value: EvmWord,
    pub input: *mut u8, // This MAY be NULL.
    /// The size of the message input data.
    ///
    /// If input_data is NULL this MUST be 0.
    pub input_size: usize,
    /// The optional hash of the code of the destination account.
    /// The null hash MUST be used when not specified.
    pub code_hash: EvmHash,
    pub gas: u64,   //< The amount of gas for message execution.
    pub depth: i32, //< The call depth.
    /// The kind of the call. For zero-depth calls ::EVM_CALL SHOULD be used.
    pub kind: i32,
    /// Additional flags modifying the call execution behavior.
    /// In the current version the only valid values are ::EVM_STATIC or 0.
    pub flags: i32,
}

/*
 * Virtual Machine constants
 */
pub mod constants {
    use aion_types::U256;

    pub const GAS_CODE_DEPOSIT: U256 = U256([1000, 0, 0, 0]);
    pub const GAS_CREATE_MIN: U256 = U256([200000, 0, 0, 0]);
    pub const GAS_CREATE_MAX: U256 = U256([5000000, 0, 0, 0]);
    pub const GAS_TX_DATA_ZERO: U256 = U256([4, 0, 0, 0]);
    pub const GAS_TX_DATA_NONZERO: U256 = U256([64, 0, 0, 0]);
    pub const GAS_CALL_MIN: U256 = U256([21000, 0, 0, 0]);
    pub const GAS_CALL_MAX: U256 = U256([2000000, 0, 0, 0]);
    pub const MAX_CALL_DEPTH: i32 = 128;
}
