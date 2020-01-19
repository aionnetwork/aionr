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

//! Vm execution env.

use crate::bytes::Bytes;
use crate::uint::Uint;
use crate::hash::H256;
use crate::blockchain::State;
use crate::vm::{Transaction, Call, Env};

/// Reporesents vm execution environment before and after exeuction of transaction.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Vm {
    /// Contract calls made internaly by executed transaction.
    #[serde(rename = "callcreates")]
    pub calls: Option<Vec<Call>>,
    /// Env info.
    pub env: Env,
    /// Executed transaction
    #[serde(rename = "exec")]
    pub transaction: Transaction,
    /// Gas left after transaction execution.
    #[serde(rename = "gas")]
    pub gas_left: Option<Uint>,
    /// Hash of logs created during execution of transaction.
    pub logs: Option<H256>,
    /// Transaction output.
    #[serde(rename = "out")]
    pub output: Option<Bytes>,
    /// Post execution vm state.
    #[serde(rename = "post")]
    pub post_state: Option<State>,
    /// Pre execution vm state.
    #[serde(rename = "pre")]
    pub pre_state: State,
}

impl Vm {
    /// Returns true if transaction execution run out of gas.
    pub fn out_of_gas(&self) -> bool { self.calls.is_none() }
}

#[cfg(test)]
mod tests {
    use serde_json;
    use crate::vm::Vm;

    #[test]
    fn vm_deserialization() {
        let s = r#"{
            "callcreates" : [
            ],
            "env" : {
                "currentCoinbase" : "0000000000000000000000000000000000000000000000000000000000000001",
                "currentDifficulty" : "0x0100",
                "currentGasLimit" : "0x0f4240",
                "currentNumber" : "0x00",
                "currentTimestamp" : "0x01"
            },
            "exec" : {
                "address" : "0000000000000000000000000000000000000000000000000000000000000001",
                "caller" : "0000000000000000000000000000000000000000000000000000000000000001",
                "code" : "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff01600055",
                "data" : "0x",
                "gas" : "0x0186a0",
                "gasPrice" : "0x5af3107a4000",
                "origin" : "0000000000000000000000000000000000000000000000000000000000000001",
                "value" : "0x0de0b6b3a7640000"
            },
            "gas" : "0x013874",
            "logs" : "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            "out" : "0x",
            "network" : "Frontier",
            "post" : {
                "0000000000000000000000000000000000000000000000000000000000000001" : {
                    "balance" : "0x0de0b6b3a7640000",
                    "code" : "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff01600055",
                    "nonce" : "0x00",
                    "storage" : {
                        "0x00" : "0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe"
                    }
                }
            },
            "pre" : {
                "0000000000000000000000000000000000000000000000000000000000000001" : {
                    "balance" : "0x0de0b6b3a7640000",
                    "code" : "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff01600055",
                    "nonce" : "0x00",
                    "storage" : {
                    }
                }
            }
        }"#;
        let _deserialized: Vm = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
