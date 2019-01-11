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

//! View onto transaction rlp
use bytes::Bytes;
use aion_types::{H256, U256, to_u256};
use blake2b::blake2b;
use rlp::Rlp;

/// View onto transaction rlp.
pub struct TransactionView<'a> {
    rlp: Rlp<'a>,
}

impl<'a> TransactionView<'a> {
    /// Creates new view onto block from raw bytes.
    pub fn new(bytes: &'a [u8]) -> TransactionView<'a> {
        TransactionView {
            rlp: Rlp::new(bytes),
        }
    }

    /// Creates new view onto block from rlp.
    pub fn new_from_rlp(rlp: Rlp<'a>) -> TransactionView<'a> {
        TransactionView {
            rlp: rlp,
        }
    }

    /// Return reference to underlaying rlp.
    pub fn rlp(&self) -> &Rlp<'a> { &self.rlp }

    /// Returns transaction hash.
    pub fn hash(&self) -> H256 { blake2b(self.rlp.as_raw()) }

    /// Get the nonce field of the transaction.
    pub fn nonce(&self) -> U256 { self.rlp.val_at(0) }

    /// Get the gas_price field of the transaction.
    pub fn gas_price(&self) -> U256 { to_u256(self.rlp.val_at::<Vec<u8>>(6), 32) }

    /// Get the gas field of the transaction.
    pub fn gas(&self) -> U256 { to_u256(self.rlp.val_at::<Vec<u8>>(5), 32) }

    /// Get the value field of the transaction.
    pub fn value(&self) -> U256 { self.rlp.val_at(2) }

    /// Get the data field of the transaction.
    pub fn data(&self) -> Bytes { self.rlp.val_at(3) }

    /// Get the timestamp of the transaction.
    pub fn timestamp(&self) -> Bytes { self.rlp.val_at(4) }

    /// Get the v field of the transaction.
    pub fn v(&self) -> u8 {
        let r: u16 = self.rlp.val_at(8);
        r as u8
    }

    /// Get the r field of the transaction.
    pub fn r(&self) -> U256 { self.rlp.val_at(9) }

    /// Get the s field of the transaction.
    pub fn s(&self) -> U256 { self.rlp.val_at(10) }
}

#[cfg(test)]
mod tests {
    use rustc_hex::FromHex;
    use super::TransactionView;

    #[test]
    fn test_transaction_view() {
        let rlp = "f89b80a0a054340a3152d10006b66c4248cfa73e5725056294081c476c0e67ef5ad25334820fff80880005748de2c04d69830e57e0841f38b2e601b8608bc5c4e5599afac7cb0efcb0010540017dda3e80870bb543b356867b2a8cacbfcdffb6e1b3784f4497b6121502a0991077c657e4f8e5b68f24b3644964fcf6935a3d6735521ae94c1a361d692c04769e8e8fb19392a9badd73002ce13dbf5c08".from_hex().unwrap();

        let view = TransactionView::new(&rlp);
        assert_eq!(view.nonce(), 0.into());
        assert_eq!(view.gas_price(), 0x1f38b2e6.into());
        assert_eq!(view.gas(), 0xe57e0.into());
        assert_eq!(view.value(), 0xfff.into());
    }
}
