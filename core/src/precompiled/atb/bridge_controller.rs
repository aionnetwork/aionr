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

use std::cmp;
use acore_bytes::Bytes;
use aion_types::{H256, U256};
use vms::{ReturnData, ExecStatus, ExecutionResult};
use num_bigint::BigInt;
use num_bigint::ToBigInt;

use key::{Ed25519Signature, public_to_address_ed25519, verify_signature_ed25519};
use precompiled::precompiled_utilities::{WORD_LENGTH, pad};
use precompiled::atb::err_code::ErrCode;
use precompiled::builtin::BuiltinExt;
use precompiled::atb::bridge_event_sig::BridgeEventSig;
use precompiled::atb::bridge_strg_conn::BridgeStorageConnector;
use precompiled::atb::bridge_transfer::BridgeTransfer;
use precompiled::atb::bridge_utilities::compute_bundle_hash;

pub struct BridgeController {
    connector: BridgeStorageConnector,
    contract_address: H256,
    owner_address: H256,
}

impl BridgeController {
    pub fn new(
        storage_connector: BridgeStorageConnector,
        contract_address: H256,
        owner_address: H256,
    ) -> Self
    {
        BridgeController {
            connector: storage_connector,
            contract_address,
            owner_address,
        }
    }

    pub fn initialize(&self, ext: &mut dyn BuiltinExt) {
        if !self.connector.get_initialized(ext) {
            self.connector.set_owner(ext, self.owner_address);
            self.connector.set_initialized(ext, true);
        }
    }

    /// Checks whether the given address is the owner of the contract or not.
    fn is_owner(&self, ext: &mut dyn BuiltinExt, address: H256) -> bool {
        self.connector.get_owner(ext) == address
    }

    /// Checks whether the given address is the intended newOwner of the contract
    fn is_new_owner(&self, ext: &mut dyn BuiltinExt, address: H256) -> bool {
        self.connector.get_new_owner(ext) == address
    }

    /// logic
    pub fn set_new_owner(
        &self,
        ext: &mut dyn BuiltinExt,
        caller: H256,
        new_owner: H256,
    ) -> Result<(), ErrCode>
    {
        if !self.is_owner(ext, caller) {
            return Err(ErrCode::NotOwner);
        }
        self.connector.set_new_owner(ext, new_owner);
        Ok(())
    }

    pub fn accept_ownership(&self, ext: &mut dyn BuiltinExt, caller: H256) -> Result<(), ErrCode> {
        if !self.is_new_owner(ext, caller) {
            return Err(ErrCode::NotNewOwner);
        }
        self.connector.set_owner(ext, caller);
        self.connector
            .set_new_owner(ext, H256::from_slice(&vec![0; WORD_LENGTH]));
        self.emit_change_owner(ext, caller);
        Ok(())
    }

    fn is_relayer(&self, ext: &mut dyn BuiltinExt, caller: H256) -> bool {
        self.connector.get_relayer(ext) == caller
    }

    pub fn set_relayer(
        &self,
        ext: &mut dyn BuiltinExt,
        caller: H256,
        new_owner: H256,
    ) -> Result<(), ErrCode>
    {
        if !self.is_owner(ext, caller) {
            return Err(ErrCode::NotOwner);
        }
        self.connector.set_relayer(ext, new_owner);
        Ok(())
    }

    fn is_ring_locked(&self, ext: &mut dyn BuiltinExt) -> bool {
        self.connector.get_ring_locked(ext)
    }

    fn is_ring_member(&self, ext: &mut dyn BuiltinExt, address: H256) -> bool {
        self.connector.get_active_member(ext, address)
    }

    pub fn ring_initialize(
        &self,
        ext: &mut dyn BuiltinExt,
        caller: H256,
        members: Vec<H256>,
    ) -> Result<(), ErrCode>
    {
        if !self.is_owner(ext, caller) {
            return Err(ErrCode::NotOwner);
        }
        if self.is_ring_locked(ext) {
            return Err(ErrCode::RingLocked);
        }
        let thresh: i32 = threshold_ratio(members.len() as i32);
        self.connector.set_member_count(ext, members.len() as i32);
        self.connector.set_min_thresh(ext, thresh);
        for m in members {
            self.connector.set_active_member(ext, m, true);
        }
        self.connector.set_ring_locked(ext, true);
        Ok(())
    }

