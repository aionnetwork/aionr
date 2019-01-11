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

#[macro_use]
pub mod factory;
mod vmtype;

extern crate bit_set;
extern crate aion_types;
extern crate parking_lot;
extern crate heapsize;
extern crate blake2b as hash;
extern crate memory_cache;
extern crate acore_bytes as bytes;
extern crate common_types as types;
extern crate ajson;
extern crate fastvm;
extern crate libc;
#[macro_use]
extern crate log;

pub use self::factory::FastVMFactory;
pub use self::vmtype::VMType;
pub use fastvm::vm::{
    self,
    Vm,
    ActionParams,
    CallType,
    ActionValue,
    ParamsType,
    Ext,
    Error,
    ReturnData
};

pub use fastvm::EvmStatusCode;

pub use fastvm::env_info::{EnvInfo, LastHashes};
pub use fastvm::basetypes::constants;
