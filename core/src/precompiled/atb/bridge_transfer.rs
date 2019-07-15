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

#![allow(unused)]

use num_bigint::{BigInt};
use aion_types::H256;

#[derive(Debug, PartialEq)]
pub struct BridgeTransfer {
    tx_value: BigInt,
    recipient: H256,
    src_tx_hash: H256,
}

impl BridgeTransfer {
    fn new(tx_value: BigInt, recipient: H256, src_tx_hash: H256) -> Self {
        BridgeTransfer {
            tx_value,
            recipient,
            src_tx_hash,
        }
    }

    pub fn get_recipient(&self) -> H256 { return self.recipient.clone(); }

    pub fn get_src_transaction_hash(&self) -> H256 { return self.src_tx_hash.clone(); }

    pub fn get_transfer_value_bytearray(&self) -> Option<Vec<u8>> {
        let value_array = self.tx_value.to_signed_bytes_be();
        match value_array.len() <= 16 {
            true => {
                let mut ans = Vec::with_capacity(16);
                ans.extend(vec![0u8; 16 - value_array.len()]);
                ans.extend(value_array.clone());
                Some(ans)
            }
            false => None,
        }
    }

    pub fn get_transfer_value(&self) -> BigInt { return self.tx_value.clone(); }
}

pub fn get_instance(
    tx_value: BigInt,
    recipient: H256,
    src_tx_hash: H256,
) -> Option<BridgeTransfer>
{
    if tx_value.to_bytes_be().1.len() > 16 {
        None
    } else {
        Some(BridgeTransfer::new(tx_value, recipient, src_tx_hash))
    }
}

pub const TRANSFER_SIZE: usize = 80; // 32 + 32 + 16

#[cfg(test)]
mod test {
    use num_bigint::{BigInt, Sign};
    use super::get_instance;
    use aion_types::H256;

    #[test]
    fn bridge_transfer_instance() {
        let tx_value = BigInt::from(100);
        let recipient: H256 = [0xffu8; 32].into();
        let tx_hash: H256 = [0xfdu8; 32].into();
        assert_ne!(
            get_instance(tx_value.clone(), recipient.clone(), tx_hash.clone()),
            None
        );
        let tx_value = BigInt::from_bytes_be(Sign::Plus, b"1234567890123456");
        assert_ne!(
            get_instance(tx_value, recipient.clone(), tx_hash.clone()),
            None
        );
        let tx_value = BigInt::from_bytes_be(Sign::Plus, b"12345678901234567");
        assert_eq!(
            get_instance(tx_value, recipient.clone(), tx_hash.clone()),
            None
        );
    }
}
