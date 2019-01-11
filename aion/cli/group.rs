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

use std::str;

pub enum Group {
    Operating,
    Misc,
    Account,
    Network,
    Rpc,
    Ws,
    Ipc,
    Wallet,
    Stratum,
    Mining,
    Database,
    Log,
}

impl str::FromStr for Group {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "operating" | "aion" | "operating options" => Ok(Group::Operating),
            "misc" | "miscellaneous options" => Ok(Group::Misc),
            "account" | "account options" => Ok(Group::Account),
            "network" | "network options" => Ok(Group::Network),
            "rpc" | "rpc options" => Ok(Group::Rpc),
            "ws" | "websockets" | "websockets options" => Ok(Group::Ws),
            "ipc" | "ipc options" => Ok(Group::Ipc),
            "wallet" | "wallet options" => Ok(Group::Wallet),
            "stratum" | "stratum options" => Ok(Group::Stratum),
            "mining" | "sealing/mining options" => Ok(Group::Mining),
            "db" | "database" | "database options" => Ok(Group::Database),
            "log" | "log options" => Ok(Group::Log),
            _ => Err("invalid group name!!".into()),
        }
    }
}

impl<'a> From<Group> for &'a str {
    fn from(v: Group) -> &'a str {
        match v {
            Group::Operating => "aion",
            Group::Misc => "", //misc is in CMD only
            Group::Account => "account",
            Group::Network => "network",
            Group::Rpc => "rpc",
            Group::Ws => "websockets",
            Group::Ipc => "ipc",
            Group::Wallet => "wallet",
            Group::Stratum => "stratum",
            Group::Mining => "mining",
            Group::Database => "db",
            Group::Log => "log",
        }
    }
}
