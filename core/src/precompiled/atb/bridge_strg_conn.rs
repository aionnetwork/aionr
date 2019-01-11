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

use aion_types::{H256, Address};
use precompiled::builtin::BuiltinExt;
use blake2b::blake2b;

type BridgeStrgKey = [u8; 16];

fn data_with_byte(input: u8) -> BridgeStrgKey {
    let mut ans = [0u8; 16];
    ans[15] = input;
    ans
}

// get big-endian i32 from bytes
fn from_signed_bytes_be(bytes: &[u8]) -> i32 {
    unsafe {
        ::std::mem::transmute(
            ((bytes[0] as u32) << 24)
                + ((bytes[1] as u32) << 16)
                + ((bytes[2] as u32) << 8)
                + (bytes[3] as u32),
        )
    }
}

#[derive(Clone)]
pub struct BridgeStorageConnector {
    owner: BridgeStrgKey,
    new_owner: BridgeStrgKey,
    member_count: BridgeStrgKey,
    min_thresh: BridgeStrgKey,
    ring_locked: BridgeStrgKey,
    relayer: BridgeStrgKey,
    initialized: BridgeStrgKey,

    bundle_map: u8,
    active_map: u8,

    contract_addr: Address,
}

impl BridgeStorageConnector {
    pub fn new(contract_addr: Address) -> Self {
        BridgeStorageConnector {
            owner: data_with_byte(0x0u8),
            new_owner: data_with_byte(0x1u8),
            member_count: data_with_byte(0x2u8),
            min_thresh: data_with_byte(0x3u8),
            ring_locked: data_with_byte(0x4u8),
            relayer: data_with_byte(0x5u8),
            initialized: data_with_byte(0x42u8),

            bundle_map: 0x1u8,
            active_map: 0x2u8,

            contract_addr: contract_addr.clone(),
        }
    }

    pub fn set_initialized(&self, ext: &mut BuiltinExt, init: bool) {
        let value = match init {
            true => data_with_byte(0x1u8),
            false => data_with_byte(0x0u8),
        };
        ext.set_storage(self.initialized.into(), value.into());
    }

    pub fn get_initialized(&self, ext: &mut BuiltinExt) -> bool {
        let data = ext.storage_at(&self.initialized.into());
        (data[15] & 0x1) == 1
    }

    pub fn set_owner(&self, ext: &mut BuiltinExt, owner_addr: Address) {
        ext.set_storage_dword(self.owner.into(), owner_addr);
    }

    // None means some error happens; may not trigger in Aion
    pub fn get_owner(&self, ext: &mut BuiltinExt) -> Address {
        ext.storage_at_dword(&self.owner.into())
    }

    pub fn set_new_owner(&self, ext: &mut BuiltinExt, owner_addr: Address) {
        ext.set_storage_dword(self.new_owner.into(), owner_addr);
    }

    pub fn get_new_owner(&self, ext: &mut BuiltinExt) -> Address {
        ext.storage_at_dword(&self.new_owner.into())
    }

    pub fn set_relayer(&self, ext: &mut BuiltinExt, relayer_addr: Address) {
        ext.set_storage_dword(self.relayer.into(), relayer_addr);
    }

    pub fn get_relayer(&self, ext: &mut BuiltinExt) -> Address {
        ext.storage_at_dword(&self.relayer.into())
    }

    pub fn set_member_count(&self, ext: &mut BuiltinExt, amount: i32) {
        let bytes: [u8; 4] = unsafe { ::std::mem::transmute(amount.to_be()) };
        let mut data = Vec::new();
        data.extend(&[0u8; 12]);
        data.extend(&bytes);
        ext.set_storage(self.member_count.into(), data.as_slice().into());
    }

    pub fn get_member_count(&self, ext: &mut BuiltinExt) -> i32 {
        let count_word = ext.storage_at(&self.member_count.into());
        let bytes: [u8; 16] = count_word.into();
        from_signed_bytes_be(&bytes[12..16])
    }

    pub fn set_min_thresh(&self, ext: &mut BuiltinExt, amount: i32) {
        let bytes: [u8; 4] = unsafe { ::std::mem::transmute(amount.to_be()) };
        let mut data = Vec::new();
        data.extend(&[0u8; 12]);
        data.extend(&bytes);
        ext.set_storage(self.min_thresh.into(), data.as_slice().into());
    }

    pub fn get_min_thresh(&self, ext: &mut BuiltinExt) -> i32 {
        let thresh_word = ext.storage_at(&self.min_thresh.into());
        let bytes: [u8; 16] = thresh_word.into();
        from_signed_bytes_be(&bytes[12..16])
    }

    pub fn set_ring_locked(&self, ext: &mut BuiltinExt, value: bool) {
        let value = match value {
            true => data_with_byte(0x1u8),
            false => data_with_byte(0x0u8),
        };
        ext.set_storage(self.ring_locked.into(), value.into());
    }

