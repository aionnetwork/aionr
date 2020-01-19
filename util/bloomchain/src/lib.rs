/*******************************************************************************
 * Copyright (c) 2015-2018 Parity Technologies (UK) Ltd.
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
#![warn(unused_extern_crates)]
extern crate ethbloom as bloom;

mod chain;
mod config;
mod database;
pub mod group;
mod number;
mod position;
mod filter;

pub use bloom::{Bloom, BloomRef, Input};
pub use chain::BloomChain;
pub use crate::config::Config;
pub use crate::database::BloomDatabase;
pub use crate::number::Number;
pub use crate::position::Position;
pub use crate::filter::Filter;
