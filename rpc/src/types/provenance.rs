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

//! Request Provenance

use std::fmt;
use types::H256;

/// RPC request origin
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Origin {
    /// RPC server (includes request origin)
    #[serde(rename = "rpc")]
    Rpc(String),
    /// IPC server (includes session hash)
    #[serde(rename = "ipc")]
    Ipc(H256),
    /// WS server (includes session hash)
    #[serde(rename = "ws")]
    Ws {
        /// Websocket origin
        origin: String,
        /// Session id
        session: H256,
    },
    /// Unknown
    #[serde(rename = "unknown")]
    Unknown,
}

impl Default for Origin {
    fn default() -> Self { Origin::Unknown }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Origin::Rpc(ref origin) => write!(f, "{} via RPC", origin),
            Origin::Ipc(ref session) => write!(f, "IPC (session: {})", session),
            Origin::Ws {
                ref origin,
                ref session,
            } => write!(f, "{} via WebSocket (session: {})", origin, session),
            Origin::Unknown => write!(f, "unknown origin"),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json;
    use super::Origin;

    #[test]
    fn should_serialize_origin() {
        // given
        let o1 = Origin::Rpc("test service".into());
        let o2 = Origin::Ipc(5.into());
        let o3 = Origin::Unknown;
        let o4 = Origin::Ws {
            origin: "test origin".into(),
            session: 5.into(),
        };
        // when
        let res1 = serde_json::to_string(&o1).unwrap();
        let res2 = serde_json::to_string(&o2).unwrap();
        let res3 = serde_json::to_string(&o3).unwrap();
        let res4 = serde_json::to_string(&o4).unwrap();
        // then
        assert_eq!(res1, r#"{"rpc":"test service"}"#);
        assert_eq!(
            res2,
            r#"{"ipc":"0x0000000000000000000000000000000000000000000000000000000000000005"}"#
        );
        assert_eq!(res3, r#""unknown""#);
        assert_eq!(res4, r#"{"ws":{"origin":"test origin","session":"0x0000000000000000000000000000000000000000000000000000000000000005"}}"#);
    }
}
