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

//! Spec seal.

use rlp::RlpStream;
use aion_types::H256;
use ajson;
use bytes::Bytes;

pub struct POWEquihash {
    pub nonce: H256,
    pub solution: Bytes,
}

impl Into<Generic> for POWEquihash {
    fn into(self) -> Generic {
        let mut s = RlpStream::new_list(2);
        s.append(&self.nonce).append(&self.solution);
        Generic(s.out())
    }
}

pub struct Generic(pub Vec<u8>);

pub enum Seal {
    POWEquihash(POWEquihash),
}

impl From<ajson::spec::Seal> for Seal {
    fn from(s: ajson::spec::Seal) -> Self {
        match s {
            ajson::spec::Seal::POWEquihash(pow_equihash) => {
                Seal::POWEquihash(POWEquihash {
                    nonce: pow_equihash.nonce.into(),
                    solution: pow_equihash.solution.into(),
                })
            }
        }
    }
}

impl Into<Generic> for Seal {
    fn into(self) -> Generic {
        match self {
            Seal::POWEquihash(pow_equihash) => pow_equihash.into(),
        }
    }
}