    pub fn ring_add_member(
        &self,
        ext: &mut dyn BuiltinExt,
        caller: H256,
        address: H256,
    ) -> Result<(), ErrCode>
    {
        if !self.is_owner(ext, caller) {
            return Err(ErrCode::NotOwner);
        }
        if !self.is_ring_locked(ext) {
            return Err(ErrCode::RingNotLocked);
        }
        if self.is_ring_member(ext, address) {
            return Err(ErrCode::RingMemberExists);
        }
        let member_count: i32 = self.connector.get_member_count(ext) + 1;
        let thresh: i32 = threshold_ratio(member_count);
        self.connector.set_active_member(ext, address, true);
        self.connector.set_member_count(ext, member_count);
        self.connector.set_min_thresh(ext, thresh);
        self.emit_add_member(ext, address);
        Ok(())
    }

    pub fn ring_remove_member(
        &self,
        ext: &mut dyn BuiltinExt,
        caller: H256,
        address: H256,
    ) -> Result<(), ErrCode>
    {
        if !self.is_owner(ext, caller) {
            return Err(ErrCode::NotOwner);
        }
        if !self.is_ring_locked(ext) {
            return Err(ErrCode::RingNotLocked);
        }
        if !self.is_ring_member(ext, address) {
            return Err(ErrCode::RingMemberNotExists);
        }
        let member_count: i32 = self.connector.get_member_count(ext) - 1;
        let thresh: i32 = threshold_ratio(member_count);
        self.connector.set_active_member(ext, address, false);
        self.connector.set_member_count(ext, member_count);
        self.connector.set_min_thresh(ext, thresh);
        self.emit_remove_member(ext, address);
        Ok(())
    }

    fn is_with_signature_bounds(&self, ext: &mut dyn BuiltinExt, signature_length: i32) -> bool {
        signature_length >= self.connector.get_min_thresh(ext)
            && signature_length <= self.connector.get_member_count(ext)
    }

    fn bundle_processed(&self, ext: &mut dyn BuiltinExt, hash: H256) -> bool {
        self.connector.get_bundle(ext, hash) != H256::from_slice(&vec![0; WORD_LENGTH])
    }

    /// Assume bundleHash is not from external source, but rather
    /// calculated on our side (on the I/O layer), when BridgeTransfer list
    /// was being created.
    pub fn process_bundles(
        &self,
        ext: &mut dyn BuiltinExt,
        caller: H256,
        transaction_hash: H256,
        source_block_hash: H256,
        transfers: Vec<BridgeTransfer>,
        signatures: Vec<Bytes>,
    ) -> Result<Vec<ExecutionResult>, ErrCode>
    {
        if !self.is_ring_locked(ext) {
            return Err(ErrCode::RingNotLocked);
        }
        if !self.is_relayer(ext, caller) {
            return Err(ErrCode::NotRelayer);
        }
        if !self.is_with_signature_bounds(ext, signatures.len() as i32) {
            return Err(ErrCode::InvalidSignatureBounds);
        }

        // Computes a unique identifier of the transfer hash for each source_block_hash,
        // uniqueness relies on the fact that each

        // verify bundleHash
        let hash: H256 = compute_bundle_hash(source_block_hash, &transfers);

        // ATB 4-1, a transaction submitting a bundle that has already been
        // submitted should not trigger a failure. Instead we should emit
        // an event indicating the transactionHash that the bundle was
        // previously successfully broadcast in.
        if self.bundle_processed(ext, hash) {
            // ATB 6-1, fixed bug: emit stored transactionHash instead of input transaction Hash
            let bundle = self.connector.get_bundle(ext, hash);
            self.emit_successful_transaction_hash(ext, bundle);
            return Ok(Vec::new());
        }

        let mut signed: i32 = 0;
        for sig_bytes in signatures {
            let signature = Ed25519Signature::from(sig_bytes);
            let public = signature.get_public();
            let address = public_to_address_ed25519(&public);

            if verify_signature_ed25519(public, signature, &hash)
                && self.connector.get_active_member(ext, address)
            {
                signed += 1;
            }
        }

        let min_thresh: i32 = self.connector.get_min_thresh(ext);
        if signed < min_thresh {
            return Err(ErrCode::NotEnoughSignatures);
        }

        // otherwise, we're clear to proceed with transfers
        let mut results: Vec<ExecutionResult> = Vec::new();
        for b in transfers {
            if b.get_transfer_value() == 0.to_bigint().unwrap() {
                return Err(ErrCode::InvalidTransfer);
            }

            /*
             * Tricky here, we distinguish between two types of failures here:
             *
             * 1) A balance failure indicates we've failed to load the bridge with
             * enough currency to execute, this means the whole transaction should
             * fail and cause the bridge to exit
             *
             * 2) Any other failure indicates that either the contract had code,
             * which means the contract is now considered null.
             *
             * For how this is documented, check the {@code Transferable}
             * interface documentation.
             */

            let result = match self.transfer(ext, b.get_recipient(), b.get_transfer_value()) {
                Ok(value) => value,
                Err(error) => {
                    return Err(error);
                }
            };

            if !self.emit_distributed(
                ext,
                b.get_src_transaction_hash(),
                b.get_recipient(),
                b.get_transfer_value(),
            ) {
                return Err(ErrCode::InvalidTransfer);
            }
            results.push(result);
        }
        self.connector.set_bundle(ext, hash, transaction_hash);
        self.emit_processed_bundle(ext, source_block_hash, hash);
        Ok(results)
    }

