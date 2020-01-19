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

use std::collections::HashMap;
use crate::precompiled::atb::bridge_utilities::to_signature;

pub enum BridgeFuncSig {
    ChangeOwner,
    AcceptOwnership,
    InitializeRing,
    AddRingMember,
    RemoveRingMember,
    SetRelayer,
    SubmitBundle,
    Owner,
    NewOwner,
    ActionMap,
    RingMap,
    RingLocked,
    MinThresh,
    MemberCount,
    Relayer,
}

lazy_static! {
    static ref HASHMAP: HashMap<[u8; 4], BridgeFuncSig> = {
        let mut m = HashMap::new();
        m.insert(
            to_signature("changeOwner(address)"),
            BridgeFuncSig::ChangeOwner,
        );
        m.insert(
            to_signature("acceptOwnership()"),
            BridgeFuncSig::AcceptOwnership,
        );
        m.insert(
            to_signature("initializeRing(address[])"),
            BridgeFuncSig::InitializeRing,
        );
        m.insert(
            to_signature("addRingMember(address)"),
            BridgeFuncSig::AddRingMember,
        );
        m.insert(
            to_signature("removeRingMember(address)"),
            BridgeFuncSig::RemoveRingMember,
        );
        m.insert(
            to_signature("setRelayer(address)"),
            BridgeFuncSig::SetRelayer,
        );
        m.insert(
            to_signature(
                "submitBundle(bytes32,bytes32[],address[],uint128[],bytes32[],bytes32[],bytes32[])",
            ),
            BridgeFuncSig::SubmitBundle,
        );
        m.insert(to_signature("owner()"), BridgeFuncSig::Owner);
        m.insert(to_signature("newOwner()"), BridgeFuncSig::NewOwner);
        m.insert(to_signature("actionMap(bytes32)"), BridgeFuncSig::ActionMap);
        m.insert(to_signature("ringMap(address)"), BridgeFuncSig::RingMap);
        m.insert(to_signature("ringLocked()"), BridgeFuncSig::RingLocked);
        m.insert(to_signature("minThresh()"), BridgeFuncSig::MinThresh);
        m.insert(to_signature("memberCount()"), BridgeFuncSig::MemberCount);
        m.insert(to_signature("relayer()"), BridgeFuncSig::Relayer);
        m
    };
}

impl BridgeFuncSig {
    #[cfg(test)]
    pub fn hash(&self) -> [u8; 4] {
        match *self {
            BridgeFuncSig::ChangeOwner => to_signature("changeOwner(address)"),
            BridgeFuncSig::AcceptOwnership => to_signature("acceptOwnership()"),
            BridgeFuncSig::InitializeRing => to_signature("initializeRing(address[])"),
            BridgeFuncSig::AddRingMember => to_signature("addRingMember(address)"),
            BridgeFuncSig::RemoveRingMember => to_signature("removeRingMember(address)"),
            BridgeFuncSig::SetRelayer => to_signature("setRelayer(address)"),
            BridgeFuncSig::SubmitBundle => {
                to_signature(
                    "submitBundle(bytes32,bytes32[],address[],uint128[],bytes32[],bytes32[],\
                     bytes32[])",
                )
            }
            BridgeFuncSig::Owner => to_signature("owner()"),
            BridgeFuncSig::NewOwner => to_signature("newOwner()"),
            BridgeFuncSig::ActionMap => to_signature("actionMap(bytes32)"),
            BridgeFuncSig::RingMap => to_signature("ringMap(address)"),
            BridgeFuncSig::RingLocked => to_signature("ringLocked()"),
            BridgeFuncSig::MinThresh => to_signature("minThresh()"),
            BridgeFuncSig::MemberCount => to_signature("memberCount()"),
            BridgeFuncSig::Relayer => to_signature("relayer()"),
        }
    }

    pub fn from_hash(bytes: &[u8; 4]) -> Option<&BridgeFuncSig> { HASHMAP.get(bytes) }
}

#[cfg(test)]
mod tests {
    use rustc_hex::FromHex;
    use rustc_hex::ToHex;
    use super::BridgeFuncSig;

    #[test]
    fn verify_hash() {
        assert_eq!("a6f9dae1", BridgeFuncSig::ChangeOwner.hash().to_hex());
        assert_eq!("79ba5097", BridgeFuncSig::AcceptOwnership.hash().to_hex());
        assert_eq!("1664500e", BridgeFuncSig::InitializeRing.hash().to_hex());
        assert_eq!("06c8dcde", BridgeFuncSig::AddRingMember.hash().to_hex());
        assert_eq!("67a3914e", BridgeFuncSig::RemoveRingMember.hash().to_hex());
        assert_eq!("6548e9bc", BridgeFuncSig::SetRelayer.hash().to_hex());
        assert_eq!("46d1cc29", BridgeFuncSig::SubmitBundle.hash().to_hex());
        assert_eq!("8da5cb5b", BridgeFuncSig::Owner.hash().to_hex());
        assert_eq!("d4ee1d90", BridgeFuncSig::NewOwner.hash().to_hex());
        assert_eq!("18ed912f", BridgeFuncSig::ActionMap.hash().to_hex());
        assert_eq!("b22fce40", BridgeFuncSig::RingMap.hash().to_hex());
        assert_eq!("1a286d59", BridgeFuncSig::RingLocked.hash().to_hex());
        assert_eq!("6c44b227", BridgeFuncSig::MinThresh.hash().to_hex());
        assert_eq!("11aee380", BridgeFuncSig::MemberCount.hash().to_hex());
        assert_eq!("8406c079", BridgeFuncSig::Relayer.hash().to_hex());
    }

    #[test]
    fn verify_from_hash() {
        let mut array = [0u8; 4];

        array.copy_from_slice(&"a6f9dae1".from_hex().unwrap()[0..4]);
        match BridgeFuncSig::from_hash(&array).unwrap() {
            BridgeFuncSig::ChangeOwner => {}
            _ => {
                panic!("Signature does not mathc!");
            }
        }

        array.copy_from_slice(&"8406c079".from_hex().unwrap()[0..4]);
        match BridgeFuncSig::from_hash(&array).unwrap() {
            BridgeFuncSig::Relayer => {}
            _ => {
                panic!("Signature does not mathc!");
            }
        }

        array.copy_from_slice(&"ffffffff".from_hex().unwrap()[0..4]);
        assert!(BridgeFuncSig::from_hash(&array).is_none());
    }
}
