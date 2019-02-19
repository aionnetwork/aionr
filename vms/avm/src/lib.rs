extern crate core;
extern crate rustc_hex;
extern crate libc;
extern crate num_bigint;
extern crate rjni;
extern crate aion_types;
extern crate vm_common;
extern crate avm_abi;

#[macro_use]
extern crate lazy_static;

pub mod avm;
pub mod callback;
pub mod codec;
pub mod types;

pub use avm::AVM;
pub use avm::AVMExt;
