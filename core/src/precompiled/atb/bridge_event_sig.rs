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

use precompiled::atb::bridge_utilities::to_event_signature;

pub enum BridgeEventSig {
    ChangedOwner,
    AddMember,
    RemoveMember,
    ProcessedBundle,
    Distributed,
    SuccessfulTxHash,
}

impl BridgeEventSig {
    pub fn hash(&self) -> [u8; 32] {
        match *self {
            BridgeEventSig::ChangedOwner => to_event_signature("ChangedOwner(address)"),
            BridgeEventSig::AddMember => to_event_signature("AddMember(address)"),
            BridgeEventSig::RemoveMember => to_event_signature("RemoveMember(address)"),
            BridgeEventSig::ProcessedBundle => {
                to_event_signature("ProcessedBundle(bytes32,bytes32)")
            }
            BridgeEventSig::Distributed => {
                to_event_signature("Distributed(bytes32,address,uint128)")
            }
            BridgeEventSig::SuccessfulTxHash => to_event_signature("SuccessfulTxHash(bytes32)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use rustc_hex::ToHex;
    use super::BridgeEventSig;

    #[test]
    fn verify_hash() {
        assert_eq!(
            "a701229f4b9ddf00aa1c7228d248e6320ee7c581d856ddfba036e73947cd0d13",
            BridgeEventSig::ChangedOwner.hash().to_hex()
        );
        assert_eq!(
            "1a2323d99020f3db8e6ea85b1eea81e5bf422695877228e3d8a0241d7e957a6c",
            BridgeEventSig::AddMember.hash().to_hex()
        );
        assert_eq!(
            "7693a3e9eac51f172f145e6f54bc5554168997a1f4efb40f3fad091aa7cfb0e7",
            BridgeEventSig::RemoveMember.hash().to_hex()
        );
        assert_eq!(
            "1fa305c7f8521af161de570532762ed7a60199cde79e18e1d259af3459562521",
            BridgeEventSig::ProcessedBundle.hash().to_hex()
        );
        assert_eq!(
            "474886369a779dce80c9a0ff6858efe3459a0ce6e55cc4488da3369aef6dd95c",
            BridgeEventSig::Distributed.hash().to_hex()
        );
        assert_eq!(
            "32352009ab75df12033696171abe663b6adf6e5bb22a2271c0f8fcca0ac9e52e",
            BridgeEventSig::SuccessfulTxHash.hash().to_hex()
        );
    }
}