    pub fn get_ring_locked(&self, ext: &mut BuiltinExt) -> bool {
        let data = ext.storage_at(&self.ring_locked.into());
        (data[15] & 0x1) == 1
    }

    pub fn set_active_member(&self, ext: &mut BuiltinExt, key: H256, value: bool) {
        let mut _data = Vec::<u8>::new();
        _data.extend_from_slice(&[self.active_map]);
        let key_bytes: [u8; 32] = key.into();
        _data.extend_from_slice(&key_bytes);
        let hash = blake2b(_data.as_slice());
        let mut my_key = [0u8; 16];
        my_key.copy_from_slice(&hash[16..32]);
        let data = match value {
            true => data_with_byte(0x1u8),
            false => data_with_byte(0x0u8),
        };
        ext.set_storage(my_key.into(), data.into());
    }

    pub fn get_active_member(&self, ext: &mut BuiltinExt, key: H256) -> bool {
        let mut _data = Vec::<u8>::new();
        _data.extend_from_slice(&[self.active_map]);
        let key_bytes: [u8; 32] = key.into();
        _data.extend_from_slice(&key_bytes);
        let hash = blake2b(_data.as_slice());
        let mut my_key = [0u8; 16];
        my_key.copy_from_slice(&hash[16..32]);
        let ans = ext.storage_at(&my_key.into());
        (ans[15] & 0x1) == 1
    }

    pub fn set_bundle(&self, ext: &mut BuiltinExt, key: H256, value: H256) {
        let mut _data = Vec::<u8>::new();
        _data.extend_from_slice(&[self.bundle_map]);
        let key_bytes: [u8; 32] = key.into();
        _data.extend_from_slice(&key_bytes);
        let hash = blake2b(_data.as_slice());
        let mut my_key = [0u8; 16];
        my_key.copy_from_slice(&hash[16..32]);
        ext.set_storage_dword(my_key.into(), value);
    }

    // None means some error happened
    pub fn get_bundle(&self, ext: &mut BuiltinExt, key: H256) -> H256 {
        let mut _data = Vec::<u8>::new();
        _data.extend_from_slice(&[self.bundle_map]);
        let key_bytes: [u8; 32] = key.into();
        _data.extend_from_slice(&key_bytes);
        let hash = blake2b(_data.as_slice());
        let mut my_key = [0u8; 16];
        my_key.copy_from_slice(&hash[16..32]);
        ext.storage_at_dword(&my_key.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_i32_from_bytes_be() {
        let data = [0u8; 16];
        assert_eq!(data[12..16].len(), 4 as usize);
        let value = from_signed_bytes_be(&[0, 0, 0, 0]);
        assert_eq!(value, 0);
        let value = from_signed_bytes_be(&[128, 0, 0, 0]);
        assert_eq!(value, -2147483648);
        let value = from_signed_bytes_be(&[0xff, 0xff, 0xff, 0xff]);
        assert_eq!(value, -1);
        let value = from_signed_bytes_be(&[128, 0, 0, 1]);
        assert_eq!(value, -2147483647);
        let value = from_signed_bytes_be(&[127, 0, 0, 1]);
        assert_eq!(value, 2130706433);
    }

    #[test]
    fn check_dataword_from_byte() {
        let value = data_with_byte(0xfeu8);
        assert_eq!(value, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xfe]);
    }

    #[test]
    fn check_dataword_from_i32() {
        let bytes: [u8; 4] = unsafe { ::std::mem::transmute(12i32.to_be()) };
        let mut data = Vec::<u8>::new();
        data.extend(&[0u8; 12]);
        data.extend(&bytes);
        assert_eq!(
            data.as_slice(),
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 12]
        );

        data.clear();
        let bytes: [u8; 4] = unsafe { ::std::mem::transmute(1024i32.to_be()) };
        data.extend(&[0u8; 12]);
        data.extend(&bytes);
        assert_eq!(
            data.as_slice(),
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0]
        );
    }

    #[test]
    fn connector() {
        let connector = BridgeStorageConnector::new([0u8; 32].into());
        assert_eq!(
            connector.owner,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0u8]
        );
        assert_eq!(
            connector.new_owner,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x1u8]
        );
        assert_eq!(
            connector.member_count,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x2u8]
        );
        assert_eq!(
            connector.min_thresh,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x3u8]
        );
        assert_eq!(
            connector.ring_locked,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x4u8]
        );
        assert_eq!(
            connector.relayer,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x5u8]
        );
        assert_eq!(
            connector.initialized,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x42u8]
        );
        assert_eq!(connector.bundle_map, 0x1u8);
        assert_eq!(connector.active_map, 0x2u8);
    }
}
