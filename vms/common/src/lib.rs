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

extern crate acore_bytes as bytes;
extern crate aion_types;
extern crate ajson;
extern crate rlp;
extern crate blake2b;

pub mod traits;
pub mod avm;

#[cfg(test)]
mod test;

mod fvm;
mod types;

pub use fvm::{
    ExecutionResult as FvmExecutionResult,
    EnvInfo,
    CallType,
    ExecStatus,
    ActionParams,
    ActionValue,
    LastHashes
};
pub use avm::{ExecutionResult as AvmExecutionResult, AvmStatusCode};
pub use types::*;