    fn add_log(&self, ext: &mut dyn BuiltinExt, topics: Vec<H256>) { ext.log(topics, None); }

    fn emit_add_member(&self, ext: &mut dyn BuiltinExt, address: H256) {
        let topics: Vec<H256> = vec![BridgeEventSig::AddMember.hash().into(), address];
        self.add_log(ext, topics);
    }

    fn emit_remove_member(&self, ext: &mut dyn BuiltinExt, address: H256) {
        let topics: Vec<H256> = vec![BridgeEventSig::RemoveMember.hash().into(), address];
        self.add_log(ext, topics);
    }

    fn emit_change_owner(&self, ext: &mut dyn BuiltinExt, owner_address: H256) {
        let topics: Vec<H256> = vec![BridgeEventSig::ChangedOwner.hash().into(), owner_address];
        self.add_log(ext, topics);
    }

    fn emit_distributed(
        &self,
        ext: &mut dyn BuiltinExt,
        source_block_hash: H256,
        recipient: H256,
        value: BigInt,
    ) -> bool
    {
        let padded_value: H256 = match pad(value.to_signed_bytes_be(), 32) {
            Some(value) => H256::from_slice(value.as_slice()),
            None => {
                return false;
            }
        };
        let topics: Vec<H256> = vec![
            BridgeEventSig::Distributed.hash().into(),
            source_block_hash,
            recipient,
            padded_value,
        ];
        self.add_log(ext, topics);
        true
    }

    fn emit_processed_bundle(
        &self,
        ext: &mut dyn BuiltinExt,
        source_block_hash: H256,
        bundle_hash: H256,
    )
    {
        let topics: Vec<H256> = vec![
            BridgeEventSig::ProcessedBundle.hash().into(),
            source_block_hash,
            bundle_hash,
        ];
        self.add_log(ext, topics);
    }

    fn emit_successful_transaction_hash(
        &self,
        ext: &mut dyn BuiltinExt,
        aion_transaction_hash: H256,
    )
    {
        let topics: Vec<H256> = vec![
            BridgeEventSig::SuccessfulTxHash.hash().into(),
            aion_transaction_hash,
        ];
        self.add_log(ext, topics);
    }

    pub fn transfer(
        &self,
        ext: &mut dyn BuiltinExt,
        to: H256,
        value: BigInt,
    ) -> Result<ExecutionResult, ErrCode>
    {
        // some initial checks, treat as failure
        let value_u256: U256 = U256::from(value.to_signed_bytes_be().as_slice());
        if ext.balance(&self.contract_address) < value_u256 {
            return Err(ErrCode::InvalidTransfer);
        }
        // assemble an internal transaction
        let from = self.contract_address;

        // increase the nonce and do the transfer without executing code
        ext.inc_nonce(&from);
        ext.transfer_balance(&self.contract_address, &to, &value_u256);

        // construct result
        Ok(ExecutionResult {
            gas_left: U256::zero(),
            status_code: ExecStatus::Success,
            return_data: ReturnData::empty(),
            exception: String::default(),
            state_root: H256::default(),
            invokable_hashes: Default::default(),
        })
    }
}

fn threshold_ratio(input: i32) -> i32 { cmp::max((input * 2) / 3, 1) }
