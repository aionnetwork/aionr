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

//! Trace filter related types

use std::ops::Range;
use aion_types::Address;
use ids::BlockId;

/// Easy to use trace filter.
pub struct Filter {
    /// Range of filtering.
    pub range: Range<BlockId>,
    /// From address.
    pub from_address: Vec<Address>,
    /// To address.
    pub to_address: Vec<Address>,
    /// Output offset
    pub after: Option<usize>,
    /// Output amount
    pub count: Option<usize>,
}
