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
//! Error utils

use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// Error indicating an expected value was not found.
pub struct Mismatch<T> {
    /// Value expected.
    pub expected: T,
    /// Value found.
    pub found: T,
}

impl<T: fmt::Display> fmt::Display for Mismatch<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!(
            "Expected {}, found {}",
            self.expected, self.found
        ))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// Error indicating value found is outside of a valid range.
pub struct OutOfBounds<T> {
    /// Minimum allowed value.
    pub min: Option<T>,
    /// Maximum allowed value.
    pub max: Option<T>,
    /// Value found.
    pub found: T,
}

impl<T: fmt::Display> fmt::Display for OutOfBounds<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match (self.min.as_ref(), self.max.as_ref()) {
            (Some(min), Some(max)) => format!("Min={}, Max={}", min, max),
            (Some(min), _) => format!("Min={}", min),
            (_, Some(max)) => format!("Max={}", max),
            (None, None) => "".into(),
        };

        f.write_fmt(format_args!("Value {} out of bounds. {}", self.found, msg))
    }
}
