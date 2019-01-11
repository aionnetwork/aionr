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

use std::error::Error;
use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum P2pError {
    DefaultP2pError(u32),
}

impl fmt::Display for P2pError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            P2pError::DefaultP2pError(error_code) => write!(f, "NetError {}", error_code),
        }
    }
}

impl Error for P2pError {
    fn description(&self) -> &str {
        match *self {
            P2pError::DefaultP2pError(_) => "Default P2P Error",
        }
    }
}

#[test]
fn p2p_error_test() {
    println!("Message: {}", P2pError::DefaultP2pError(1));
    let des = P2pError::DefaultP2pError(666).description();
    println!("Message: {}", des);
}
