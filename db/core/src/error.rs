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

use std::fmt;
#[derive(Debug, Clone)]
pub enum Error {
    NotFound(String),
    OpenError { name: String, desc: String },
    FlushError { name: String, desc: String },
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::NotFound(ref String) => write!(f, "db: {} not found", String),
            Error::OpenError {
                ref name,
                ref desc,
            } => write!(f, "db {} open faild: {}", name, desc),
            Error::FlushError {
                ref name,
                ref desc,
            } => write!(f, "db {} flush error: {}", name, desc),
            Error::Other(ref String) => {
                write!(f, "db crashed: {}, please clean and resync", String)
            }
        }
    }
}

impl ::std::error::Error for Error {
    fn description(&self) -> &str { "lower database error" }
}
