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

use aion_types::{Address, H256, U256};
use vms::{ExecStatus, ReturnData, FvmExecutionResult as ExecutionResult};
use precompiled::builtin::{BuiltinContract, BuiltinExt, BuiltinParams};
use super::bridge_controller::BridgeController;
use super::bridge_deserializer::{
    parse_address_from_call, parse_address_list, parse_bundle_request, parse_double_word_from_call,
};
use super::bridge_func_sig::BridgeFuncSig;
use super::bridge_strg_conn::BridgeStorageConnector;
use super::bridge_utilities::{
    boolean_to_result_bytes, get_signature, int_to_result_bytes, or_default_d_word,
};

pub struct TokenBridgeContract {
    activate_at: u64,
    name: String,
    connector: BridgeStorageConnector,
    controller: BridgeController,
}

impl TokenBridgeContract {
    /// Constructs a new TokenBridgeContract
    pub fn new(params: BuiltinParams) -> Self {
        let connector = BridgeStorageConnector::new(
            params
                .contract_address
                .clone()
                .expect("Token Bridge Contract needs contract address"),
        );
        // if owner address is not specified in configuration file, assign a default one.
        TokenBridgeContract {
            activate_at: params.activate_at,
            name: params.name,
            connector: connector.clone(),
            controller: BridgeController::new(
                connector.clone(),
                params
                    .contract_address
                    .expect("Token Bridge Contract needs contract address"),
                params
                    .owner_address
                    .expect("Token Bridge Contract needs contract address"),
            ),
        }
    }

    fn is_from_address(&self, ext: &BuiltinExt, address: Address) -> bool {
        // TODO: does address impl equal
        return ext.context().sender == address;
    }

    fn fail(&self, err_msg: String) -> ExecutionResult {
        ExecutionResult {
            gas_left: U256::zero(),
            status_code: ExecStatus::Failure,
            return_data: ReturnData::empty(),
            exception: err_msg,
            state_root: H256::default(),
            invokable_hashes: Default::default(),
        }
    }

    fn succeed(&self, return_data: Vec<u8>) -> ExecutionResult {
        let length: usize = return_data.len();
        ExecutionResult {
            gas_left: U256::zero(),
            status_code: ExecStatus::Success,
            return_data: ReturnData::new(return_data, 0, length),
            exception: String::default(),
            state_root: H256::default(),
            invokable_hashes: Default::default(),
        }
    }
}

impl BuiltinContract for TokenBridgeContract {
    fn is_active(&self, at: u64) -> bool { at >= self.activate_at }

    fn name(&self) -> &str { &self.name }

    fn execute(&self, ext: &mut BuiltinExt, input: &[u8]) -> ExecutionResult {
        // as a preset, try to initialize before execution
        // this should be placed before the 0 into return, rationale is that we want to
        // activate the contract the first time the owner interacts with it. Which is
        // exactly what sending the contract currency entails
        self.controller.initialize(ext);

        // acts as a pseudo fallback function
        if input.len() == 0 {
            return self.succeed(Vec::new());
        }

        let owner: H256 = self.connector.get_owner(ext);
        let sender: H256 = ext.context().sender;
        let relayer: H256 = self.connector.get_relayer(ext);
        let tx_hash: H256 = ext.context().tx_hash;

        if let Some(signature) = get_signature(input.to_vec()) {
            let mut signature_array: [u8; 4] = [0u8; 4];
            signature_array.copy_from_slice(&signature.as_slice()[0..4]);
            if let Some(sig) = BridgeFuncSig::from_hash(&signature_array) {
                match sig {
                    BridgeFuncSig::ChangeOwner => {
                        if !self.is_from_address(ext, owner) {
                            return self.fail(String::from("invalid sender"));
                        }
                        if let Some(address) = parse_address_from_call(input) {
                            match self.controller.set_new_owner(
                                ext,
                                sender,
                                H256::from(address.as_slice()),
                            ) {
                                Ok(_) => {
                                    return self.succeed(Vec::new());
                                }
                                Err(_) => {
                                    return self.fail(String::from("set new owner failed."));
                                }
                            }
                        } else {
                            return self.fail(String::from("parse address from call input failed."));
                        }
                    }
                    BridgeFuncSig::AcceptOwnership => {
                        match self.controller.accept_ownership(ext, sender) {
                            Ok(_) => {
                                return self.succeed(Vec::new());
                            }
                            Err(_) => {
                                return self.fail(String::from("accept ownership failed."));
                            }
                        }
                    }
                    BridgeFuncSig::InitializeRing => {
                        if !self.is_from_address(ext, owner) {
                            return self.fail(String::from("invalid sender"));
                        }

                        if let Some(address_list) = parse_address_list(input) {
                            match self.controller.ring_initialize(
                                ext,
                                sender,
                                address_list
                                    .iter()
                                    .map(|vec| H256::from(vec.as_slice()))
                                    .collect(),
                            ) {
                                Ok(_) => {
                                    return self.succeed(Vec::new());
                                }
                                Err(_) => {
                                    return self.fail(String::from("initialize ring failed."));
                                }
                            }
                        } else {
                            return self.fail(String::from("parse address list failed."));
                        }
                    }
                    BridgeFuncSig::AddRingMember => {
                        if !self.is_from_address(ext, owner) {
                            return self.fail(String::from("invalid sender"));
                        }

                        if let Some(address) = parse_address_from_call(input) {
                            match self.controller.ring_add_member(
                                ext,
                                sender,
                                H256::from(address.as_slice()),
                            ) {
                                Ok(_) => {
                                    return self.succeed(Vec::new());
                                }
                                Err(_) => {
                                    return self.fail(String::from("ring add member failed."));
                                }
                            }
                        } else {
                            return self
                                .fail(String::from("failed to parse address from call input."));
                        }
                    }
                    BridgeFuncSig::RemoveRingMember => {
                        if !self.is_from_address(ext, owner) {
                            return self.fail(String::from("invalid sender."));
                        }

                        if let Some(address) = parse_address_from_call(input) {
                            match self.controller.ring_remove_member(
                                ext,
                                sender,
                                H256::from(address.as_slice()),
                            ) {
                                Ok(_) => {
                                    return self.succeed(Vec::new());
                                }
                                Err(_) => {
                                    return self.fail(String::from("remove ring member failed."));
                                }
                            }
                        } else {
                            return self.fail(String::from(
                                "failed to parse address from calfailed to parse address from \
                                 call.",
                            ));
                        }
                    }
                    BridgeFuncSig::SetRelayer => {
                        if !self.is_from_address(ext, owner) {
                            return self.fail(String::from("invalid sender"));
                        }

                        if let Some(address) = parse_address_from_call(input) {
                            match self.controller.set_relayer(
                                ext,
                                sender,
                                H256::from(address.as_slice()),
                            ) {
                                Ok(_) => {
                                    return self.succeed(Vec::new());
                                }
                                Err(_) => {
                                    return self.fail(String::from("set relayer failed."));
                                }
                            }
                        } else {
                            return self.fail(String::from("parse address from call failed."));
                        }
                    }
                    BridgeFuncSig::SubmitBundle => {
                        if !self.is_from_address(ext, relayer) {
                            return self.fail(String::from("invalid relayer"));
                        }

                        if let Some(bundle_requests) = parse_bundle_request(input) {
                            // ATB-4, as part of the changes we now
                            // pass in the transactionHash of the call
                            // into the contract, this will be logged so that
                            // we can refer to it at a later time.
                            match self.controller.process_bundles(
                                ext,
                                sender,
                                tx_hash,
                                H256::from(bundle_requests.block_hash.as_slice()),
                                bundle_requests.bundles,
                                bundle_requests.signatures,
                            ) {
                                Ok(_) => {
                                    return self.succeed(Vec::new());
                                }
                                Err(_) => {
                                    return self.fail(String::from("process bundle failed."));
                                }
                            }
                        } else {
                            return self.fail(String::from("parse bundle request failed."));
                        }
                    }
                    BridgeFuncSig::Owner => {
                        return self.succeed(or_default_d_word(Some(
                            self.connector.get_owner(ext).as_ref().to_vec(),
                        )));
                    }
                    BridgeFuncSig::NewOwner => {
                        return self.succeed(or_default_d_word(Some(
                            self.connector.get_new_owner(ext).as_ref().to_vec(),
                        )));
                    }
                    BridgeFuncSig::RingLocked => {
                        return self.succeed(or_default_d_word(Some(boolean_to_result_bytes(
                            self.connector.get_ring_locked(ext),
                        ))));
                    }
                    BridgeFuncSig::MinThresh => {
                        return self.succeed(or_default_d_word(int_to_result_bytes(
                            self.connector.get_min_thresh(ext),
                        )));
                    }
                    BridgeFuncSig::MemberCount => {
                        return self.succeed(or_default_d_word(int_to_result_bytes(
                            self.connector.get_member_count(ext),
                        )));
                    }
                    BridgeFuncSig::RingMap => {
                        if let Some(address2) = parse_address_from_call(input) {
                            return self.succeed(or_default_d_word(Some(boolean_to_result_bytes(
                                self.connector
                                    .get_active_member(ext, H256::from(address2.as_slice())),
                            ))));
                        } else {
                            return self.fail(String::from("address is empty"));
                        }
                    }
                    BridgeFuncSig::ActionMap => {
                        if let Some(bundle_hash) = parse_double_word_from_call(input) {
                            return self.succeed(or_default_d_word(Some(
                                self.connector
                                    .get_bundle(ext, H256::from(bundle_hash.as_slice()))
                                    .as_ref()
                                    .to_vec(),
                            )));
                        } else {
                            return self.fail(String::from("bundle_hash is empty"));
                        }
                    }
                    BridgeFuncSig::Relayer => {
                        return self.succeed(or_default_d_word(Some(
                            self.connector.get_relayer(ext).as_ref().to_vec(),
                        )));
                    }
                }
            } else {
                return self.fail(String::from("signature is invalid"));
            }
        } else {
            return self.fail(String::from("signature is none."));
        }
    }

    fn cost(&self, _input: &[u8]) -> U256 { U256::from(21000) }
}

#[cfg(test)]
mod test {
    use blake2b::{Blake2b, blake2b};
    use aion_types::{Address, H256, U256};
    use vms::ExecStatus;
    use executor::fvm_exec::contract_address;
    use fastvm::basetypes::DataWord;
    use key::{Ed25519KeyPair, generate_keypair, public_to_address_ed25519};
    use num_bigint::{BigInt, Sign, ToBigInt};
    use precompiled::atb::{bridge_event_sig, bridge_func_sig, bridge_transfer, bridge_utilities};
    use precompiled::atb::bridge_event_sig::BridgeEventSig;
    use precompiled::atb::bridge_transfer::{get_instance, BridgeTransfer};
    use precompiled::atb::bridge_utilities::compute_bundle_hash;
    use precompiled::builtin::{
        BuiltinContext, BuiltinContract, BuiltinExt, BuiltinExtImpl, BuiltinParams,
};
    use rustc_hex::FromHex;
    use state::{State, Substate};
    use super::{*};
    use helpers::get_temp_state;

    lazy_static! {
        static ref OWNER_ADDRESS: Address =
            Address::from(Blake2b::hash_256("ownerAddress".as_bytes()));
        static ref CONTRACT_ADDRESS: Address =
            Address::from(Blake2b::hash_256("contractAddress".as_bytes()));
        static ref TX_HASH: H256 = H256::from(Blake2b::hash_256("transaction".as_bytes()));
        static ref MEMBERS: Vec<Ed25519KeyPair> = {
            let mut members = Vec::with_capacity(5);
            members.push(Ed25519KeyPair::from("a7dbfae4604fdd9bef082dcc5ae92952be5cd1b96e1d2a0fb1bbe6b9cfcb137df0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb91f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb91".from_hex().unwrap()));
            members.push(Ed25519KeyPair::from("4d4054a82269f1490ebe140164f01f286b9e9e0d67b60abaea2e99d3787df9351c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cb1c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cb".from_hex().unwrap()));
            members.push(Ed25519KeyPair::from("7371dd7329f819cba415c6d7a08346b8a508723c8169d959bb7dd97f830c2597f80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3cc".from_hex().unwrap()));
            members.push(Ed25519KeyPair::from("f56b845f97af1cd3281dc85a7e5c75405604e8746d05d0c2a2bf4d2bf43d1f82f2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5f2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5".from_hex().unwrap()));
            members.push(Ed25519KeyPair::from("20d81feb94a07805941f607be1bcbfe908c0dab8002e1bcc7156646d853784c4e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57fe08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f".from_hex().unwrap()));
            members
        };
    }

    pub struct ReturnDataFromSetup {
        submit_bundle_context: BuiltinContext,
        payload_hash: H256,
    }

    fn builtin_params() -> BuiltinParams {
        BuiltinParams {
            activate_at: 0,
            deactivate_at: None,
            name: String::from("atb"),
            owner_address: Some(*OWNER_ADDRESS),
            contract_address: Some(*CONTRACT_ADDRESS),
        }
    }

    fn get_contract() -> TokenBridgeContract { TokenBridgeContract::new(builtin_params()) }

    fn get_ext<'a>(
        state: &'a mut State<::db::StateDB>,
        substate: &'a mut Substate,
    ) -> BuiltinExtImpl<'a, ::db::StateDB>
    {
        let ext = BuiltinExtImpl::new(
            state,
            BuiltinContext {
                sender: "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .unwrap(),
                address: "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .unwrap(),
                tx_hash: *TX_HASH,
                origin_tx_hash: H256::default(),
            },
            substate,
        );
        ext
    }

    fn setup_for_test(
        ext: &mut BuiltinExt,
        transfers: &Vec<BridgeTransfer>,
    ) -> ReturnDataFromSetup
    {
        let contract = get_contract();

        let payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9"
            .from_hex()
            .unwrap();
        let result = contract.execute(ext, &payload);
        assert!(result.status_code == ExecStatus::Success);

        let call_payload: Vec<u8> =
            "6548e9bca092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
                .from_hex()
                .unwrap();
        let transfer_result = contract.execute(ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);

        let _ = ext.add_balance(&*CONTRACT_ADDRESS, &U256::from(10));

        // we create a new token bridge contract here because we
        // need to change the execution context
        let submit_bundle_context = BuiltinContext {
            //sender: Address::from(Blake2b::hash_256(&*MEMBERS[0].get_address())),
            sender: Address::from(Blake2b::hash_256(&*MEMBERS[0].address())),
            address: *CONTRACT_ADDRESS,
            tx_hash: *TX_HASH,
            origin_tx_hash: H256::default(),
        };

        let block_hash = H256::from(Blake2b::hash_256("ownerAddress".as_bytes()));
        let payload_hash = compute_bundle_hash(block_hash, &transfers);

        ReturnDataFromSetup {
            submit_bundle_context,
            payload_hash,
        }
    }

    fn get_ext_zero_sender<'a>(
        state: &'a mut State<::db::StateDB>,
        substate: &'a mut Substate,
    ) -> BuiltinExtImpl<'a, ::db::StateDB>
    {
        let ext = BuiltinExtImpl::new(
            state,
            BuiltinContext {
                sender: Address::zero(),
                address: *CONTRACT_ADDRESS,
                tx_hash: *TX_HASH,
                origin_tx_hash: H256::default(),
            },
            substate,
        );
        ext
    }

    fn get_ext_default<'a>(
        state: &'a mut State<::db::StateDB>,
        substate: &'a mut Substate,
    ) -> BuiltinExtImpl<'a, ::db::StateDB>
    {
        let ext = BuiltinExtImpl::new(
            state,
            BuiltinContext {
                sender: *OWNER_ADDRESS,
                address: *CONTRACT_ADDRESS,
                tx_hash: *TX_HASH,
                origin_tx_hash: H256::default(),
            },
            substate,
        );
        ext
    }

    #[test]
    fn atb_test_get_owner() {
        let contract = get_contract();
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext(state, substate);
        let input = &BridgeFuncSig::Owner.hash()[0..4];

        assert!(!contract.connector.get_initialized(&mut ext));
        let result = contract.execute(&mut ext, &input);
        assert_eq!(Address::from(&*result.return_data), *OWNER_ADDRESS);
        assert!(contract.connector.get_initialized(&mut ext));
    }

    #[test]
    fn atb_get_new_owner() {
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext_default(state, substate);
        let contract = get_contract();

        let new_owner =
            public_to_address_ed25519(&H256::from_slice(&Blake2b::hash_256("newOwner".as_bytes())));
        let payload = "a6f9dae1a0a2789429eec5a4846c50bfbca9d6b54cc4308f8f5a7f4c12737ecb3f2e1f76"
            .from_hex()
            .unwrap();

        assert!(!contract.connector.get_initialized(&mut ext));

        let result = contract.execute(&mut ext, &payload);
        assert!(contract.connector.get_initialized(&mut ext));
        assert!(result.status_code == ExecStatus::Success);

        let query_result = contract.execute(&mut ext, &BridgeFuncSig::NewOwner.hash());
        assert!(query_result.status_code == ExecStatus::Success);
        assert_eq!(new_owner, H256::from(&*query_result.return_data));
    }

    #[test]
    fn atb_test_get_new_owner_not_owner_address() {
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext_zero_sender(state, substate);
        let contract = get_contract();

        let _new_owner =
            public_to_address_ed25519(&H256::from_slice(&Blake2b::hash_256("newOwner".as_bytes())));
        let payload = "a6f9dae1a0a2789429eec5a4846c50bfbca9d6b54cc4308f8f5a7f4c12737ecb3f2e1f76"
            .from_hex()
            .unwrap();

        assert!(!contract.connector.get_initialized(&mut ext));

        let result = contract.execute(&mut ext, &payload);
        assert!(contract.connector.get_initialized(&mut ext));
        assert!(result.status_code == ExecStatus::Failure);
    }

    #[test]
    fn atb_test_initialize_ring() {
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext_default(state, substate);
        let contract = get_contract();

        let payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();

        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Success);

        for x in &*MEMBERS {
            assert!(contract.connector.get_active_member(&mut ext, x.address()));
        }
    }

    #[test]
    fn atb_test_initialize_ring_not_owner() {
        let state = &mut get_temp_state();
        let substate = &mut Substate::new();
        let mut ext = get_ext_zero_sender(state, substate);
        let contract = get_contract();

        let payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Failure);
    }

    #[test]
    fn atb_test_transfer() {
        let mut state = get_temp_state();
        let mut substate = Substate::new();
        let contract = get_contract();
        {
            let mut ext = get_ext_default(&mut state, &mut substate);

            let ring_initialize_payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
            let result = contract.execute(&mut ext, &ring_initialize_payload);
            assert!(result.status_code == ExecStatus::Success);

            // set relayer
            let call_payload =
                "6548e9bca092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
                    .from_hex()
                    .unwrap();
            let call_result = contract.execute(&mut ext, &call_payload);
            assert!(call_result.status_code == ExecStatus::Success);

            let _add_result = ext.add_balance(&*CONTRACT_ADDRESS, &U256::from(10));
        }

        let mut submit_bundle_ext = BuiltinExtImpl::new(
            &mut state,
            BuiltinContext {
                sender: MEMBERS[0].address(),
                address: *CONTRACT_ADDRESS,
                tx_hash: *TX_HASH,
                origin_tx_hash: H256::default(),
            },
            &mut substate,
        );
        // assemble the payload
        let block_hash = H256(Blake2b::hash_256("blockHash".as_bytes()));
        let mut transfers = Vec::with_capacity(10);
        for i in 0..10 {
            // generate a unique sourceTransactionHash for each transfer
            let source_transaction_hash =
                H256(Blake2b::hash_256(format!("{}", i).into_bytes().as_slice()));
            let recipient = public_to_address_ed25519(&H256::from_slice(&Blake2b::hash_256(
                format!("{:x}", i).into_bytes().as_slice(),
            )));
            let transfer = get_instance(BigInt::from(1), recipient, source_transaction_hash);
            transfers.push(transfer.unwrap());
        }

        let payload_hash = compute_bundle_hash(block_hash, &transfers);
        // ATB-4, do one assert here to check that transactionHash is not set
        let mut input_data = Vec::new();
        input_data.extend_from_slice(&BridgeFuncSig::ActionMap.hash());
        input_data.extend_from_slice(&<[u8; 32]>::from(U256::from(payload_hash)));
        let bundle_result = contract.execute(&mut submit_bundle_ext, &input_data);
        assert_eq!(*bundle_result.return_data, [0u8; 32]);

        //        let signatures:[u8; 96] = Vec::with_capacity((*MEMBERS).len());
        //        for k in &*MEMBERS {
        //            signatures.push(sign_ed25519(k, &payload_hash).unwrap().into());
        //        }

        let submit_bundle_payload = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef87500000000000000000000000000000080000000000000000000000000000001d000000000000000000000000000000320000000000000000000000000000003d000000000000000000000000000000480000000000000000000000000000005300000000000000000000000000000000a0fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d67bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e90a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a20000000000000000000000000000000aa08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205eda0d12a94f45cf9e0524069746acd51ab53a3190ea520bbcaf58329ab5c8626b4a0d8ef096f4f07a57163bdd28728f2a5d4689cd8c7089618223f567e30a95e33a017063ae510ba37ff55f5fa533c6d953a33f6a5252feaa35d45b260dda8c5f2a0614182bec524d648f7dba8a686037f0f535f52f6d3a3b031ef6fc78bc4c641a0b90da4145ddddbe57b2839bf4ca2ab966a5c1e66db2eb977f26c90621bc1820000000000000000000000000000000a0000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cbf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f0000000000000000000000000000000575312f45d7791c702bfe38f056287fb8b64514cbc77f76f17b421ec3451a25eda66e2fdb4b7a13dd48c9953c12bc8ea7f2f09f1e0dd1c6a01e5b1557964826f6760f529c41c666bb07c0b492ae9700c8f0a46783006ca140905413c5f14703fe1831b6f93aa1efddcb49408edaabbebf688b5580b5f58f41c19a3e0ffe39aec40383882b5f0b02053706cbb8265991d1ce70c140e6480422d8e7a70cb7fff54700000000000000000000000000000005b57dba44d5107751d16daf30a9e186730197e4d12fbeeff57d335c80e7f91e04c386d3fa9f0336a3900a11be4788f71d58a9782525c11fd4e10c4dccdf03340ae50ed9967d016e2ee4b2fade9baaa818291a0047da50aa3dd54e71bba48f20056aaaaa8e915d3cea4c9d84721489d5c568468da5f80ac3e0434b55cef0d23c001a4439c6c23fbff79c27dd7d7edbaae0743b469a48a2ee4a65374606a5d6c203".from_hex().unwrap();
        let transfer_result = contract.execute(&mut submit_bundle_ext, &submit_bundle_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);

        // VERIFICATION

        // ATB-4, assert that transactionHash is now properly set
        // ATB-4, do one assert here to check that transactionHash is not set
        let submit_result = contract.execute(&mut submit_bundle_ext, &input_data);
        assert_eq!(*submit_result.return_data, (*TX_HASH).0);

        for b in &transfers {
            assert_eq!(submit_bundle_ext.balance(&b.get_recipient()), U256::from(1));
        }
        assert_eq!(submit_bundle_ext.balance(&*CONTRACT_ADDRESS), U256::from(0));
        // context verification
        // we expect on successful output:
        // 10 internal transactions (that all succeed)
        // 10 Distributed events
        // 1 ProcessedBundle Event

        let mut i = 0i32;
        let logs = submit_bundle_ext.get_logs();
        assert_eq!(logs.len(), 11);
        for l in &logs {
            // verify address is correct
            assert_eq!(l.address, *CONTRACT_ADDRESS);

            // on the 11th log, it should be the processed bundle event
            if i == 10 {
                assert_eq!(l.topics[0], H256(BridgeEventSig::ProcessedBundle.hash()));
                assert_eq!(l.topics[1], block_hash);
                assert_eq!(l.topics[2], payload_hash);
                continue;
            }

            // otherwise we expectg a Distributed event
            assert_eq!(l.topics[0], H256(BridgeEventSig::Distributed.hash()));
            assert_eq!(
                l.topics[1],
                transfers[i as usize].get_src_transaction_hash()
            );
            assert_eq!(l.topics[2], transfers[i as usize].get_recipient());
            assert_eq!(
                l.topics[3].low_u64().to_bigint().unwrap(),
                transfers[i as usize].get_transfer_value()
            );
            i = i + 1;
        }
    }

    #[test]
    fn atb_test_non_a0_address_transfer() {
        let mut state = get_temp_state();
        let mut substate = Substate::new();
        let contract = get_contract();
        {
            let mut ext = get_ext_default(&mut state, &mut substate);

            let ring_initialize_payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
            let result = contract.execute(&mut ext, &ring_initialize_payload);
            assert!(result.status_code == ExecStatus::Success);

            // set relayer
            let call_payload =
                "6548e9bca092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
                    .from_hex()
                    .unwrap();
            let call_result = contract.execute(&mut ext, &call_payload);
            assert!(call_result.status_code == ExecStatus::Success);

            let _add_result = ext.add_balance(&*CONTRACT_ADDRESS, &U256::from(10));
        }
        // we create a new token bridge contract here because we
        // need to change the execution context
        let mut submit_bundle_ext = BuiltinExtImpl::new(
            &mut state,
            BuiltinContext {
                sender: MEMBERS[0].address(),
                address: *CONTRACT_ADDRESS,
                tx_hash: *TX_HASH,
                origin_tx_hash: H256::default(),
            },
            &mut substate,
        );
        // assemble the payload
        let block_hash = H256(Blake2b::hash_256("blockHash".as_bytes()));
        let mut transfers = Vec::with_capacity(10);
        for i in 0..10 {
            // generate a unique sourceTransactionHash for each transfer
            let source_transaction_hash =
                H256(Blake2b::hash_256(format!("{}", i).into_bytes().as_slice()));
            let recipient = H256(Blake2b::hash_256(
                format!("{:x}", i).into_bytes().as_slice(),
            ));
            let transfer = get_instance(BigInt::from(1), recipient, source_transaction_hash);
            transfers.push(transfer.unwrap());
        }

        let payload_hash = compute_bundle_hash(block_hash, &transfers);
        // ATB-4, do one assert here to check that transactionHash is not set
        let mut input_data = Vec::new();
        input_data.extend_from_slice(&BridgeFuncSig::ActionMap.hash());
        input_data.extend_from_slice(&<[u8; 32]>::from(U256::from(payload_hash)));
        let bundle_result = contract.execute(&mut submit_bundle_ext, &input_data);
        assert_eq!(*bundle_result.return_data, [0u8; 32]);

        let submit_bundle_payload = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef87500000000000000000000000000000080000000000000000000000000000001d000000000000000000000000000000320000000000000000000000000000003d000000000000000000000000000000480000000000000000000000000000005300000000000000000000000000000000a0fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d67bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e90a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a20000000000000000000000000000000a0fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d67bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e90a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a20000000000000000000000000000000a0000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cbf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f00000000000000000000000000000005ad584f372d53de182bf4ae386aadaf4ea062f4723ddf2682aab341fd819a9198f1a157d5b94431e9edf76eb9d94e2e3b522e5c9150cadaf74e47bb330de57d139d7c20009d03248fc36a30da2ce5af2160ac7cfd9169e329ab9549a5c839dd4393f990938fd25259f89205b386779f604df11973aba078fe298abddeef3538cedfb496cce3d5f72b1b2df8f983a182da08896714b2c432d5c3399b2ea83ffb1f00000000000000000000000000000005e949b381b7455a757a5c1148bdf8a91501549ac9a70e5165f57cc666c0ed1d0c2fa0db8ac66ddab8dd526170ef3f1d0de5fb3851895707ffe85af95c09dc8b0f93ed20c7079dc48d1502b4f136682e3079f034f19d37a4bf4cb6ecaef93f640318439830c36ef0ebf50cd849d1241df06e597bdbe7ec7f5e301df042b1661706e64c6854618d466300aaac89c51de2d375037b50043d7918171db9a37dae420a".from_hex().unwrap();
        let transfer_result = contract.execute(&mut submit_bundle_ext, &submit_bundle_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);

        // VERIFICATION

        // ATB-4 assert that transactionHash is now properly set
        // ATB-4, do one assert here to check that transactionHash is not set
        let submit_result = contract.execute(&mut submit_bundle_ext, &input_data);
        assert_eq!(*submit_result.return_data, (*TX_HASH).0);

        for b in &transfers {
            assert_eq!(submit_bundle_ext.balance(&b.get_recipient()), U256::from(1));
        }
        assert_eq!(submit_bundle_ext.balance(&*CONTRACT_ADDRESS), U256::from(0));

        // context verification
        // we expect on successful output:
        // 10 internal transactions (that all succeed)
        // 10 Distributed events
        // 1 ProcessedBundle Event

        let mut i = 0i32;
        let logs = submit_bundle_ext.get_logs();
        assert_eq!(logs.len(), 11);
        for l in &logs {
            // verify address is correct
            assert_eq!(l.address, *CONTRACT_ADDRESS);

            // on the 11th log, it should be the processed bundle event
            if i == 10 {
                assert_eq!(l.topics[0], H256(BridgeEventSig::ProcessedBundle.hash()));
                assert_eq!(l.topics[1], block_hash);
                assert_eq!(l.topics[2], payload_hash);
                continue;
            }

            // otherwise we expectg a Distributed event
            assert_eq!(l.topics[0], H256(BridgeEventSig::Distributed.hash()));
            assert_eq!(
                l.topics[1],
                transfers[i as usize].get_src_transaction_hash()
            );
            assert_eq!(l.topics[2], transfers[i as usize].get_recipient());
            assert_eq!(
                l.topics[3].low_u64().to_bigint().unwrap(),
                transfers[i as usize].get_transfer_value()
            );
            i = i + 1;
        }
    }

    #[test]
    fn atb_test_transfer_not_replayer() {
        let mut state = get_temp_state();
        let mut substate = Substate::new();
        let contract = get_contract();
        {
            let mut ext = get_ext_default(&mut state, &mut substate);

            let ring_initialize_payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
            let result = contract.execute(&mut ext, &ring_initialize_payload);
            assert!(result.status_code == ExecStatus::Success);

            // set relayer
            let call_payload =
                "6548e9bca092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
                    .from_hex()
                    .unwrap();
            let call_result = contract.execute(&mut ext, &call_payload);
            assert!(call_result.status_code == ExecStatus::Success);

            let _add_result = ext.add_balance(&*CONTRACT_ADDRESS, &U256::from(10));
        }
        {
            // we create a new token bridge contract here because we
            // need to change the execution context
            let mut submit_bundle_ext = BuiltinExtImpl::new(
                &mut state,
                BuiltinContext {
                    sender: MEMBERS[0].address(),
                    address: *CONTRACT_ADDRESS,
                    tx_hash: *TX_HASH,
                    origin_tx_hash: H256::default(),
                },
                &mut substate,
            );
            // assemble the payload
            let block_hash = H256(Blake2b::hash_256("blockHash".as_bytes()));
            let mut transfers = Vec::with_capacity(10);
            for i in 0..10 {
                // generate a unique sourceTransactionHash for each transfer
                let source_transaction_hash =
                    H256(Blake2b::hash_256(format!("{}", i).into_bytes().as_slice()));
                let recipient = public_to_address_ed25519(&H256::from_slice(&Blake2b::hash_256(
                    format!("{:x}", i).into_bytes().as_slice(),
                )));
                let transfer = get_instance(BigInt::from(1), recipient, source_transaction_hash);
                transfers.push(transfer.unwrap());
            }

            let payload_hash = compute_bundle_hash(block_hash, &transfers);
            // ATB-4, do one assert here to check that transactionHash is not set
            let mut input_data = Vec::new();
            input_data.extend_from_slice(&BridgeFuncSig::ActionMap.hash());
            input_data.extend_from_slice(&<[u8; 32]>::from(U256::from(payload_hash)));
            let bundle_result = contract.execute(&mut submit_bundle_ext, &input_data);

            assert_eq!(*bundle_result.return_data, [0u8; 32]);
        }

        let mut incorrect_relay_submit_bundle_ext = BuiltinExtImpl::new(
            &mut state,
            BuiltinContext {
                sender: Address::zero(),
                address: *CONTRACT_ADDRESS,
                tx_hash: *TX_HASH,
                origin_tx_hash: H256::default(),
            },
            &mut substate,
        );

        let submit_bundle_payload = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef87500000000000000000000000000000080000000000000000000000000000001d000000000000000000000000000000320000000000000000000000000000003d000000000000000000000000000000480000000000000000000000000000005300000000000000000000000000000000a0fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d67bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e90a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a20000000000000000000000000000000aa08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205eda0d12a94f45cf9e0524069746acd51ab53a3190ea520bbcaf58329ab5c8626b4a0d8ef096f4f07a57163bdd28728f2a5d4689cd8c7089618223f567e30a95e33a017063ae510ba37ff55f5fa533c6d953a33f6a5252feaa35d45b260dda8c5f2a0614182bec524d648f7dba8a686037f0f535f52f6d3a3b031ef6fc78bc4c641a0b90da4145ddddbe57b2839bf4ca2ab966a5c1e66db2eb977f26c90621bc1820000000000000000000000000000000a0000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cbf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f0000000000000000000000000000000575312f45d7791c702bfe38f056287fb8b64514cbc77f76f17b421ec3451a25eda66e2fdb4b7a13dd48c9953c12bc8ea7f2f09f1e0dd1c6a01e5b1557964826f6760f529c41c666bb07c0b492ae9700c8f0a46783006ca140905413c5f14703fe1831b6f93aa1efddcb49408edaabbebf688b5580b5f58f41c19a3e0ffe39aec40383882b5f0b02053706cbb8265991d1ce70c140e6480422d8e7a70cb7fff54700000000000000000000000000000005b57dba44d5107751d16daf30a9e186730197e4d12fbeeff57d335c80e7f91e04c386d3fa9f0336a3900a11be4788f71d58a9782525c11fd4e10c4dccdf03340ae50ed9967d016e2ee4b2fade9baaa818291a0047da50aa3dd54e71bba48f20056aaaaa8e915d3cea4c9d84721489d5c568468da5f80ac3e0434b55cef0d23c001a4439c6c23fbff79c27dd7d7edbaae0743b469a48a2ee4a65374606a5d6c203".from_hex().unwrap();
        let transfer_result = contract.execute(
            &mut incorrect_relay_submit_bundle_ext,
            &submit_bundle_payload,
        );
        assert!(transfer_result.status_code == ExecStatus::Failure);
    }

    #[test]
    fn atb_test_transfers_greater_than_max_list_size() {
        let mut state = get_temp_state();
        let mut substate = Substate::new();
        let contract = get_contract();
        {
            let mut ext = get_ext_default(&mut state, &mut substate);

            let ring_initialize_payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
            let result = contract.execute(&mut ext, &ring_initialize_payload);
            assert!(result.status_code == ExecStatus::Success);

            // set relayer
            let call_payload =
                "6548e9bca092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
                    .from_hex()
                    .unwrap();
            let call_result = contract.execute(&mut ext, &call_payload);
            assert!(call_result.status_code == ExecStatus::Success);

            let _add_result = ext.add_balance(&*CONTRACT_ADDRESS, &U256::from(1024));
        }

        let mut submit_bundle_ext = BuiltinExtImpl::new(
            &mut state,
            BuiltinContext {
                sender: MEMBERS[0].address(),
                address: *CONTRACT_ADDRESS,
                tx_hash: *TX_HASH,
                origin_tx_hash: H256::default(),
            },
            &mut substate,
        );
        let mut transfers = Vec::with_capacity(1024);
        for i in 0..1024 {
            // generate a unique sourceTransactionHash for each transfer
            let source_transaction_hash =
                H256(Blake2b::hash_256(format!("{}", i).into_bytes().as_slice()));
            let recipient = public_to_address_ed25519(&H256::from_slice(&Blake2b::hash_256(
                format!("{:x}", i).into_bytes().as_slice(),
            )));
            let transfer = get_instance(BigInt::from(1), recipient, source_transaction_hash);
            transfers.push(transfer.unwrap());
        }
        let submit_bundle_payload = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef8750000000000000000000000000000008000000000000000000000000000008090000000000000000000000000000100a0000000000000000000000000000140b00000000000000000000000000001416000000000000000000000000000014210000000000000000000000000000004000fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d67bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e90a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a2d0b1be7d92bf8830457c084ff4da1c2841879b483c3cface85dcd40f69e1ab8e1f8c0bfd983fbc0c61f48b51ee0d8a95148dd96756fcd00ec76260f6efa8030fc83176b698c10e17d80324d6ea14f2bc47fd2b0d247aa1c10815bb31bf1f44953c504a2e2be2c29b5b5f35f6a52f4f7c8733a7f26387484d8ef2d3ff92c53fea33465cfcceb111b0c4fc1bd728fbf5d478dd8acc8cffbbb6ff1aa123e3cfcead439be98bf9021ac2c94c2e5a0aacd02cf5fc252f9a69b1d32fc32c8293474eadf93916fa5d2fb992814e85a225c21fc518c1fc375bcc6415bf127158eeb4fc32e7d20c296e273f8446ea555069d310daebc3a84bf070bad7f184ed8199a6ce23d8d1c9f15f619c5784cc310914e0867500299fac0197e2ee16ce029de21555d61bf3dd7ca6cdfe766f689ed771d8738c3aeb4a0367e9feb7b6b65c142a4e31fcda2da4e3366bca0044c0340f33a10067c0be5235a081f4a87b4f3958bb2aea77d7f629d9cf2c7ce0404ac62c874a9b1eafe7e31f3747cbcc8b6f56481dc74aed823eb4e62aa79fe41b30978e8aa72109ef073083b204f0bab92dbb87ffd76da8a9d541fee5fbbf187e450b509d3af66d232ff80e2d6a6890ae2ea1ff5ebc49932bd17e0a4badad68cb294e2c28109c538c32dfd6039ceca39a7aea65cd5848e1b9d5b21f4151d7f8c94400096ce1cb0dabdf43a3c516d2986c869c7754fa41eb660076bd0c4fc7b8db14f39336f022447a088de7eef69b31151d00dd787a6b57482dfd612aaa69d3cf2bf1cca259f89911ff384d47f0176e91c8df10e68f43622ebbab224687a72ac7bb2fb78d2754c0189cf62d796854a43b140072464f522172d96fb4ef9e2d9ebd0be30d7dcc622c9dc60b7bd711a47ed775a32eacea4b23367bb5348d2ec2b14137062ed579afc7b16e70a3c64e58310d93bbf8a17fcc8cb682df8c87f218dfa8151f2eaafe20e6f8d87b243f8fe63de6d49485a8bf6eeae75ffb48c4af5f14fdc78faafce1e8c20034ceeff0d913cbc2351510bbd260ddef97c9e36f5e33370c795599501e8f834363461eb001fd227831992c57b5bedf22beb2e0fe05edf9f16c9ca40f7ee1e4eee5bf4cfb667b8664f3b206b4728de63dedf9dc6ed635143ad253c1031c3d5829abb51779843dcfb7d87c129da487ff6f56373416364d05337c982f06c593f4216b286abcbb5db7d67e811c21214a64734e6548dd0d402596f655a38c3e8347dc2b3be1bb7708d1f33af2a4eef3d16d72556f9e87139a328460af11f246f2c24fc72d7517d1b8f4569d3ee5ed18198abf89026e3ed49b4a25a4047c237e996126c7d0aada79ddb85554c485bc51d1973a6088ad1fd331d91f4d7db6e05ca2aed53102c69fa0b6e16b9123ac2cf2ec980e53700affd2b55c896323e89a3cdb918b2fe8d8228cca361cefcf07c84c9e322f0039e93a27221fcf657fb877a1d4f60307106113e885096cb44a461cd0afbf947df21e471af47ec8d88b09c748a07c8abbbcc2ebc27d8fabc94efc815e3cc14548c382ab61f3cd2d77376e20c553f54d4c3df9e0ac58764da4d97b47e236983fcc15583e48c508deba792d94963df1c2a9f659a698e723421ae47e62d2a3add38875541eabe0a5170a3eb7603750958c20d2e5bfe446e63cd098056baeededbcca47b23ff7fa44c2fe165707e626b7f077ea55394155be91275d12378bdf1185c4fe345fd1f8e05ecb7e3e54091ca67838d60c67f827c69ce0439f55fbb16942e4d2c9a18b28a3111e9aee34cc0bd669f32c5b49a28f48f0633b61aeeff6b2405493c50b9388c7dcbd0d6c43d6f538d86db7d8cef951e32c5357e914864b624b52adbfa0d71c2fdefcdaffd470ca5fb57492e69c7979329e425a3f04a10f7cfa490bd3f7e6d06bc7c8c41a8f8d6818e69033ef764c2c8bb78b44ff4f774887a281a64dee23fb2b9e7809df4eadfadb7691203557a5f92a3ee4e3a91bac2fc9f8e92a1d85d9c9cbe4b894e3bdd2a348fc0f2e6e7e4fe41439b8ed51d1d31581c542ff9479bcc007062ea8a45a5aacacfc4eb4492175b8471428624c81ae24c3a4557e3ba0f079a908a3042ac02a1c1f48f073a836aec0cc19446dfd7515236e88f98df4267da46c1550eefd7e7c441afba6857cb0eda294139e55cb6d80116d662af92dc04c43d2327691c0564d0bca7028a4630c54159f096f08264f0d2c67bdd1d537496ba3f677578023d57b375323639a92c9a44ccf4075e6f498d6e9cbc1a430e4b00e4409c99dac3a14443bb49871d2b55a67f58d591e990dd8b8a93b8f4bfc57a9e08e72a4ca54eed3610aea2205b323b70b81d1302b6842367a47b20b5738be37c42e8436c32f23918b676ba1a71da33d77b37b547c004538131cd766bf3c8a74ab93fbae90ad75f8a6654cab3954a3c2c774e5146b67e613828ffb3e815c210931e08be6343057595247c749cd33c02c12798d8b9d2b465ce658d852bb531b167a7ee8f6a9ffe1d103f7fb730cc6bef26290a8bbf11a63c725b209e6071f38660bb5836da9a59c68e66fd46c7d4110a533ebb30fa0e41c979720b879ae01b855ac7a13843295224cfe3dde3ea13ef18e8aa91852068ebd4b2f5cd95fc8e1ccb1957d1d74e9f1dbce16a12c451b96bd44adb5517e84676b91c1831f51dbead99729e86281c0512f45dddb49bf76c78c0f9356568e36a4aca85e52aef541337b56105507355e4a53f98246205c598ae5fc7d2d8fef36bebda4b4fa9a12c47f941ebe71d3c133475d3083789a9abb52dac36b45ee7b5fe251d5547c8662e96f22264a67b6669bb29f06548efe2145a076c246e6f835fa5465525754d891f8421490713b8de667e200ceac76a44e882c582563c50fa3c9fefe2f34734f8be6a5675bd5cbbbc9610cd4482d8d3e24dd4227f46b9f1fd7a1dd2851432d99559345641e036a4cfdd94b673714a9430c775f1250a9901077431c5611d34f90a78e945fc3610459b525f20934858d241a4108aabc5b9c96ff8e3dafbf94c9d1c14cce4a79322103dfc76a5520e97ad8fb07f9e16a565ddf426edb447836c388a14139939a2774a9ccf85763ed88ec5542b0423cefabb13ae975f95d0226b1b282122d62c4ec11f19afb536e452bdf26045ea65c47c0d37ef5bd718b54fe597664fef53d81ace8c842c7bffe8488b1d6f8f8a43b9b0ca73bf5c8c2c9f656dd280011e4701c40e38b5a4cda5d42977efd1f26504eddf6e2b5b43f1897d7e08103d86e9a722826d14b921983df9bcee18e057c9ef90316a1b951ad5322dc273fa1cb19ea675890d30426ff44ff22a8cc8fac9f930514e608dc9589984c02a448b1913e116f6c9976bba54d002d21badb72c288286350a793d10f65cdf43d9bf7de4ae022e3ec3a90f89aaf32c93f298d79b9522407afbd2b5b5da2fb9dae88ef06a7767785048044810e5e015dfa2a15f1c8dd502738b2b2560b2ceb4ed7ab09dbbdfd80b64bd83e431c84c7b9379eab6ea899b29314b759f783f22cc7690e95303872b96552eb8c88b04f4cdb8ab7b574b9f734c48e49803dfa47a3f16d0d0033d1d8349c59c5b0eda28e5932080bdd9ca4c428e5d0cd63aea0d59053b7dbe1f2701633be079f72f90d17d640562accf14239ae4a3e17b1e6549d3ba89188398b1f3e91a2ae50f47236811b6d5b6b6b189f090c40037f65e6d97ef66951cf3d5da20ffc458dcaf4ad8b63044d757e11a3f957bf1d2023102ee4fb09e3bc8342efe92ad46ca5a9f9f46d54b055376b8d7911339593b3e6dde050c09207019f82ca52cd6eebe2e076223740fad93a5ca24e9a5e4186ef8e68de48d5e6f7aeec7e3e612fa1018b7d5bd5e32e5a454ce9b9c7c5057f08f26b5bf857c4c6d35a5da8f039dfbd550af42d3078817f794d54836d1da40127641273019815272b1b63a8c09d61447ecd5694ed51d3781f85e317fed8b37b9cf33ac7ec873887b761d10c81058ad648602a768b17adde051cfc348cf525e69a76cf7c82756341eabb7e8bfcc2b521b465fa2756cd51ca5924e850a72ce0037980f6b6bb0822c132f28a42ff2f65d4117479347b9303a40b203651f60baee0278ea49c5a58228019f40c5e48114185c0b8bbe6c88966f4393e3069a2b28a979feee27750fc8f527e7cc6ecee584b6143b0d6291267750812e92d489816efe50e4b3f662f1be8202bb0f6754e9ef19454e57b150db547ae7ad917990feb5e87391945dc3c56fa13947b5a85643fc296235c63f0263ab528bffacf9939237da396c2fea2c1740ba6ba47f0883fd89380ebdf31ca7e1af1082655aa37c234af3cce71b0282e6060a98ca321bf16f9301e20624da852111efb9a6f3cdb1efe00f35faf8b93ed426eb0a175303915640c5fee9fc5e273744bf0c54dfce0f0089e72d484e78fd45f4813b619b5be9f0fc5116a027bdb42313ca517b1a68e1702fb13b355ff36c75a7abc2b3ee957f56611be8ec653289091884edb6cb7842e614139c028111e00256cfdb83e2f39996617c211e05f88c020ef58dae331529db4428570ecb7355bbaffc2ec7f790bf1acea31492eab6794544ef0d21fbd1700318d48baadb18bf945c3c058a230a82cddedb4328e56637ad458a10faa1ebf9da2db58e955bf4d5d6f09663d2dacd2275ffc0500ddbdc9176ca802c8d119dbdea61f64dc62b9111762ab1b6332e25b9d7c328f021693bfd2a54714c312cf2afee05a299b4b64edcbfa20f92e00cb66a4a9e29b026db93fdd3e47d1233d16aacb2ed55f367177958600eca5a04233e417c3d02224c6351a3f70446cc95501e9f052742f8393c020c1251bacefd6c35fc2fd6c818e921570caeba614d682babcba06e05e80c963cb40ca9c1ecca284c95ddaffc4bbd6d51fb2ba013012e9488c568621a05cd87aeb449acfda0a292fca7209612d7608d974f4f05c82f90699d23d8d3cfe53cf12cdab3537c998f3015887ffa3a13a48f5e6643d3de4620d031231999a9025e69bdaa76e7d3d46253f833b8870af967139fa8897d1c85859fa6e5354af8838e3f3b8baacff7ac34f5d26dd2cc5032d8f1a312c23cfba99e9638386ba6934607ac75ec5f472b89283e95898f5d67bae73b0e10d0dfd3043b3f4f100ada014c5c37bd5ce97813b13f5ab2bcf4edc0903c505591a63444a6d38bf28c525e8937894f89b2347e9fb88d2a7647411e4455a55b61958b48cf383631868b03fd38c4a30e0ec99cc3d385ae0a21c393f875e3c78c6cdd279f1161b080b41d343ea019c6a840ac15e404092fe053673e9f51095013f953099982812ce6406166649836c678b79a2c9f33bf243f6a7c14fddb5d99c69c774fd8ac42eef5d325eaee0f1e64d8e4c14963294b5b7d121277799ea093a0c8d5954657587db9d5d8044d90e0a062d8ee0fa5b50cb508dce56bfb5dd11100536b1ac5c27ab3da7bc6e1896d3cd529ab407083981f521c6d7f66007efa99319e83e214fcb39c201753b613d99a860806e933327ebf420f41b49369febbcc064cab0cf98aec874b83f2922b5cd1764ebb3a6d9a52e798dbf1edd5e0f62c82980e0d93de0980527cad72a42dd365a7928cab27dd0a84f2311f8863fbd58bae663a7dd90d7735c32b51df77d139ca16db7e38d839a2c79957d277183c24f8fb8f5d34c9b3b0a13c8656eddfd56f4f9b18a9af8c4795d543af28202095e4c9a42c9b5b8906dfe89e5353277e2aab11ad595e12989100e3b6953d54523923e045a4f8c7742e92ac6b84d0573862d05a85092a9f337f5c9265cc6e6a35699a78370a37c174b21fe83abc3016a9be654c162f3b3954081e0f7870a9e5ae1380133417c092c960d1ada73c3a317541fc38a7d9051a6f7e45782f4bedb2e1526ad03ccd5e7ca0420ec03d4f108aae107938ebee9a0789c3052e198f6560ceb2a032932565d1d0ec85cbede8c72300cca1b6ed7c2acb8f88e5c4434ce9aba6f81c0f5f61efe900a48666810917eb2b65243c59ad19b203c2d3f7fb02b672245baa2096dcc732594dfeffc859034db0153c86485ea5ec54e32cc34164ae6927c2f227d7f9978d0d55b67a408ebd308120bd3801957d6b33b18ca77fbef613728092afb5030d9ae03975ba27e1dcc188ebbfdc8a65c77cee63061148fe900988269d48ee599b706d458ed954278eeec36f51f61ab1e0dd2a0945ce7e3191381f96464523ab407dc6ea41c71af626bbda4901a9d2bdc00beaacec6cb005edb9101e3be6942aa63ddcf226c06590ef438870a913eb71359f5e10077de89c25295ce1946ccd6503ba2267f176a19b524db8fbed4db0f0c9d968b60ebc8a5da150b31a60dd3439799b5badec3fedf8526a606920a9bc8e5ffd1f9ee979525c848ce67d794815715726c207d3b080d2072a2fe92eaa602b7988ff3db27c7bfa78050bb082f7b027d694fa61b38d9fc84a518f8faf43259b1ca9d8be73f37eecea0b6b4029cd17cfd9e07a056e399e26964dd5c75e929a69ed13384d50d02355b8d70a1fbb4fa2ae1de0c4bfbbaa7b5e8f4fb878ad74948fe9d14f3b4a9a5c11fbf43d30b7488368f61559f8bb094c21256fb202a359544ccbc54407461edae4bae31c9b39005c0371fab82574f3bd6a688d0d8b710e4c187465503c0e2d1645ee6f53525c1854437cfa8865cb2a6378cbb72fc218f0c242d4b2024a21b7a46d27bc00b8459160665babf58c84e9d61ff96d6678c0229fff212e5b286b343bd52ab9386f3dd929c13c54ea4bd19737fc5e9658b8983c2cf52f8fc2e5eb9f2240ae1ed73d528176398223dc3ac2ad16210452435905e02ee8950bc6d2708becd3047996ade4b10d9235ab882a4ff75549a2f71b3a967b64a687bf1dd0f5b8c4fb516bda8d4b336ef600764a9e7a424ba36d4b655bdd25e208ffbb3050346d87055ecc0b7d05da109956a16d781750b5410447158226b4c6d05a34467d23218fd50d28a2f2086667f855ada1ae9c51e0988a87fb6e4a0a8597f305d012fa39c9b59ba19d0f41ba9fdbbe2c8422c8cd2a1f63ccbbc098867fc538743bab5463572cf18ab25891a1dc9f4fb1e32e24318ed3b9035492d8d9a6c238bf0b4aea3819a94b751f7bac6bf4685212439a960d63c0ff1aad8448577de5c93dca986df98872ae758de18ce723b293e6ffed9e2b5125b0eb31407bab9a068570480d77595676148c2af0ee7372b9dc029254892088b38245f0f55058f02fa15ecf6b40731937add58043d9d84f2ff77631b41e60957d3debd2cd9c01e560226e235bdc12205b06391b63e40ea453426d56f7ac7bd15f5778faac40cf4eab3a75b69971dd04b8927fc364b01194573e8aa705ea71f62e7e089047b58fc7d45a2e03fe0d602db7347af78bb1d970ca2efc189809e2a1e0b7cc552ac19be186306c5f9fca8d06fa7583a3fd7af89b53ee3ac82c392b6ee3f578f9c9cdc8f839319199990a162d230c9de8533b37b8bd5e9cbc520bded8e0aa853e73e026fa8e6b5c27853f9a0356231f47366a8ac38d665be04132ef13a5526ac6a65921882fda2ce67d4b373a70b59f7c00f93c03cca0ba8a9965094d3a77e72e83ae079f26dc3552d63f178d0b89ef9807162e9b0479ed3992159aaf3f11cdd9f3771d61e8d06ffc0cfcb4a34003f0bf9dd6518c3d45f3b29aa73b2de22e8a93c23fa9f2c9988a21b004fde8065e3291dcf38852f6207bcf4b43a0087e7787817cda4f79d10305d0761817ffc0bd9e2604af7b99c1ca82f3198ebb3e1c9178b0485dc245f65f8808bb3812b6ee5e8f26b67c1dc2fb25d320abb7dac3a85abf4dd5ce81090dc121fcb320b7a271daae619c9c907cf4a48340f10873e355bab43f0de0f3a9c85477db72b16679d4b129579030b80eea201cb0d9b56054ed266b8af0f56d7c94d59eef348a242b792a275c23a9844ccf34e36f30b43fdc2f7f0fe01a985bd65acfdf7d70d2e37aebbb02be6743ddc899af42bd040f9c3b93066dc58c8a582f5e8af49a2b22f925dc999d3171ca871991da4c2c3e13262abdc1c6472428540fe90d85ff9cbd94e56d066ebf6a616e972c125c1322fbf55ffd9db5823ffbe93a765dc8e38291e2e0921016900c3527c2cdd7a4b1f9d3016f8b9158046248f60fab917edf4926533e1b5d893af806c1f4af8f282a494041bd16f8e095ea1419dafef5c72c132d974522478f70bd3919bd4dca8210acb3058e2f1b240ed819373204cc8cc6a38bdddc43c61150d596c1fc37eb2f69c1cec0de4829363fd63e8acaecc067603ba6e1f598ea91e6f32ea9eba65ad451bbf4e53bb0b1c2c3aeaa9f4a1efac7ed239bb6415bd18ebeeee5498f4a21d650427e3eb588d88ab66d0fd12f12dabd0d3e870e9883919010f5c3c26a5e5bd6b0e6c09731964ee30b3a22da7ac1ce183bf173112fbb8ccb347a6fbf806520d37c7019797e3acd7faf6e6b6acd084b1fd9f29d6f8b543ed2832a5b30cc89230801e3e21273c9a05b28ac15c2d9aa56295cc6b31e01be04e65969e3ec5bcd16a1918308e921ed677eb72139edafe341cb25af27146a0ab6a97144e72e093e6eb2fe5290146da790a6ed18b77c8d559bc36d5bfea89b9eabcc87eae6ab88aa56a1b4cb1aab1ac48d6d2cc3e98c2881d6cb8e7e7013be445216dec6729774e08d1bec1a8c60b59df49795e6ff9e92d96ed9a17f66b2d8999e1bc9056a4edec36299c1fa79b7da7799a568bc124ceb5bbeea79a249af0d804b6286cf20c80fdaae35515fbb91be28fe89831f19b3d558742774edec46b2db3c8b24369040d379c0d96b9d55da1cd0e4e29fa229674e3803657e8319a5cde81d7c60ed0e1b73a39fec7964ff60c80cdda4986b8311d2c7b0d7547d65e5a434f49bf002f9a654b6f3f372a9dabf2f658e668ece734dd15532ea4bdafc377d9dcdde922a7b515c3977e1c5ee8f1c5fe529d65fbb088daa9801a392acfdc4b73425191a41b05d9d26630b285d53779ed68882d2fcdb1c71121803634b4f34ef5f429bfe1ef79560632000d2596c7b32c39dc19c28a6b9e4fc22ad346c5f80fe7f04b021c0b7280c5bffae0571c518fe6b2da80009eccd46ce27b652fd3cdab1e4051d4eb5724b4eb7669313f5a7d60d3a8fd991a1ac6f93e0100fea167fec9f1aa916ff95efcc25c75805816e861c6c4af067ecfa7d203568389af35befe64445c4a42bf07aaade715dbde1d049e1c9bf981e3de4c1b5502d45d85a4ca9eb85912b2788ff29dc6b041149f77fde1f1e7e86c4e0df1484f517bd4be2308f7b5e648106c6665d161a7f36cc2efe33a4fb0515663fccb69b1ddfe85107549be105fc233a9eedb66b30e4666214658506272a11c2e94584a5f49a14de5a0d48d31426ac400e09fabad5ee933dadd0a19ed81394fd8fd8048b8d462728e30af995d868b726e383196cea2e20d42426898d75218063af8491ab94fbe373c2eeaa68a3a02eedd3ef736894f4a1a69b804c1fac8a985d601ed130c8f0766e92a61e4fda972f619c8d10955d1c36bf50543042d013579649181afb5810e99edadc0ad0d736119929907140d39b31447ef0ec8d2003bd23ba8d2c22b146ba0b477ef7d6e1719075e99dac20bc5720724fd105d68217e3f5465c33e8d94ff18182b3178b09e47faa6f2dcd219f1688285248dee9690315ff9b13948215218949360ee55087631043cb448de0d72a4014cb71b5f5d282f0b9cca097c54a14bb3fc678ebf313aa35c1646ef3281e68e68582b01e5d11bd3c5e50cfff4b2bb984c7616a02e5373b6c044542559f14d7b4ca3a2624ed3713fa549d9026553db0843c8cb4bdcfc886dcd4f80ba5965466d2358b854d9cdfd6dd4a6d6982525043453cc1872d932ec2cce8c6e8482e8c3aa85133e05383c51ee17494ceec69a554d5426965b1469110a3c812105e5b7d3b0d46ce27b32e88856ef3462fab8fd5e0fe6e21a65f2ed2a183fbf6da0b2d7f89df799340aa3a7a275f3f2764022a5ba8cede5dc30d5ece0c32c47fed0c646f26eec9886a1112e49667dd5ec27cd724d79054a5b03bc0aeb662bfb1f93fe37a1ab25cbcd36cce6c450d371100c01ff58c6a339c875debe289ae4083ab559002dc5bde71630648a869bca6ce2229e6f9a5ae21fe152ef1ca3f426f3a809d251e88ff268916b81e6b1395713564adb530aa4ee2cd4aef30e002ccd6b7118dddd3b5643d13048065729983f7eae20e94e8471cada60e96d3b6d9a864e3ec0bafa93f18d7593a930ed73ab9e23862035b26aa3ae2c6a22effede2b2acb72d3948e4e31d17ceea4cda806ffeed4cac276757700bd9da36e7fa7d59eba06981c0ae8526bc928ad0430d956c82ca8cadc37c209f16a7866073b918517cbf977cff17ff4affac8f8d9aa38017f3b3472f35ec8f57c96764da3243d19b7b4d1f3179dc2e615223bbc70c6203d9ad716f655df9af08a6d18685dd54241490da1d9d48746a978bf4b969686018585eb27417b04f1a02bd7664edeebc5a5b7bd53a0efcf57382d37a312a354c80535648bdb6119f50fc264ef1e123b70cae73d0b535be2fc188d36818818eb4c54e49d386011b1aad0259084708b98846850406f35cea559b6ccab2d71feeae46a859a38225f7db5e75f36bf509413df1ebacd75ad7638b7fccaf7ffe11ee194d1aec50661db0913c1836ca79fb9f4e3d1ff8b20948bbd0a66192365e60d0ab21d477e4d274fca308e56b211627721c294e405bc114045f47cfa7c0b09e459f371cacba680c4d0821657f6927f63e0d31d2deb7342d5942824a7cab3ca7aaec68618e209551b8072c6d15cf5ab25fe0fba8b9075bdf3942b75b6b7390317dcf22b50a5e3aa16c439cdc0c7ffc86bb79bc562bc68d16caa1a67e37893b12a36b7096db3a92c217336c2cfd1ecdff85b3337e7488e1274526d8fed7dcdf1a24998a6f771255b443308e3ef902552298e536d7ae208bd721609792369f9c65d34def79ddb4c7d1f3469cba35d2fdfbaca41bffc680f668fe653b36e587756557e8375673aced5fd7cd81c26beea35e121d4abe14a868ff186e28cbfcf21b6a7c38a7d76c529455abea80f7a825dffa85a78ea13fc4eb03164a450ded228a0b60947b8a7d3198614504bbc020eaba41271fc541d2ba7f9c2e3478c9c6a8ee395cf42773ca4a676a187172435cd6d7dedf526eb63b4b8e6d394381a9f257ba93a1a93001741f5155181937f2c2e0547a58fc09b783bcb9fd82f2b76dc8a4a7275f3139a2652637401a4013bd80f1974a6020ee35ef858e8ddcc9f2c5c999223bc6a10f3792ab6797c1e809740282d0bd7480faf2bfcbefc960a74744d6d9c3b648ec0524c42e70ae5f5b65b8742949cb25dc0038459ab620611f96e6d4fe453b63cb7223136aee66c6dfdf8ca2c7f57f43a1056a61cbb20f8cbf83e4b4ee7367c8b7ac88f82c02beffa0091c5158b8b68b2492462d501452ddc65a3117bf0417bcb116f19aac5fd5e08b30aac36e866a6cd45fb3b6fe9b3ebb56eaba49a609b06e7c371bd45922344259db86f7e0105bc28df34c68c91b26eacf7d147a6181f32771ef719323818dec77fcf24ad943e6db0c396d66b4c552ccca0e723320054bb94373ab6aa34ed979b173c89fb3cd5d81a226808d5888567c785fde55b3b4a42714eba0ed24c4b7492155e75e453b77c9619d28038b974910b569e6fc887e597b2b10d39b04d4e35b4789b796fdd41c108398a0027d0b100d4a8dbdab7297eaa9987c1bbdabbf7712ed8c7ae1f15175ae497d7228782f6a021f2febec3ea82086bb7d9526b2debba57370e3986a91e2fc184aab36d692370ce7839ec08fa6fb57c517054276ed8796bec8886e7dc3e23e6e0969cc76f43506af4dfdbfd5d98529d81aa82433c9b97abd066974786b7824d4479cb2740092b53a735370b8eafd82411788e5f0cba34d8ecd9f43bcf9631db539397a2ca042b8aa91ca88c9b6069536b57aeab90a2894601b6bec83c49aa170e117ee1fee4f343fdf2cc67daf382f9c5df1c9233376b0fbe2a7b29b2a7ebfef81e9d2b1def1c53fb9f34491cf5497925811c3ee58f50aa8e51dbe06dd368bd298d1cb1907729786fcd19b0b808d56c0abd9baf18e263ab061c79615e8d248c38b01a3a30b595fa741976cc62594f75d376ac67df7c7e3d1366a925e215047fd9bea02972276528f9deac00529ae1de11e8aaff998bb3c6ec06950b603901606163455f15f60af0f6690598820744e2d22eede248613de4f61c0641421f5fe582c5d564fa97e5378d8b0563b5af6fcdb976c0e38d14283dc65b1a950327e2a8226f043b0fb91641864066a80f16dd29cd8564c5c18f85edf9a70c159e6ef86eeea90d5cf6de9a4fb113c2ba5fc48e84b3dcdf6cb523128aa225a186cde631701c95ae8b7c0e0e66f1aa5381582d7fdc48cb406b6609aeb7a802a7935b6e3aa549263e7ea67c9b6cdb5162cc7a8a8b57a48d27e5c29dbef263f9244b92bc4cfb9209d3d5e94ff300ff04b0e3dc99395d79dce974141a7bbeff7dbf2574f0bca59e228407f7bf73a1e72d1c896c9246d293d15b4e0b3d45f877df4a95d3f432e85f68429b58308af63d2a356ba6cbef7d4ca84621e54bbba8794c37a550ccd96b6ad388f101ac5dd47f91a2b2fcda072e48c2ade243b172825cdeaf8ec03e5f4dc2a6c6205fb2b1953956c02c132211da54a924d4dd44456bba456c7de66347511ec2f82cbfc2193e9b612da6f06862d21af633ce88989a11982cb5f7c78e434a54d5ee9ec2d3206ec7c50c79fc4059ddf812fd4e6af0b2d2ff7ec26b63af52329f68515afd9b8bb6f4bc34ca08dd7802aa490f383c9db03ddd760e6e3fa30b6d83c0e4bee9cb42be86099048e5a2075bac9e2a97c4baabcba0e2f16fd48717f3fea440d5cbfc621deea81584af42b01212d951943d2600a3967390b449f521223e69963534531fac8b8905b23534ec5c9eb3e5c04135f78e2b477c4978ddce30d28b518f4cfd13ec1ad0bf39227c591629b2f7d3623e391d2629ebe2575354cb8acffa562c7575babb0afcba472830f370464a522e80863432e24f22fc281caecadb238cadc0a74cd3eecb0b7ff02da8974e8771cb9e00f2de9758265544c584ce214355862c1a1ae5d660ebcd2d88841a20b9ae05b7317320c9f1673504e4d1b03eafb63739244dbb95a82921c8957fdd63d29440a1dfecc185927ba863f38fc8fd578a52a119ae84d4b0135ec4005c51ce290de9fc5a92a96f8e09b0a907081889a5428d06bef98aff74a44c9a57d009d03dfa4203f4e30b9650f9c82667ec3042ef5913bdb16a021707d7fd0ec62622cd1aa89badfcb3d251fcc2baaca9f925be006004ce9bbcd9896baed6eed15c7c02de1699b63a54e9d199aec3467d5c17279dfaa9fd8f3583a1c5d9e4d60f9889340e2232cbc4850c63d7d6d0df26dc87fda0e6b77dfb28c064e942b1bb93bd4cb399e5ce709cc3a1d86f074eb1699c12142b9155e6981828d3744ca57d49f7c3039a8af87e1fdab7472c36924083717b7c6b0b2779ccaf083d20d4f87c3083fe7c2164aeb75111acd250c6db0a1764208a2fc8cd2f57fb3ac2425ca9c294c4b5b7811a60fdd2f2be5803bae1044c6ef4cec3b87244b782e4cda8124c5f98f81c429aad010a3a8d8e6790fe69ed821e6cc0f5c1f89012dfe0944f9d09d6b824959f9edfa83b8c998685bc58d12c28b88b3b75db94947ec8b98cc032abcafb0c6529bd88a9b66fd77b4e7eb8635adabdae2ea0375ebcdaa65833f767a172af4932ce346e7b910f2b32fa64157a4a52b3fea421671301de905edcb6b3d1aa68372d13c44bbc26afca0c3525d662e416ce7dd90a7cda3dedb55b64f5c46ec2953bd2ccb6f4fc6602030829f528484ce4203648a5973d19f573022761d89ce07bfcfb986acb2817e256f4fb639233ae1d006cc68649a27e57ea0eab8fb7ea85e4c76cbeb91a35486b079d66ad8f15e5faa58dc2c56042ac3992305190b882abeb52503a4f7287cec6c66ff340390eeed3e0023898088841c085decaa313c697bc2bff0d5cbadc5069047ebb823b68b80f4ca0caa4916ecb332e9d583f96a31ca1f75fd4e197e67813af1a9b1121d280d27e994d97237ae2a0f5a097a7ed375e331d65667242e616c666627f5f2fe5260ed88e979e19760b7f9964d05e9d97322e46d10e560a474988b8581689df263033e3bbcafcf9df3617a6c81c7763dde9223903a1a594f962aadc2b6d3e63f62a5dd670843cde9a973147c23e657022034f77e283b11a8eed6412cccd6665c455246ad8b17a753d20fab49cb1d07ffe3cf01c9d3c1d56a6ed77345501dbf58fad2f34c92aa99a064c6f95718cef02648731fbe80daf1a1fe94b86200b313070638cb64989bb97bb56a4175c133686e15bde26e36dff5557a02794d97cd5e296bbc4b7f08cebe4044e2f0fcecbebb7205cb9351d29b1d26beecf855477572fa14e27ced95039ca519c02f2d1145b72cdafcc9206e6e01a4e2908f6aa0ee1e062433c8d1644963702cab9ac3f7b221d1f28c4099598e698768ab8a5ea37aaf9316b34ca0f06ffa447edfb9db85f2f720123eee1773ab0e43709d855db9902c0a5a36476da105b287174c1a5567ac58b82d0c6f069d793b9d167059042868456710832671b072c8d50cdeca04e3c7d6a76938d5b7eb5074fa755caa0a742e3962f2a496edf6c0cc74cb8709b247a46959b6058147d1eb824ccf95a75403206c6602b39c4f4a68b6bfda1d85679b269843934481c32b6c388fad52eb02e68c20d3bc58f14f062d4e271b64bc642b73f5a86a6cd9e7af5320d88575bf7b67b2cef265f7698f54994e3d49c2218672f33588710bc6c430eb2960601e7e02d880b0fc17fb5f5ccf5e2fa19e85995d8ab14118f5b56a02bf7dc31bcaf5424f4994b7705d8963553e68093a7dc1a01b568946dbf2d9ab52d3fae0cc47d669415e36184e35ff5a92dd1aca145cd45920bb2e238c430e2597b634b1c32ad683f8b36867ce795e328fd1efcf90986f22a08b6b6a9826a354d4f74bfcc7202f542e33766c7af0e0f07b1d6e0b4c535fdf1d7c3d764a96d19db7a3328635fbb44f1375e57ed4f00ade0bec99f02ef0263d7687d1fb9a71ed1a96da367eae06a19781e0de071cb6e3d2c3abc0fc10bdbabb2607006034e7f3fc1b4b1f21ab54bd0b46ff0bc28c156d13bad3cce8a9a0045b73b46075f06a377d585105cf657e8dc55b3b1660fc74f2e5b5867ec298241d05d08e6266a5f7bda24834898801980e0cc9ca71659cd2b63f7fe5804f4ad738f9f947e139f17a36fb56fbc6bcc4ffc462e2e0faf8b905ec32f0d2f06634af8096908f76e90763d00e2e046259a49ccba94f57a198bb938177d1796e2eb22b6b6684f71e140699ba23b7761748ddde5b2aec9777c385d162b7b1efa34ed593438c6346f192972efc697e4dca0adc132bf1765e6efbcc01c226c7c71b1126dff67fea3fd12daa5c07887d4c3f694e7fc1f346e50ba422615f143bbcab7cda50ec1716c3518fc347cb2d8952cd020ca09bd24822e3609f3bb4b6ca10f355615fbdf9d10b355a872d378ad9e8526b6a61f20ef09ca91c582a45c03c6702fec433b40bcc4ad160a9d5b1aa3ec17d7799d108d48f834eb010805fd95e246a829e95005bc4f57a00f32261ed791c69d140b3032e4d6db21d98cf3eeaa914ac950a7c27efc164287850b09ab6c8277126ead7c9283b2bf265b54e616b7ecfecd47f4c16bb766c5f078db7634846110e5fe1adbe90c952b59a89843a50ac3ef1665f759363f815d35462737b9f2d9cc0567f8cb1baf1d9f8c95c5acefde5a279d7a0dfc34de012c658409789a9b1aa7d91682eabe3460f82b2df0bb018be94a247642edd88a28d7035c03dc094161393cf519e4565d8c01b7dbaec1850e73d787bad906a80b3abd4c94ee6aac8f2de60c04dd40b7b19dcdca4ce18dccdb90a4e319606a263d3c40d69f529e14cc9a7c8b806adf4c9f0026010c8a5d160f74828842b76891cb815ca4de3e9595f44d41a155a621bfd073cdd638b11ccbf397ac86e4c7d1dfbc6183fc51356e6f51a1137f0877b1a5ba92142e66786cbe5b9a4f5480a2cc1e965e0f3d46902f339ddc4640a50f000314f578bebf3cb6c4492b8220b54433f7ce61859e2eded82a706f0f44198d3d84eb1da204ed879ff529330f72bc8cc062d0e72cde50a73914a0ca7c8b66a6353bb871c7b898577ba3076dd98c0bfd8576b015c2bef4674c440eab16f65b40938e5e494946f9db628b9e007dc7e0b3329f816cd17e72692c94755ffdd61caea22d29a026b39eb73b77302bdf543ee53250e898103f38eda3dd96f10824132cb286393c794e2a3051698e3d9bda0c2139c3793e6d0d9b787b1bbc4eeffed4ee9c761fc370d0d28d34780a46840ffaccf7d353736c667bc7af83fa2cac1b65b47845a0b90bcc91ab8b53f1027696d3e00c3466c00cbce0b9487e78d29ee12dd8f3c2923c704e33dd2ff39aef39858b409f2deb1fa7a2f11daf26bd9bd7f1e6261ef9d8df56efec3456294b283141d0dad0c74d597f800ffed28364fc55a5f9c3c4417eb9862745d23e9a135fc92e2ca41a9c1bf1bd22a9a72ef5540b38d94ce7bb51420c22e759627b0da4ad4b8da32be4d901388ade5e87ffbe0c91e6598d66ca554d5959013d7c8f0236d540d17276adeeadf411cec5a3d7660a2628402d929eabf2891e819a7576b07e0288bfd561d25f38eef44bed3ee8ca8dd049cf19c17520daf220ca915e47f9eeccddb35a1e11031fcaf1b32dada20b8f4c410ae57b7346206be246a1892ae4af86fe77c98b7c60c4f15c7df48df6aa69e7c1b46a9400a44cc8112e4def560ea387838b03896b4a31fbcfaf8b788418e3aec6cdc4081398ca16fcb1df6c2c020da13aed70982358905014bd07a5caa22a0c19b02b8fc652ebca77c1c61a73b6fb5789c42fbf137b0944453f7d227a8a3f9be7e041f1e3d73b90835874faf20f9cdbc27347596265ee0cd3b3c3dc8bdb838a5ac2b1360b7d8683819171e2259f370d1143da4b96a2a97eaf6f246e5b03043492cfe2dccb34ebbf4bafe41e52f022eceb359883c58b25a09dc23de04606603a3dcbe0ee697d2b4a4279d1a36507ab8d64a27935798facde82eba7243137ca59c6db4c84e9b87af837f4599aa2433388e2a071e22209b52902b4ff86636e5a8625aaba18f28c2718e20b064c345eb7f6ea8084acd2dafc3e49da589834bed05e551a2b0c28e09d4b25058b4a61608af8feacb318c52e356df15bca9c2b49d03d7b02b36d24f182482a67dd30389b87961b8801dd5f1b162035902e18c3d4d53716ac5b3852b3f01ee887929ff87cfb59ec2c01dc38583e3d0a4bd557b1c7241f28aa1469a89e21e39688d30d071f64ae274ca2aa563883c69246b276a861c1a8d26e1ff64d345c6409a4b010e365c5764ecd21149b96f87bcebd676be2fc9da9cdc3d418646665da389a4a878302df500dc6f2eb25454c8640b01c6e3ccc30d652a80f50be8dee0202ff209eee48b1b00cd8bed885f0f19688b21525a23db73008d784efbb1de71cb855df04c181cd2b0aa457dd5364779d1386ceb9ad511219d3b59fa6a532540797d68029464899d8204fad8bbbb6509af5727ae104075d926770960880d70f03e474bef80d0a0ca8a394effc1759ad96def9cc275cca073b5c476b4141a27490347f186439b7c6b841ea97f131e1a61791f2ea64d07814c8e0a40147a09da94a4aee0734ef8a4f539e97e1738b5a1f3fcf464ce3ea95eb9a44c99133f96c8a0e271d0668c319f548215cf4319377e003a0af2ba17bcf249a145d810b5526954760ef848e02111c06423acc1140ea932aad566348bf0c1bce4bc8b6d6b10bde1400d829b1c86f90d3cfd63a6151dbdffb96c3e3ff51e0e48abbf0742a18252ff4524189debb362d19d1157bf230d97cb18fcb89a988575f21485794752e998c87c377113a1f335d714fac4e66465d47ac8467f36fddc41c91da15b8744a8cd8f4d28ef69f1394bea73ffea8161a56b3d87d83a63544f2c78f808eaa7ac3f7d9d79851bfc27c627200c7290c69f3fc98d4d69ea961038331882e98ff5037a9f5b2fd7ae946148c07e612346ac27035ac193c12c41a0d7551d41b755d8dcfc70b92fc48a78c240fee5a2f1cf25c4f02cf353b409febc231e8fdc5bbcfe863a747c85c2348ac3a0aebb9511a96299bd27c8ff3d48189cc7e1f0a7d5401f74edf384e91750826c87cf99411c2192f7add872202bb8b72f8ced21d3fb73429589540b42c624f61b5c9666f6581ee0b56b72dcd3a502cc90678a0111e95cc386a0acf9a32029cb5c45e79dfaf0db6c4fa76672423c5cbe572dce2555f893a5786d8ac88b543c2a57b76173f810a320b5d69b9442faa01e68ce5adfb96acd5b913702ad55d2fe3d45621be351e5a866dad776ca1fa42ea1937fdbb33c610ee3e59f75fc6dca223573b58ba3dce7f86d3523f53fd276c741b656e92d438927d769b4887c348d2eb258ad35d70fb978aa5c5f48f1999b0c747e262822c6d421a4c12be817e2155e6c86da761c563121eb98ad30c263cf8eb4e7aacc44d34a85c6b0eb7b1aeef7f5f6247337259eeea114785d12d12619b14353e5e4d50309688381aa490b75e32def00cfa7201b0a0f3d495ffc85e8fd7fe546553d310ab0875b7bcba66efec1c2f9a351b84700bb8c067abafea6044a376c36d23b557e5b7047fc621730c8558e7bdcd6765aeb0cb76d2789199c1af17601faf352561d87095ee761711e6e294f2906fb8855dd053a6ea430415684e36b7547c90142a39d79124b0d99bf2a0ed0f3b08eceab7b9d08c7d4f7df815c08d776dd45195ac26067bceeb91abbf33000a5ec4b3f725018339f333cf8cad5b297865bd40739689ce4863ac9c29a960e39942811dc337f2fbc4284d35987c615ccc14eaefd1346dc0eb568f3a591242d1261f5ad776a3611f79aa0bbe24bd4ed488fbe6ac1d9fec7aca4e8c1f2daa421ee93aca213fa1fd4a0e1a22f577896b3d35ac0e4b5b91d193a8ac49448180fd3c9df24a4db140a1eafee5ec74e3146b48febd628235f80f046e05164e986ebb0de258c202228c020029391d867100527bed871856532441dad96e1e6d52c822d70434098a3bef6668edce20d1693c16b15c900fca5407a9262916327dbf5abf5d3e0c6ce32bcbd00e07f6c831f225c1e2e7d67fd8c48cd5143ff5e61ed59685aac24aab7eff47a0b5dc302641ae08d733a029b7668bfe564724ac5af1e2e9b3ccdd1966ae33feedcb298756dc7e61b0f37a756bb55e909b472dc7b404f470046dc30900495b990e280887ccbe4f854731e7ee3ceff94ee8590f5f61043b96ea1240669856d3c3c9b37e9a0c234082930e4e2c4ab02d3ab922b89447c2d0a4c2ca8d0e33c0496b2b028664c55fb896f19ee450c4d266efa01b551bc1cef015788769f1963e7c9a0efe08d00e8496b4c5ba0367152250d3f162bdc7c354e3320e4f84d6a020ce48cff7940c1de5ed800c1eaddfa3f70d7077c232b25a969f1f28a427113f6c1993912914b0677b0cd2608b07f0acc56a47eccf54ec87e826d1a89828681fad5f6a014afba4bc51aba137322d32bb64bfdd79ac68ea29efb1e0a428affe258ef5c8b8e86d71ec9e807606be6a1a51c91a1f5e0e8d261196a055e848febe0bd13d1e36cb9f4aead984a2b68645fbd3f05a694821caf03fb34c23940ccc795b36df4012b75bd4604a8cb0a91c41a83d117ec063b5fb70f9a5d1c74c9b648c3a4ae35c4e22266df6dce7e88295cfc8fe0f6d411c56ecacb5f0832d03207d7f46b96092aa2ee509e878d584f4fb213c8a3bbac5d0973da22c18f69591b1a602aab5fc2f00ee04d3c6b9c5b2009f94ba1b6cb900dc1ff65477b46933d6b2e1b3c91ca9dfe3c7b74b6c13862f866c9c78d11abf8f5adab26a1b8f8602641a63c132345496154bcc56499304b0bd2d7466330bb17fd32f1be3777854568a0b70687abc2446effb907a615a9a4e2d1760f56b55b173bd3a0cdb7183107beb85a041bff597992ab835975c84e70bfba0bec66f42ce3f93b8685116ccd8a53e442536424b227d190835280aa75a576cec09cb22747bbbbee8394c4b3a0e38b10bf3e15f33fe13c61199526a5d326cd85eaa3198848e80722a8685a1d104bbb92adcfba86005acdcdb8109d1e5ae03f20ac950b91aa8b0c6a53f79704db9eab474b52a1ac654dd935425f06b5be25dbc34218f1789ba2a90b9508f4437b700ffa9dc98f9f739d5f900bfb397a0142152f0e468d59febd2d5f057c54dda4b50b7e11100a2905fd289dbb4c7e6897a4728417c4f1fa89507ffb6daa96b28a69f5f85af1dae539eb95ca9bbffb3f0781a8fb2be32f814485d845480f353c91aa5f1211999086ce14754e5a6f32de3a5b259aff8404d336f150d1f068e5e17b72a3d12d7b14a83e6c86f71c63a71fabe8fe833ecd26110b2faeb4b719666c1c4e8e7ee1084474b94c4963cbf1a8549277a931b5928d4bf3fda15a0abeb07e5085041ecea55d2dc620aafc3f6f92ac580e7a6fc39b4b67d9485cf9f5aba2ebeb097c44b4f2afef4bb38875599777faff92b75b953c9f7f6e9924110a48b751e3516e3b8915bd700c999f1ad85c5becfce3f4b1a067a707408625acc614c7ea0a1277a370768466fcb347076a4fa66288490277a1def95ee2570ec1ceafc7fe657212f6a2eb8d84f56f1b01705f30f5f0f28a179d724b32c3d697a475e39ad40203e2ba22ff25459551758837c5342e5706e5d4e533e2d17894f4b9feb0a3c032859717b711a181c93a712a7268a8da9eaf3f810267aeb454bd76ad9febcae01ab06d6ecf3193c096a0708fb0dcecb96f52682fbd5899d495a374ca35e479832e667d31455e4a6d6643b7a185870985ce58cb3b209c4f4fa886fffb312902f8375e9b53a7a83aebdd3b2f10d0cbc96617e90ff57adca48954607bb032c3937193cec8215c17c534bc7a098ead618a6fcdf9b9831fb60e9486579f30682e4cdf468882de97791da7a87ee479789215e9eaf5ece51f956cfd4483c3822406b65b55906c5967efe9535911f87d3fe22717143d59b07b54fb6e72f9dc74d4994ab7839c1d0202a2afdee937042c5498a4cda814bd5b1f43575ec6f97a68fb2b0c54b62266a664555e237af85ea322f3a248e29a957ffaec7a1b7ba71429d7d7baf71361e03d44063040bee13a6dae369e1c7f81924af837ab27b90bb35e37e3ec0c5cf96193fc1780f2a9f4d441f6faa8397f4c77b15bff9086c877ffb9b8c5d9678758101772dfc9f4f9c911112ed9bd8c9cd64a7de947728a3218e2a481aab0757e1cb9e20c614f4937d5973e2553e860833e979fd55d0582bd0ccbe322d8fe136d33e78b915dfaa199544e164254e4b38bad466ed75f0f3c8d6e318c1c17b1d672e7ada7140411e0215eb49d47e3fe7725b5a1df8d841912c36c50495c7cebc8ae48ab59f8025a6cb1187e4e847d693e3087324e41b55f7493fce5cbf87cd560190ac0d54c59cf70d5727e31b4a7495682c6eefcb29fd0b765d0e7d84d86b113b7f5256687d0ba3f998ea0945eb35b494b5d2a3e74d766693ddf75d41c46a9713f04df64da711a26ff4d589467b04ef82cdbefc12ef8eb7c488a21f3cc449bed4b42f8c0a4dcf127b2d9a95144398a28cab120e283b6d66b2d22b0738c65ae0004f34c70ee8215cd77a5797971e71cc2cc69a229392caf4191d03861b0c4d95f777c8116660d4fe61299211b45dc2586cdc76e2a4d207da59b6a388956fea4c1a924eb6ff1c207f8d961287684ecb5e33bd9a8ef35af1c71f48f520d55671646570faef702b27c20aa749edf76e02dcd7c906b982584dab45221554605a741f591715d3c1a058f6076338e92ef9ae1149af45cac0589aea095e3a26774befc1c53701e40401dcdfb9a2b23debc093c54a7be998c97c2b30c921e5e2fed8dd3112b51fbe8d997cad396fe0f81fd7e15d8d1539c8ff2c393a2511c8ead319321c5a1f7419a39a6f756f4c106612b7a1728a05d229deeaf22e1c2f17a26025e0634a19f407ccdb992afa96e348e3b933e84995bd2737aa7d6bb3c513f40b6b7a117ca97d9bf49ccf9dc25ac88185ef3e40fdd8d7d4324ab51e62be630d6112f2d6e69e6ab914fb510c787a152a04ee5ef6f6d58e0151a48de6f518df73c570625ac8fe323697c096d9b5e44c98c38c3df9d744f87df590acf8a8699d8998ceba9acb54693d7e7705d43d2ded5f11ac1841f34c36f634ac095a500ae4b9f4232044d54c6e74d60058fa38f578c6e469113fb0402541769acff73d2fe810f972deaba9da4e9da8d9d9b68c550154f36e5b7ceeccebf2f8ee27fa184df767c2cfb6c2d73834a9d6d8e36764a6168f02785a3a5bd2d8e585508bd21c0b4040529487142a7fa51614cacf86fb4b40129b51f203777e15c968849d433d22f02c70ba29657e12d8d559f14769ff22d1869b88da162eaee45fe0cd97b078801bbadf1c4b0f129442b6a4227ea2d4bef6dbde20eca614f3b0cebe9ebffa76606e59687fe65c9bc178014b126d5352711be91eb638773ed4c4cc75bb47b3fb6133375e1193d1da94228a5d3fd1e6ed8bd0c9eb25ab8496dab05d5b049c97111ccef135708d575fe1596477cc03ff21e9b32a6e24e9faeb6717481dd7f84087a914390a419c0765b670dfaf57fd9fdd19c1fd7dc1bf6f84f8db8d3dd0887ca1066293b483e3253167e429739846abeee86d4889f58886ee1d5be5ed81e7d337c34df08144dc7acd6c8b366845feedcec5aa8a629e828637b8e90f1327d586e0986a2407ddd8a184318fd5b832b57a78dc144f74347a309ffe482b7515434fa60fd2099485743adee40dd16a4e5db55c1c3d5e99ee7a934b4e868037814c22b42bbcb010cd3ab9c84b87a222c1239ac814a26f4d41dbcdc5cc4c8f177c575115e5df41ea6089f3708f441e394e361c31b452aaa39bb19194a6f0d92a50754ed0502d2cad8667ae0f21afa57a0708a04b52697c00d45890b55162b82a45cdf9e54ee52f33371eb96c2ef494caa8cd0d3ebc32644c62a715fbd32691557cd19d338af6d8c4eb8b0f75b41ede6447c89f01a90ed8d69cbffc46a75812b1ad668ef13beddd1bfe778588bbc65f4c19146b1401a248691dc4e4cc7e9a9b9c66a5548281ea8db145a16040c7786f01d53e871d695b4cebccd87ff0930cbd6754ccb21dcfdee3997c51a4244382c905a6bbe40590ed77e07ee7e5d4679ec6fb6c25f9830c6e9e6d07d515639c3095391034aa1befc3686d6823d9743d6b7b1c5993ec6e8daa8749bb86fede382c86d192b9f8ca8432ed28e3b08dc472636334a1ca79170539a82c63c100fcc4819c477534235ee279d71a3d385f498a6578b4960edbdf1577e46fbe812b0704ec6e024181b46b6da9f412b9f56eff1b94ce7b188c3d980bb7982b244fac8be244d00167b269c878d45dcfd94758255970f766817761fd72acba15002826cc25e229dc7ae2119d56ce0e87fbcab9faa9144eb51085b631f5ff48ff1d1e8077461240f6452e59b5e135aaeb029be4dd26624043fd455f2d70dd133f3fb79aea952ff114d7d7460c191b697ad66631ea553b86e1983e54b10fdba4b48d99c091d8621a24829cf4d9b46d3942b46f2c3a748abdc00f7054b31c1b5665161e420e31253f6404433c496fc55fe3d2ef558372b7fdc9416ea52008798686707c768e86452600cec6cb5f18843b480283dc4387e240f1468ee365dc44d847ebfb2a666afe3d9be38b77cab8abe534e61837c6de0fea5e3b0d6b1784f29c03ad0606f745abaf99506d151d2d315564b07965d459c2dede62b93c5f2558dafed6c6a7dd90c168584d2f873279ecad14a7fe043638791b1b5304abf89995676143a38a0750a0db8883f61e49b346453ef84858ede768a4ecc48bd9885d4cd3f067503c0007829d628f92fbc5e86f2185143b45eab2f7000fea213af5627e0a1fdd85e0698ec3e079a735b47e86202b3993527797641c0843331d412b3bce019a461de998e402167def430847fd3d36dc6df73ed6a137844d6385dfcb6cbd5964ffda786bb105fc82cc16f21702615a8d7fb32d163897dc12cf42d8593fe7e1cdb2bc38bb89ebaa6aa3a21e8e20caf0f6f40cb23c89a48bdc2d8b4e920e753d0f6b9232a025f07002a38a1e90c6cdf7f69d1d6f1bd0a8c221d9ef9562bcf8e8fb8e743eebe82bf052c8f2a393ef0c130997bd261463dd582dc495c5516153a905572b03e6ce290016d0721ab7126bd1cbeec8a4b0f2bb7aba8b8149890f9b5cd9c5543d8e04c51ba414dd15f6acb331d6b434cf116fea63fb9577b67a15d0e1e82410e29fdd68c1d5a6dea77225ea3ee0a699a65ed48e8ca0f4cd148bb3dddc799e5b067cdfdc3f4c16d72929b7d0ed248576c10aa833cf23be60fce4cf862fe92b98d873e2d4cb3044121a9b52858a4cb7e622cde5793457ca8b70e7aad3284276ee1225c24c6b5af8d8ee0f6a65e134f280911e6114d7d703cf0edbbd9c07476681b2a4cbc2931ff21b257aa89c8def9ca190643c58c7828f14b57c8023a2341d0c97c725f0da442a6403cb34dbf7088311decbc5ce35e8c6261dbe8b5ddc013dc9da9c587487f451b7f5019bd1fed46932e3268fce32f9a0ffe3586db03e13cc37ef56215d0f2a73884328b74b6efe890b965f138ec08b781e0f1ae7f8eb351070b9d9d9412026b4527c27b17829e0c3070a5a023acdd7311cabf49cf74ba1cee615c07c4564c06009f91c339bced3c1ec6f92efa79eaf3d7c8d229f24219dae088cef907a4f2e6350403a7431edd591732b9d0049016486359aa60a634454d2542f6400a26397627eccaf2bb70e79b5ebe04799cdf81627045f9a05875a69474572b934f6ef0d9f44c3ba8d11e07f1917ff24a6ca728bdd86dac00daaac9c935651e1b150790fcd80dcb5412bda6795db43166ba6fcf058fe2778499956e5a3134f0bc807c8e8ea7e62d81dbdd6d62350d24248323a80f02a9e9043f72b08294dd60c522a6e17701ef20ff1626606634f96f05366ef84efd0dbe816f8ec46377b19c53210ef7f2b8508d5c2af18603b2865aee9c51bbcd564568ead47df5cfaf5e74b60eb2891c5d43b317cff0be4bf2514a1a4663026f14b43a9dba9518a8964f4ef76eab08f5834cdb866c6ebede7d3d6664133228dd5284477f26dc8a1f5db8ebb57a34f856e099a3f0562f0df83c59cc9a987577322c4fb3ae3b8263e6b647599365b8518c425b1890a6c6570443cca2b97f65bd8d3d3703e3ec1cd0070127c206c279e475561f6ad5a440d7e8ba0dd12fe8df4dbe7181436096bf1ea75c86539c0c2a2f816f0a7925751f5a51f2d5dd4d83e636b02de255edacf579bc7822521993e7863fa25505f39cedddd1db7a7516735f2a840537a94e198a5aa831c7e9fe3b681bb6353f5d486ffa0d69c01176221ebd1d525cfbf8d58ef34ab9194a11e17b8f3bc2bcd0c587c59bd5f8fcd12f5210ed76801d4b8eb5f292f170e7214d29fdbcc48a30ec690a67ac053fffc859542e7e72373e19db25783be2b9d30b381b86a4dbdaa86d61f3aa7e7500132810f05140138e12debb5008aa9dc2939ddb5cbe4e4e17120a4cfc0dd799406d3c4db9fc9ebafdc9e4b8968f29a2b3145ff13db5d6845b698efcf374c626621361f97fcf59f53a33393742952e748919b3428b3f75b5569affac02ee66f738b5747347e65d8caee4e747ca594c75723975cd399f9fe49aed1da4267d369ef70d76d6ee1ab0347c8cc8e59157b035225112a2962a5d9693b9b2cad9998eeaf887b31f425ba15a2910c44d84a988b54359bff7e2d9ca648c1319526555ef1a98ad48a2b730365e3e6412c6f2e4b58ef7315d7eccaa3309c6e2894e00e750bd5536624c30f231ce695ba6c964654694e6d75aa3f15f6a495aec6658c79b65524c06fbb1ba686f021c01e5203ce31ff38c585495c9998a340c55818976e331054adf8437203bc7b6b5884eb5c006b5ed31b405422894c7ac0cf76cf5d64f09a1b99f64062d2bd36d47fc1a2d58cb0fef5bb24bb920f636fd594fde41cbc56a91db777e660a9eddcba366c7f09cf43797fc9dbe8c6f21a45804a5b4363d61c40300d0aa737b40500d37b57e570bccd513cd296780511744a86a88f2790dd7c7f80b93f7c7a29f231973070c1ed7e55c9e2dd57e96ff2a855d19e27fb684d77dfc61de0a4e839e7bd88fde479842b8aec92b4581a53bf9370e3c5be61984225751fea0318e79764074194570cac2e74ced536a58cf4455d00c3ecf345a947fcb81df90021614b1777331ffdc9ef1a7a889d237e0b5977f8e45c53da7364a37dd3f6e9b69d064f84122cc6671f0b59fb16be96e9cc68c2667a997ee585e1dc1a3f9e8dbab196963df0bf9b0c160898524b57a28b5f460940304e31bce874571f2ea05171375b31993946df8ce56d2fbac5cc6d5af13c4e08f2e2af377000089fe0b6a2923464c78774fbf6d0e8ade362bfd14728036650bdda1985e194fe088bfbde5af4c94cb27705f1ff28bc29d772ade6ebbb66bcb4d188d356177221133f8c52d60d6954de90a369c29684a27e8c446e9f916a9f0d6c60400d0019b293e92ced7375b06b908932188e5ca115db348f9be280ff742d6f313300900a6493b53421da759e1021f47a681eb9319d9392b5c9b9e9efc1e32f565502da471b87beb8755272280acd2d6aaeaf8dc3a0c8ae4901d14c7f25e6ddfd4b80e39af6900a9cb90f1d281b49d7886bc31c7b1d6381c9f42690a5745cc1877f9087e07f5bf51f948d35ca057dc2d406508031c31335b429fa7fb1d85ff2fd089123328ddc2b3f0f7852dbe7808ec185923aee88e5613bf52428e6d8f96870643c76bb2af94758f2c663ea7de837bbd95556a7ef46e97ab58b03d9db274f05d88881810a07f2e106f13cab80487cda78d4cdcdfa1ad240548b77592499383b2f0670894d0e1467d62dbf884a7073763537aaf85ccc02893291ee6a41a90ffcbe7f0fc4da8bed68966f850fb6410e3061a54dc99cf2620470438ebdd477eef30fb5730394dfcce76306a438a484549bc786e3a71de76faee35cbfb25fc6619a6f36848fee04e864e889709d52f11587256dddc3224c1ef19c6dcef35f9bfd676f7fde14db7d36ed8b33734773649a424f8f54008adafc222eddf2716559d499e72630ce630712762d953f905c6082b60a037806d370ce891e8bbb38051d06266380cc290e8ecb1a6e56286dcbfb9c28f6b789e8116b225c2a4909dd047bd441047311693258d00467c5f5fe55b1837eaadf04200f13675d0a878c544f67e212678178ae34f009bbf397b5200250e31cbb2fce8841574f48576506b7522800afdb4aa2fda93249cc18a23128fadd963a205239c6ad662b80eff878be1099ca224f4d552dfb25249bf506cfb3052fd6e7cde60e2008bd46574e0eed884245c68259b1bab1762c055bbf6d9ddd81f03b81d815be5f52bb6bf9b0674f637f7a4b0f985c1641ccae2826289c0f17bad3571865562801fefbab45d7b1ef354a37333587a70d5d8869b278eb7c17400d7ad0e078bab034bbde4fdf710539dee0e5830020165c078c81fcc60a68048ea0007ea2e60ec43e0f67110476e001e84849872f3c725b505573809210c1493955efdeed407e18be3b33ef59b616d78eafbe899a2670228ec213621fea8fcbad452d81794a0330a6f907d6c70fad93b4ea56e0baa0141f655a73b9751e72f342a7ccf95e9b3e6ab004a9e2c6c27746cba18f7a57e5b72d26f1ce0c023aed1ee5d420f36ba9e72d96e0af338cef7efd2c7c217a354a48c82139c96566d4d6d53c14fdc2ce8b4c7b1589b43dfdbd067f85e28d7debcfc6c87a37175d6c55bd9b030460bf434093d2aa44709e339bb04fe8df307f745e4f7a0524de2f9088df62000c2622b45a624f0798e4fcad8ba21abcd3fa3132f78a0ba3ef7a779bc9bdea314b3db737297290b676c8b0c147e706ab78b759e7aca07f23e14d5331e0f4c616ba0c8b36a51b8f6ebdf1c7dec5fd303a905ac93b776af7664d0d821b56a1875fa5c553fb21428ed7c4ef476e12ef2f5365f43a1fb9b29f2aef12c473b239fcd8404f96e1e4732a313c6c50065e2f9bfd20d56fb0c879af5fa41a629a544907d65b8968003af7fc949e07dee65face515293ddbbae9ebc1ef5ffc0af1a8f5d00c15277cf37854658ebf3bb73a70edf55f31ef7f96e5464165e663999a9da26b5f7d68e41617790597cd1522544ca6869713c91273ebd3caa1df84d2fbb804f418277aaa7cf6d2a7651d925cec065673964deaa1ae42e4b50499207cc3854fcbd65b8a98965d3a86781dc00f1099c9b1288b3880423825e92cf1ddf87ab3c11c1b09092e18a84ca77f95e649ec690d91e5db4440e8b8e157ea551f239286d3ceaec02a2c614a60483a0dadd30d0a1151c0041ad9bb659c2ecd8dd2ff89c0dc006cf76dd541c5c768b9eb2b440d8a15a84893e8802120cbb879171b56f2ca7d2b70c5648d87b3dd365c7bfa850ebacc8e3fcc0a2282b1c6731251965627341639dca0436719d3b05f02dd41f96e1611c0a8be627745b9a4fcc71f3a9ab208710777b548cdf9a80e6c30f2bf0b872765cbf4e42dd4aa879cacbaea42aacb9f280cd867f50aabe340938b620afda02b6da415c95797a023b3ed194f5308910545a7d44d0edff813a81047b14f2c62c9ebeea237f0ed698aed6a85e46678e4039681617b9b829828c3cc0e0f62f86e03d522f55c24fd930ac6207089f92bae26615d0420edf7f34bc1e095415ff378f5e88df254c73bdc7b5dd038adc4c50e25838e6a4867a13bbea4153f7f248dd9e0e12eb850d61231a15b774e552c5251ba74552a36b96ff0e290ae10516c186d7e78f7ec484fb21a2acaa22a9251c85a2f799f2e1fe657f989fc4d5578902b8f0c0c318b6b6319f580c0a37af8a178ffa524f7dc52f93cbea9c6215bd6b42d575307f6a7cc30d0a60b382d0d0dce18d270d5c4455a4609c617d8468bc2ee6761b211f07b6000da74f988097c0bbb7ae8702b43a7a930c398a110902859baf4ebd9c014753fe18a2a3d0279b88aa3b02268797051d3b2bd7ba9823f8cf6bc7fc4f063a6ff2605f1fbca9c3638c100f3de584e9f302e285f44f851481d621a6a2f116598258aa5bc0eeccef26f51c2d3623da64d247cefc3ca24dbbc86500cd79ffa0e3bdf2dbe2769cc8216c3db0d80448632f51ad7843d095278c6ea91b097934c9af0926eed6052619f7772ce4691cb9a09efd883bfc5438f96fe2859d2670ad0a41d8699f2cf188f3c1e12dcec8f8e0395eb94ea4a5ff49f7a8d7093c3d79ed4a7b0d0cd68cbba70fdd091e74c4e03b446985b1431306e3c52895fd5c15fcf3321c63eb68b0e2c9a4108c0fe94c82376244e775e3b9283d9f60b62a472c72a73b814d698f6adff093425f88e198dc4f9813a3a7dbf1b876ffa3dae732d4d6ac5d38bfc7aa140938c294d6e6adae97110fcea632dffa9d00b002e5b3a982f455352c8bd3e6df188ee9ba3c57469e3addcf3ca4603069071cb84ec66a029dfb7146b63d939c8d452ca26cc456270afd340e1991a68ccdd32b34bee07abcce87954c050b608dcc8d078ad37422cb9bfe60b606623a9be06cc613facbb9b20acf3c67a2e62948de71ff1fdb8a4ebb5664af867997bc5f6da9da498a63833b2b7d6bdba1641c7c11867cc11e9096824468acd66ed4c5b36d972701cd6471d530193a4ab73b14c2b87aed49669f469c23ba5ab4d8421b52a6a89845220472d98408f9f5cf667258d29bc7af2d6323a5797b77f83ed84070a42340e000210d2b43833e0af19f2da6d60c0459a514616489b11d762ee2c88bcb1902e32d534152a52c9a914dc18c1f5c34fc0e585d0e36b2da98e09ad1bd08d99459d69ab2f93a237d1e2c988aee85b382faf99d949fcb33ac2bd45dc9667321c197672261ee853fe5925f9c12fdaf45d29e88e05d2ed78ed736efa7697da6ae615961666b5ed2443fc414ece262fa4761cde33daa794248f9026ef3e0806142279bd784996d316b6b6e482299b894c9b0099e59b4ff2c9d10b7da5abb8da612d3910f4ac46f357cf64fd705b1141057162e73932670ee4ae9d707b20a56f6a549335dcbd1e4ac96ac272499abb33d9cdbef50e16e1b5f9e098c6de9ad93bbab7d04b7b83eb5e12aeded78ae96040f2615dda5ac9b723f0c9eabc772ba2427cfd71bc7263c131924978468a586189b4bb9cf716d34eac75fb964156d3af713186639f3632f5e46e80dfc17f30b34b023eb5c85fe148db360316caae35c0780bb62e4d11c59340034e9a34d5d95ab238e9ae7f7ab8a3d9eba6b71b72fbcf2a67827a817aa8c622bc393af4945855f6f2bc7c99d6a80a19531e90adf2b38c559f7efb4d26d24a603f5c475ae6d09a1773f586cea6427bb3eb0cfae70544525899bc74eb8e1f90dc6dc1dcc1ca0f2cb11b0f5324f8d527423d4a758d165d7017ec0785461e10fc38ff2af173c3c0f03b69e6aa478115b8ef1bbb8b5308d7a979a94a3940a3178c769da6383b026465e35546cd95ab677a2e97fdc458749fdf454c08f779dc4f008bb389276890eddbf897a1b731aed65529ea175128d18418aba2dcb7f5100a8548c42dfbcb674346758dae47b8b3f721bdf63b2f9dec8f77cbd892cd0f5e13425fd1258511b219a75b4ad5e5c27a16bcd28b8882196bb9823cdee77bf7351900a48d0e86e68fa73b69a251b77aef2757995403bfd567e32a6ba940e30264e463def233c73d228b49d6a68ed7d962d077e18b49cf7b3ce2b5a411b8565218bce8498f05bf7989546945872a59736ce2706b9487944a44729d7667328ce46b86efa9e6f00617312155dfe4deba639891fce66a052231b645a2ca1bf9c2147b73ad6c72b0a14eb1d6e6ad9b825f65bc7e1b223f900a796463e8af05436016820addcc30039a5e5c5cfaaf0b111839e275530123897f0f4762a8d2c497f09150cef52129283c388cd265c81e76025a6a1ec3bb6f21fb27f454e2daa3a3fafef70874564e1a612945bb1b85489f3d1ee039d729a0a04ccafde18e538f761de556d2324a9e5ee3d29ba4f96155bbe4e2f8f6e877f08d58f44a2bf93518293695e29d5d5748a846f2295d923245a43fbea5b526ab4c318bd72782b1838adfb3aca3b45b1e5fc9a0599ea46642994f809b9fcdee3692e8e4cddd7d3371f0130c076e8971fd709f8f0836be4134b50458e0f66c5eafa94938ca15bbfc90f0500321dbe91fcaaa2b1ac9f056b8c20bb37d2f77fa8c3eacaf052278ed46b0d5a79851a44016421ab1376c46dfbbe8703c655c43ed590c5c3c9a5d6acb8a4c4b42bad404ee47be492ef6b06a8c0967e851e183e1fab4b26ca6cb108156bfdf053d54ac2f9cb0d8effc01895de97095db0b69c638969e16748f33b9e2b464cead723c820d5a0233f5238f4bf5f914bafdfe744c3c3ebd92922857d78cdbe0c2ab9fc242ee07f569d54104c2aa30caa0fb83c9949099317899a0a51629fdfa6aca11d6a758f03fec02c07b30c29bf51db89dbeebccee583f3bae21de25860aaa98cf4184bbd47d328729b53b3f40badb838a3c5b0c3aad3b25831415f9f694768d4a5ff0d237f6f9d1d800ccdecf79fb1c086c10b317de4b66ef716a507605b7991f8283b7ae09a1101e2d16e4986a7ea0179e0a511f0b7487d24ba354f27d03de5aaf02983646c35aaee284a1553b07fba2ef0dff748d8c536447eeffd98466feac5ef304ceeb3472b2ad8b95c0f83eb6f60cbba75302bdf087c8213fe1df8edc357113f159e1c72efc49280f652131798595a45f34a1d906ac2fb90cd1202fb37a801a08c0fd5318a62fd0f24d64bfb2d580eba389440b47a663292f1c8350bec0bb091f700963cbfd39f9793c223905c088b68b20595bd8543aaa46ae2c879b00ea08014e3e29c99b777a6617fd9ecad7b0e0955a28cf112be0be9ccca718ac99453b5c6df7c809a594ec9211fc10ece5c4f5f4208804027e03b86c2304a38b2db01aa1e2eab9da052c247985f5161285c90ed41f6ef109c1d612658967209bbce369bc1a0240e329bd3da29740b03cfb70a18d1c8e5b88a9ea93f73f0ab86647929ae821c7c76ba1f0daa5479effe10e80c18a9d10d0345dfda721b2a96b210e422c7c93f3b0e4b5772ac166833f1808cf7aa6aa29ab7bc72ddf0c831efe1f527b4fac20e12af5771014f975d2b299343ac7267d0925412f65002f84a9c89c67a3d573dcf3fa11ac93d98b85efccfc05b477a91a81c93b120c7aaa8d4774216b9b35db6fc8a7b3aff760e62925b682bd58c7f82ede774b154d581c218ca75d0701dbdfbe3c4c942ef7e0cbef35937df84d962cbbc558734e29b68fa04db28d6cc8c71f953bfc738ded2cf0b5b527365944933cb31076a661dcf39edf29e1081e43da656ee0a9ae1bbfc17c9e29b0ab2e9922e365fd4357bf862f3db38e21d8ee6c330e57e5609b6e102bdb063aab98415644a7792fdca9f7d854d59ea4e93022856285ce6980599e097c44837aeb2d01ebe0cced1d2c8ca175819a02eaaf8d650cbdb0bc3cd97f4cd699b5ad67d29eef176ea9b06630216fbab410a2f9a3c1486ff7cd63d9c7a8ea1d1689ba9d609644c6919ed01c4c051e598fb96cede76793800b7b76693dfb4f92a1a546b0bf4b6bb7002f3160104006e197fccfa11927c00ce43534297f5fd0e7c0f253fd6a66282b40f9c5572a216ec41ad27d58f0ea11ca28363e59c67057521f7a02098511ea81f1097bc3bd7fbca92afb20d2943d0737549c8162b2237ee205960a1f18dd3bc1abc3da4f1975e268a7e222277453a2cca117b82c84742eb22fa3e8c75b707b2dcca99e25fafda36a375589a1c3b6611d0275048f0e536dac1f2782757fbbf3add5c9ee73e2f236347621337176761030bb4a637142651f6951675aadfcb0330f67eb8edec5fde8716077c0e017e369f30bda93961ca69b8b7db6326e45f046303065f2be153794aa891619d4d32f64c99e8ad6747ed793fed8801bb853f8b0b843ffef43d7d7a495d57abe2935068b40db430274c9c937efe2899c6e60919f50749fa42870766e556d57c4ebcd09fcc6ef23e8793204e738b92a1eb57cea492c04554a57b2fa7af9fb5c9013a52c237b68d48cdbc815d34e352fd4c74125693532cb66eb58ba9498a950cbfe0d1d93195958a0eb7199831bb07b10d7453a3deed444fb8cd53eae82b44fc613356ea1db50efc40e56595b695e956724bd1926b5fb714e36cd76a02d30e5c404cbfc064018dd2547a974345dadba1d44e7f50f5deeff60c1ebdde6f19d466786117c51d254119250c301f7b290c0a3b7f5a496effdc83d5f840ab6c77434a42262eb360caa11e0083e4ee488d693f561b5112076267091915ed79a0f59a403057f0e4f377653c2894a332c1f7682e550ffd331c68294795d5122d5be9c6d6d7b4c97049f80a5949a4c2f7ca192f145ff8128369aae9e24430ccca67e9a4f16f7993406d64829dffad20a9c9866e6bdaa277760ffbc37ccf6a546c8eda6a9c812588276d74c022bd485a6eca3598ab2e6ff15ba595dc6cc3838cbc33b976e55433832613bd0791872155e623e611010d8aeb92286769e20542e442f290f98409da581a12763b7ba3750a9fabe7c96196ed8e83e8db692b58ab9cc80155fc790933e4e08dff162ef33f23b20478f27e5fcca88bd53711bae9311ea5af65e21413e87b32245683ed634f24e501ea358632b5f6c6e9a52056a76fda3b9712f4fa3b64a5e84c954446f7db1405ab75b6b10c77e525e235a2c81c969f2c5a7a076abf9cda13caecefd48552d3220c856cf4b3d0dae8e207c5b82cb4adfc49e417c2cfd63b086105012cec495f98e5861a4f87f0e198e87e00ca89767de6906329b6056c74ba848a317f73908cd0dddb61322f018b1c700c73ff4dcaf329f1f81470db4ca25d567caafef2eb57e77ca4fcea9797f4134b7820b42b58273a39b89f662231fe1c22d88e662bcce7f43a43bff7b86ba16d2dbc01535297d2f7ef1a879cf59bf28a34065ab11f75169f0f8728896a73e5b8f9902e953ddc08c12498d7a684a202047febeeffd378e0fd6d4e5fa7dec05af7cc87dbc6471319f0162ec68b29d8c0693825a8128e2978a8f17212297dfa97f3b8a87fbce61a888ffbf1bd0777911711783a8b90da30b6bdbb8e35346244c0efab0206a686e9f509a25fbb4553b9b84c0341dfc9299bcb6f6bffcc1472a1c45ffde49d61243ea2a0ec56290dc7cf1a0c6bb6f14b4997c08875b0cc05c4cbfd71f7da7f8f0a2d83045da6be90806a576cd0dc174aae6f201d01c980f6edeb2988ebcad1e7381f134116f317a9cdcc06acbee3a06085c52550377283cb1a62dd70c3c7762a17e2028e9d1d740301c09a2993c27c3f8a5e2bc64e98728d8d1b127b0bbc23c5f838f4affad99cb3e74b58d54e4e5410a2ac52b05e565883ac99088056882c225afa6116f884a7ede2c30a875944851703396ff197de1fc05dfe86febb353ab6e96d04ba74e434af640d23d8c32315f68f4740ae164247657332e78368d95b018fab8e578bda6b849b0e195a159cfff76cbc82349e4b6a4ff7215c483190de579ffc9b274c202115383efc94410efe48c4623216822e91eb7dc33ee1bd81e350a6b126c6bd884cde12c8cbbb562e5bdcdffa95d748c970df2f5f05c63ad4248fc1a622b8394244891cfffea034abaa0dd18eff65ece88303b9e1270ec77ec93808264ccabbffe3439a85fadfaa9ccdcac4c6ffead12e06cbffef56a2aa1f7e33838260ff0a0ff7c8c6b6ae0fec1247fbd3f6f574b5a86307b9ad8a70eaaae217c94218a0164db4e601511ed85d0108c0b472cdc58fad1b7cc0bab11cb38d69a5b53b427de2a32c00e49cdbe004256802f3b85afdb12c58b2fc4a815452dcb79e0fdb0dd1b39d5fa49bdd3e068077d97daa85f564a5a75f0d6258e4cbec5255d00aec1258b6120285511e181d4507410c3fd0c4c320755430cb87da2dbd751b2e90f6cac602828bf8b102dc3b2350c48df8f20934cefb2037315d67d862b8fc4f2580c54ce05f49544d956a59ffe951b410c586b27a96b689e8587b690258f0df284619397fce939443da0ea9e08e004369f186e790b94963fec153300513d76799c952707f8d2628d7a4d23aef02290cd78fcff549a9fdc2468a727bb1aa5227df4f7ed0b95b28a559b31e7d7d6e4fb6d5cc3ce6e466c20a29ddb912f6b74e0a19ab65734323ca68b908ec9b1f2bde1859def6c4bef0b87469765ba4f51328168ad32213c4e26c18822420a673895ef91fadad8d1e94465691c53ddbef5a8f7175413dfca545b7c90915261747c48e8ddc48297c5941b442c569011a2379e86ba067c23d1b6f1ad591740003abff7bbc06bd938bd6e1b2465f969abe3ed9d85249c23f1bb0763041b13d2f8456caaec4eb26557242995936af4f4f82005bbfdfd0c37f05c500a806dd55320dbce4df28ba4fd8c4dea40945b76d93d482eb15f193a7bacc6b87c34082e5b5cb570ba51d4cca139cd82446b6fb490964cd4ccc6271adc1cbc52f03e2a0b7c46c79ed27a871341257bfff679ae7f11369ced52e76b9d66a803cbf7e25778de850a3163436a86357302a31ec9a526e5f3c703423e7acead096d1718f8ca5e3dc30adb7695eee581c21217289af28442d7ab1d3dd93f42e5aff2536a2f774c51df7d5683d6fb06be95cbee21c01506e75e719b59fbe2455994906adb6dce4707120ade4d2bd8b081bcf9fa9e407e0e87d62a1fbbefec2aa6bbea712adabff8854e15728a0340eb8032446bc230bb504c39844283363c445b1853f6a28f6c44305b066d850511e5c1f46eb6b7ad51dbc12689a969d041e89f4d4802fa236b74539b885f0ce30bf57cefb623382dde4f60387656a8925b170120ba2fe346fe317a6a4a5706ca0432d47af37996af1891f4d1fa83be593e2518f7e451b392de6a086e5219e9c9ea4b4bbad49dbc9ffc9e572fd4a16147821ac94069a6fa0d776ba996bed3e80e77b03e68d819c8f751219cbc0820f9267bc052374ca41d206807c8f4873fbaac1c1b685579ce9d269c836d296f4c816432a96a0af93620050b9f0ca8b21515146d5cc1cbfa3e28cb65f4576ec479a722b617cab912142d0265f8135a096c10c188b4cd1f17df24feae63a711165d07963fe21d602ef6997e8d1572d3c8bae5d37aa9d71de7df591054418577e3c2e80b14628459dd7183e1f1e2741017669b5416b094e764f056081f2b84fb47eedd0be875c60aa66efdec6eb61344603397621040b4bc6b241fff688416357007abe47905c081b4d74dae607f0e5b280579f91d83e3f7c29f29e55d598eb13506713dcf461d77fff59f26d05d54c4edbbbfa4baffbfefcd340ac402a348ba2581e477dd270a23a881f34259e9b0e362d9aa1b4681b3cd2d1cbfa3742dec0e055e38567c7716c920cf8de3cfb8c536a81012344460224139f48bb5a9d8e830d60de4dd142f24818280753a16ae8813e78dd228466a3a6a56c6ed9f9333c9133d4f76b2c38faf487daee1de44f9e23ffb7d62b4c9ffe04357406ebffc835f0f7dd830b736c5ce1ea079de611f1828dabdacf098f2f41192f1c5d1231a18dac5f6f58609ce132b6b71bb1b334ab0964ec767fc08ce70935b64b9680fa42cab5142f61a16583305ef341b3f7828aa19755d969cb83103a5deb06cf30f35f0bce272a2ec6d9393db06cd260e1a88f2e3f75392428ca21fc7753659978290b7ccb5cee234fee43bdda83dd0cc1d40847573f22a2f973db93ccf8e9738456e392ae7da370754a8ef0234c0e22448efcaf0abd3f6272701cc22e144fa9a2b20bc4519dc93c743c5cfdce036d08345e87ec55a334aa49224481a8b114fab17c2653d1a991483c70e6406783e2135600ca0481eea6e5af4c09baed0dcc7063266e16da7b603050ee519b8f14e29e3c085107e2b5c55f1e165a2d9b8c7178591bc0731e7be31eb1594ef4905f06cfae69c119f0d239a66bce40b2de9000607fa2d84d4f40ea36a2c66d758f3564ee4aaa0219c3ca38ee729f6a1fb305f2213ce5fec32280ae58db9be318f320a827b159265b9c0a0ca2c1e4abf3b70c4360538274f001f579ab9ce2445f3ee0e14b2ed719398a2a45bd3702e226a63bc9e68246d7995064e35f6d8b7049bfd9925df87aa5a771a0a43868683921c1217ef881897fa032287671d04038af3c54dde8a60eeeefff8fac25f35ecca61c3b1cacaf804ed509ab7fff3c240d58e61dcb63a2964daaabdcab7601042b9d52e8565d06321f7e4c809adc4ce6238c33ff061cdab08d6a71441b411e82d58fd2335969a310cddb7c052bb83f4b54b371884ba39daf729e922d3662d49f4bda39ea1a6b21d4e54b9910f90ab0629645d238f6021888373b0601a5387699e3623d465773c7221e8dce942ed61ae4f69d7baed6ef9dd80dfc0f8ca988d0d7a0bf89e971dcff7e82e2429d201df038fb9b44077b2e3b37165de3e638eee963dd033bfc8451190a4103c430e6f035c871344880011f9e1e2fcc9f552c3de480e629ca34f6d16f24fc7f25d601a99b9ae7d9d6e6d0d29b6e81b14704ac04e413dc8972be8bc78f551ccd718c902b4e8fa0e9d1947cfe880d9316f17af85bfacd3154750c04573253549fdc236ae67748056ccc0cd7788751e78de05085795037d213631363c5f208822b59653e2149788b5f329d592cc4912a25b7759483ca82f6003689c2a667e372c0d4e4b2b223e45b392bcbf45dac8b7c365f4682c85fe857dab11f390e65fd1f76051d6bcc2a28a18f5411cac113176268341654773303719b22585508a3848f36a58b4b941c7e5389342ac152e8879f03b2a0b959577c9c4abc752638307a8ea5043d9c7fdaf2cfc5c5c8f0feb6a5f87a3c68b748eac9bab1a2d13dd62a6e15312ba21838cc910a3e99d3f4f718fae251c4311cf83b88ad16edb898230f25613e78e8b1e752d11d15c236e306899100915788ab18b76f2f90eec71d4e6637c769c7c48c9a0fa8143f6934cdfdb8fd0608299d31d5a729cfddf28cc3e26f387dd6acdddfcb4c7975b0e459742c524873eeaf4182a365c53ef3c4cfd194f35be2557af782514bf9ee8c0af9d1eb33792acd411af5b534b2b6e2351ae0518f7886137eeb517fbe33dcd9975e0aab68cc91eeeaefac0d4c3d779c6b4ad291bfad4685eb92d7d1c4f3268c44e7161d93d1ff6f6ff91de298c5607331f874906e5075b5b3d5e761cdf463480c3bb6a9fbfb52c243fc1720f66c01d674ac8ad8bfd1bdfe6146aaa8a7aa7901b3c70a8227907163a12a2b38916ec4a22c92e8a0173b61a10e1271f2766b427d53d6cd5abd52ccc763f920acc30745ce710d0092c26393d5048dd967131755b85da798acc77a89d494f84d8dee30f799da57a10c1828e6da8ae3ec3a14ac06e86b4b0048ebdf07bc656f50d9e201d12bf17dd90bf2c7ac8dac92c1b91e07f885ea83120178730ee2c32bc00bed70cbf4a0b370e388527513327f6e99daf7d349870a13a72bb1330fdcf5e7542126a68e186615284119fc0daa7f393545ff5aa8ee48bb216fbb355bae5256e2ba011fc48e42a276257e9307a2ba35854fda523ef32b86d19fcb9b748d9fcab71cef8313028f196f2e33c3a597ae3ee5063a832a73e8c22bc559022fb7ea54c45e3739abe631dca275a0b195976a584547da0a24ded08c95fb5693da9702cbcd1e86a87c47dec12c7661b14b752cff2dfd681812f19e1406304003cd7c5f2012a4f36f4635e6f60d7d0f1eb3098501bb95638a7f55d5e9fb6b6892c7c83f26df0e4446a5c1b19452b28335b15469eaa71754e0cb49c414116d61286112cc4cd87fb96bef8313178eb503500e1906b72b8ddf8246e9739e7ee95d1244140ef2de817a8d53381551789260b7dd0e01ca5ba940c80cfa582ba787d80c8b86b5a281fce069fe86db7a452f624268aa52b8dee38ea72f4dc3630a16dbb8c39021ca5f8df746c2521c5608b0318dbe319a6b5c5fa9a09aabcfceffda1a79ddb894bc1eccb98ded56ce322476308ff666a3e58e51ed4c9f878fe1985743e2a1be7e405d3baab8b04ebce3d3ab97504d63f4d02cf6b79d5b1790c53130fc5bbb82edc5d954e5491bcbe60156646a093c7a53cf94bb402affcc9e9d0c79432f13e19963383351ed3e1e1109fdb28d6438ee04f655915901476ba97307f1e991457b35dc51aa09a5d48da7a7e6f7fc975effbb1e0ba5859c3516a9b97f8cc59354562e3a90570dd27da5c10e2882643882d8ddce779d113794b0c2f2834c23d55cd4646dbb8056b898e6186558910d1dc7c391871c41651f77286dd0e9256f86e28dc3d1dec56a025ed737ec78a93bb12f4e1932500947ff2dc9b70b476b4063f6a8db5a5a5737e790bcfd4e84307690d1b4be704a94d9d3133139c1e6911b64f9ace832a7b066766a7af352d3ca69b4823aea123e31893a4b9a63c7184ade247c2c830a8dc15e904edc8da0ad859d7062343dacc28c3546ea1ab3990059baff1a9a9ac8dcfd97221d8b841a22ef25fb51ed52d7d35234add12efb86024225e79545ae61cadc50793628eda3da2dfa5c5b08b47e903b52769006d9fc64c3145d026d2d7c2be221a3ab9c99459f596ae7723a36d0c4dc33d9fc28f8c428e415ec45fbe0ec5d190fff0dc38d07df6572ff8fe4ebfdafb41fdf6c159bae2a2f7267c1aa1d29e89ae3b663702bc506efb69f7e32474b834a28b33ec73b0f2dde3c3fe6479cefbde276a55947286825e5a76e0a4677673f5b73bc636634449eb2070ac60995636f8e778ec7281acb9aa98c093b8b1766424d6ab884c49f62f69fbccadf4bccceb5eebcc2223772d89d3db407904abc32c8358d4029a17e19e16710c78250acb19de35268e814d97ceb1e6df2fe046ec8bc207445f32d84220527e1e64187bcaa8bfec33808e2008a5efa025294774d50373151a28f3696fe575ff5ed1445657eb1de547d9b17dc3d4527ca27f0ebda407596474efb00385198262d13ef339f778409410a21c5814615f20004486161bdab78d772a979bb3bab8eb5c5044c0ee2d8a8bab7e3a86f087330778f46ecc33614082b9f08d32e0664b76b3b2403aa814d9af3305c6ffda4a8c44c7960428546f80e60f05dd2261783cb9bfd1ab100e24a878e48b8660a0fb21806443f373b1748aba0d6ed17a5177743101edb3674c49d7824ad4ba3f70ecca28ce1d74c8411a739415e39a91ebea9380d323ef94aa90b95db5498ac154b775e5d0878747477a5ae6f80d3b3a6ddec38076ec6e1e842fc53da5fa8bbeb8002f843baf283a49b43cbd3055336a4075d09f848fc53df8c31326289d0acfde5a324821c0b2346c2d502e1dfb8d56fed01572be2336e299fe8c2ceb77cfe3b2e8f665a134406f87df40dbf0d77545cefa12f843686b232c4793142eb942c5bbca5c27a41c90e6f7530246fcd6971f8066a6d8aee8d20a0461064e3a2e041505ae5057f7b01d8f0e8ab8cf7bf93480bdbd4d97dee2dc97f38d009961b2e02aeab8f9a47a10d8cd9a6d2836800fcbc28cd2a5bcb2a0e75da4c0080f33ef2fa891fbc90d39f9f6c738351bada247e118e07855c1bc130bec118a05b3bf35680046c01ac3d9c90f1534563b6bba089a964c24be9bea385061915aebab7e4d6e8480686b2f70a721c8d9e40e6c06eb85aba122945367e129d0ba6236349d7acfec50f5ff05a292c946e339cc0d2f1ddb713c274a1e863788feffe8f8d23e2a989a5afc4b0f1e4370220367d7c7e61f00762a70b1acdf3ca5de32e421b596f8806f14d606e3376d3bb218e79d6fc4daa6857fb00ba2530eb5b279abc85676613f3bab9151a9f0741a3fbb1741e0d6d95a585e100e6d94438ac3fb80411a4570d185458753a090aed0fb97bfb5fbd431b2e67a74493974812c3481d8a042d8f4f50b693c5dfbe77d4dd719a320eed32ffee4c2195de913abc7d1f1ac641a0c2466460fcf33d1596a6e9275c76668e3fc2a356cd3e954ed3a9c533245129bfccca0908352f7da95cb7c1cc7c173e99083b6ccd92864f6c3eef1f28bd2fe6d0338e77d9fd29712577b186496eaae8522457f11852eb118b1782fe44f8dbab4eaafb645c5fe17173d7056d1060de0520884449474613e7bb9b6cdc3ee829dab64b01f53dbf7a8332898cf66854dc398d4312be5a4b80ec1557ca0e42c2320d3f75834ec469c3f8e24b2f3b2cc123e64199fee786bc884435875052079c4c3e877e0d387b114c5305fb139f839d49e225f98dad6add4e95a5b6cd2d9e9949438084d61d6db32af406477787f0ff3fed7e14cfc5fe95af18684b2eb9145086ff5a24065c1f6f659a6ca1de895385c9e0d858f284bf88599d25711fc4f05eb4f0fb00bcdd55e8fb50788466a583f5e186516f31072b86ed3eade0e4c2e25e8c732346eac39d132589f5a1e29af2cd47aca801e37e4082c6018cc2dbe910c1586fb4657d6d8f1077b2174477f7f046c1f64fff2876bac2df553bb43d7270502f8f9f0d2283e3e1368e326f90cb45b85d9b878414ebf24bc4b5c8b8774016f2092efd949d7ac4f10e1317d24f22df44be386e3923cbb18ad3405e342365ca4ca30dc6b0c09a75f00d2df0ce19ec403e65216f7db546d2c310f6280c9a0783a36b32a47ce12227a4489d9a89e8cd035d188ef8101d434a9f44dca3f2cb51639d3b441f614c4c16d6705f6c31ebd3c1d9bf5e3e2e7869c3a9687c9643f0f7f59626afdeb33f462962e636f2c3efbe2e2a69af1c28e8b8498150159396abed22706039a86b00cca4913ab02d2b32d0c2cc184e38f61d86ada66f6b771349ac833b791552b29a9bb43ce30bb06abcc198547605fd03dc0a8223c8f87613157ba6fd0bdffdbc4b4768d4f25db65cdba78c50668ce8305694ef57373f8683c82f85a2532d3916fb1d53fd6ad12ea8b345912575ac369fd31777e5fef4659844c8bb905643fdb58e4c592d214ef012e12647364321820ece18002e0d032c183a6fff095b270647b91a9d17f1b8b054a281cd31daaffb5dccf32183acef0ddedc1aa0cf639f410b5dcc650b7e97890fdcbd4d6e1c095bfe7a94ba02f386bdeece10653094e5a7afe3d75a13f0033bcf46f5edf5d69e92bc47e004c3a8ff30ffb50af91ad0a442cc73b722cceac7495c71b3f9a5be60231f11678f0124b2a0ddbe17adcdbca571823e1fdcbc2b65cadf61a895fd3033a975434c6a8a15226e2c2955d52287abb3d393288887e7fc4d93d92034f4dfb25c4971eae6944992dc2b975ac7e268e909ed41e707f2c6e56b0c37d9482a1149013a56fc01fa1d3f56a0183567acd181b096d9a4880633f63f8edb7f905a65e1e2c255ac2077038e6ee9a0c817caac603af2ca197b4d6eafe33ffd86cab3b15669af9babdb4be7fbf1d8831a105d3f8575f1ee7073b8d8b20a56cb4d0c42bf492b62eeeb817a86f4526faac7f8767fa591f2ff57f5d9364c99271aba725d0e881794d37004b44725f781ff3265e56bd78a08db24ec64b6df199a708207b4f5546f63b70082b16395875423e2fe45431fdf6f25302933328afa167cd1ed30cd04dcb4ee0e863d7fb81ef9925ad9a7e40fc97698f31cae18efbe65fec1fd6663531415ae598ea63259ceb73559ee88eb29f6e111cdc41bf1c45b230fdb68fb3d9e2478e9fb14f4130a4e24f774cfc2b2ed45a46561b428d5c44ae22cc891631d36f42d396be403146be3c2f15388aabaacd6b3a0329a4a9e6a9ce394993e8ea11747dbd5bccff4cdba633e656c8341368e0fa81e66b74d266d9b8de9d5c9b86836a13d2c491bf85a2f97c066143b033645c4e2390fd83524f479cb8c3536ec27dab8020086048335d64175e3bfec8432b556a29522d9cf8b67bf183c803604b8f95e6f8914ce9d332a938be5cdbbd37c06e632866d4dec47fcfa9e600c5f61834fd9921941a8c2da9a0c0effef94b4c29f4e11427717f9af5d5736c195710a45ac58f3d8970a3cc6251c418fe7d15f9a6ffe95d702204890af9bdce22bb654d88e3e3905aa35b8f06a58a7404add9e8431b225ba7b8ea0bd6a21b32e413dc7911e7c66fcbc77e1e7396cc99a8abe14501e8ea1461ab0a598d1b17e062446434c00d4b1449b45ce8e6b7f73388cc11b01f5332227a1015f795553de4a50f08d355afd858798a7932b5e4fba77d3f132f6daf1bd2fd20bdad348d1b1ce2c997485975e2e348c2049f6206df987f2fe5b35af6ca6e50605582936925377dbea48243f05ddd5c77dc605b1fd1dcf14ce54fcf2966c7faf929020ca4c9f5aee492c4465c0f91a57621c2c6bb31928c2a7201a885acab7229b730878b2acbd06bb5dd2cb02251f18225b053d7c27d324db38dc46448c421bfe1c21fe3a6a82839c009d0f73343cce939b81285624cfbd2bbf32f9993c43cc4718da858a61635e12d1800a12062d1d1f935aa794bb027367249b5dc23851d6b82a8dfb5efdaceeceba81a09abf6f7af0e8b12bdb5194b54dcaa678be8207eef4402f106d64b0e64db9d94d1e4a7a4cc7a679ef458e3b8c9463aadf2d474d7d2ecebf0b814aba60f008d833cff4fb4c7e2e691a8517899e3f5d2333c90f0e92b2410324e764c81a76b6e063d7f093fa219f2718ce9644ecce892ed92e77c07dcbe0b743cf06e74c5ea6cf314c99ec8b939ee62bd56ec4851c13625576ba486e3f4a2e6e409defbda1c6d11d9094c6571a48ed7dbbed0bb8ca5ac4ad79f073ed0bfd0e85ca8e5287294337fa1f1db17765dc675cef89732c0c42618fba573fa059ff5f2cbfd7d62426d0a46578967d730d7035021eda9b36348552c9fdc9102070164cd8db004ec513aa50b65850c22c4338a482fadd3cb7126a6c11fcbefd2f451ac364885933376730dedb266d19cbbfd3074ed64781afb3a63894d791a332289486af1c98462b008222eff64f9b8811abbc75ef425189a9db299f51238571479ee856c2ea4b844a348526b42f98cb0930f6063882e0dfdfb885dcd31296d859adec27b84976db140ed8ed8b1b0103300171231b3945da9a085d574c03729beaa88db2acdaaf3b4ab16508a290ae688cab8476e2cdd50091b30f158f1e229bb66c026c8a937310170a755b9f15b4631077726740892e25e394b080e4e68192356bde8741b8de1a08d81863d16696e2ed33deb27d01004ce91fbc3fce42f7e3e08d3b84b77e65c7a148f0e673745d3e648a27df710d8bfb4cda48ff08f2a07157a5270c7d507bd1f87808a0ee1ad6706ae87ef685a8cd2ff8a0a5f001a0a8ec33d2efdec76d8df7bd989121d2c742c9d3e9bab77cbc08af645554fd8dae1567426d8a9ec2af4bbad90759b6606f7678cd7cd3888424c888157ad2dbcb743dabc9d4e1af7c694958dc6020822660315fb9393fe5fb20292ec48cfed3d0a282075663535ff748a0e128f7ec415f05353e56341b8d8a01bea5f1e41b6d4b268a921fe6a6c6512e1e542f05f8510dca58aae543ae58fcd95ec36cf89c673a68b23f5c73ec2d83badac08c87576254c8b63ae48c74e8362ca9f9b0940974a4bf9d54c11fe97a3ef4712632f05b7ac177600d84e709fd7abfaf2910fe3ab6d0d6e6616bfd1dcb5e5bbcbd84baef1ca96a243ca9c37e2aaf1c5f3fee11f7515dab3d55c06f7a4e6192fa68c758e9fc3ca43c4ee1a2c12215cf3bcad825214b5fef7806a3c63463cded8afb5d468f07cc34420fa1d7710dfb1de02ffe3aa16935795702f876da9952ab455a8f2345f1407dbbf6002b61cec274f9dfcab058ec772a8e51b0c1e65442e1d35e50f106dac1c17941d7e7b22757578cbbd92ca16cefc77906f4b0714cec2f94912e2f39314531af589c3e5a7a384d474b8e51deaf21d68fef0005e1e62ea784ebc03b825d5b0b99fdd9458cf81a7b69b9741129f5a1179c327454b8b33ea9f461cfea295557b687284b9f5cc240277217b195f9bfb7a2d682986d45086c462ec7c470a425ff25ae8cf95731b8cb1078cbd64ec70ff30d9b21bf13cf84ed3128373dcfca5b114aa93621b5bcfed92636e63c2758e38c300e23837a73b4ef256a2036ffa0b0e8cba943e012147351c994f35c42f8e259b5da2059d55ef7f9529210c7636ec8257069af8cd25e2f818d13c1c1ccb12b9b77350259462d3c2db78fb6d7e2357c7a0acafd29fb047f6b7e8814f7188096cf19a80c62be749121439c67545cb437659df22e5277b375243aa2000a9f606e5bf4a698d358a72518a58ff715db5cae9643fb159cb5f6bc98f8db12fc55b8e0a69302e0eacd9df65df6633a69b06d104ddcc5ba354e8b640dafee49d5f6d19cd8a33553b2c4f8282f7b7d058ad2dedb8430e95642afc4f11343b4e5a8c99a9f5621373e79238780f2db74bfb15eb6f2baa6a73f0178fa9276f8fdfd056a6fcbd6050f69eee1e506ed97018593bfa254c223f7a8ece4ac4dccc38cfcbb29a81d9d33c9e511f76d8561d8b3afc7ea0dfc1b81f783fe07eee383760cf45bad86b1e8535034dc074939f4aa4becc440ebcf19234ef624a461f61ede7f0b5cd566aadacecdc60f2bbdc69a96b75e1dedc630ecabf59785e9ac9753bd0520851f21812d20178b0a2ae02082ebd78bb97e46b452cb354d0bf842f5fb9eb23870ec23243e101d825f41ebee80a91b8c95a79070ecc951943ef84de7763d38208871564c926d9d152da8d5b85b222a248eb1233d1c421b753d67655d86b00000000000000000000000000000400a08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205eda0d12a94f45cf9e0524069746acd51ab53a3190ea520bbcaf58329ab5c8626b4a0d8ef096f4f07a57163bdd28728f2a5d4689cd8c7089618223f567e30a95e33a017063ae510ba37ff55f5fa533c6d953a33f6a5252feaa35d45b260dda8c5f2a0614182bec524d648f7dba8a686037f0f535f52f6d3a3b031ef6fc78bc4c641a0b90da4145ddddbe57b2839bf4ca2ab966a5c1e66db2eb977f26c90621bc182a04b9478848e85771d9d678f2d80966bf9ea36f0d05b44fe5b23433e14a3eacea0c8f25bc8a9a8921c756c41f47efceb12c8129a9569456f7d80955bfcec2dcfa0ee56523ea5289f05709c26f6be6b08f364a3f2d6a5a8b6cfb2839bfe6a506ea033f85ad8f739b519c0dbe4d77f57e767b36788cf03998049dded2d1a4b7d6ba0a7d6bc965ac7e2381340ce3346f417d50ba847dd26d34e950ead888b0bafe9a0e9fff921c59baf3a338519c76594c23bc4565758e960cf151d87b482f4bba1a029805aa91d6c22700a282d4db02681bc8e20f0a81ae3b4c6f36e3b1ebbb752a05e54ac3bb6346595b32be067b747897ee57d8fe6ad2480e598470fbdabe05ca06041f70e2182cde513add756fd55eba47aa4b7a3f4c365433fe35dfcbe83dba0bf997491e33947e8a18cd49cb5962b37112d1ce4d59cb3838090e1904ef5cfa076fc3e6a59e9eb09aa966ffde6b47067114dc9f5b55612783057697665396da04cfe09508be06f943dd1f382ed765cdafa322d3f37b5fecb98463acf1701afa02c987814534c6a32c39dc304074a9a9dbd70db722e336a4845b63337b8c96ba078721873ff5b4c1f52390fde8aa3aa24c9c17ac3b4a07a421d7b383eea19d9a0d7b77efd598076ed015eefb86b7f37938050449c74b8a2cdfb87431737d6a1a003f84d0bb25f6985179e652525ab900d7d2785fccd61ce74dd1c58d6692168a017b5ea834e7e12823747775e2010c6e08da24579c57ebd8545f88505e20a31a094e8cb0fa36e2a6e229ad8beac1d0b0ca30ff8a3e7bd79f44b04b6d8ce2cf5a0b0001ba9bea9647e7e55f3f84f8c047e7b67e30ae150478a1aff3194e6ee4fa03b7900f18ceb190b000d4958d4fd03c8889a37286bb339c00f9746e5a07e0fa0ebdb2496d8304acbe801dac3389dfb4be752d55b4220823b2f6d1183d3b10ca03a2eb3d755702b7c22c5e4a633d84fa2887c6cd474b46c5ab41f970d299631a05e6415801a5abd19c7354277a087794cdeff569e056a7f75cda532e57d3c03a0d3a3dbdebd861a0d43978a6bb4069b2d5137c5331de89e15a523a60f9a10c2a01345cab81374ffe2348c1109c9483b3f3e9ef66527164795b82bf70974c698a00c2bd64e1a59365560191b001562c0b15d79ef429b54910442e84813c09987a0c216713f5288634acb0946aa6ef4cb46939c1597c33d0bded63b8f0b7f0fbda014691281679b9e5fd50283efe154da5699f621bab0d72f1f047489002c4de2a0d9df5121186cddc88b79107fdc32d06d742c1c20ac2fda73ffacbb9b684b69a0876375fef4ca736f53dbac80e95d9dabb29b2127934f32ed595a2d5cc45554a0f235f0e55e9168ed3f5ab62a3631980a804f272f25c4efc8d04c991be67d20a04b96dc845966a219924fae01a0872f9834b872b039b7a21dc67fe0c775e87da0bd11febb0d6bd6ccdaee17b3bda85905750430ec21947fa46cc1398a768084a0a6262972d99ecd88cf29381a1bd4cce4ac51e7cffc12c3c3426565365326c7a0032d28e2e269b649da22e627030ff8234f3600a938046201df65a00475aa6ea018f62f95e740c8e600cee0f51025fc25c7e8eda23b406b17602c6fc9e5bc09a0bd2562d9243aaa06ad6fe6b6768b9d6cae1437f9af3eab5ac4f0f3b50e39c8a0f28d1b9dbfbe9a476b97151b0dbed94017f0583824b62eb4e93b39fd558ba8a0007b618e3a879710ead7fbff6076905e4456bf13f4371ae0abf084fccc7bb2a03a47416fba346b982635fafcfd87d74603215b57dce7df768bc01a68c47ddea01e309bdaceef3eafa80da17a8db27a6e6a2ebf37f64f30475f76a40c26579da06744515155075ba8ad2b0038cd1d7fb2f3796001b99d58efa26f8b28434636a005b251720fdec5deb475c4fcf4b5fd59c24f135e62d35ed50a02630cbab0afa0a3b467796a1b6f7c46ffade9ec7772f37037d90ed73d2714ff3f1832b03052a01437e81c94c57946ac900e9362b41086fe5e183f0eac00487f21a928a021efa0722ae55494e150738d194c54ef116a3bb391bfc59302e2e2da7a1db2a5bb6ba0bd23c59423def9080b4877b1903a7088c0549b908616219ac365b11a429aa1a0a972a2c5242e8fc9e3cb31ff26e6c6d6f443c26657f8382ab57521389ed2c7a005123ef1bbbc44703ca02bf550b0cc51a72528c0a879e43515f9320f52ee72a04874837255658c1fea128b752ac7e6fc42273f3b89a66b874ee248940d2c72a0e0ecb499900901abe08702f55817c1da77600ae0bf5ab59e3b946fe2da9a10a0d82023a0dc24cc404f143cac22e9d8ccfb5a604d1eb132621ed6d148b8be56a060aedfaac593ec71c0c8ace039f2e497fc942e9da35e6b50c4b61cee1c0d05a0cc382761be710067066de91a2116c235ee74653402d7bc49d38119f197e2e7a027742e426b4090478fff78379ae033a272080d5e649bcc36779866e31bf6f1a0a736ac08c2fa7d19383558d5302d666a127a4ca8c09e95dd52906414c919c1a0c8555f914014060b9c647e78478dc57af495275721ae01dd1cffac9e433302a0040fc74bc1e9ec74dec090aba9c846291ad72e18d4dccc850083745942c328a0fc32bc85c963eb3167666e205104facedd24cd9e0f15fc214613083ee1db27a0a85af7b37cc03e997eca7306bd1bfb6effed64ed3ce68395ee16226cba28f9a0e7eede7a701c87ffc9ccd3b1196ecbcd5a7f123123e3da2a07bd343d056af0a0f704d055d7838a6b8f0038f6aeb079f690fb19697d0733ca45bf42e1ab0ba2a097ee9a5daf69e55f4638e151a4834481d8d16ef20242ea238b96cac8281ed6a09124523803fb43d7ca364dd67319c7e43e16ffae6b464ca08ac883a72938fba0b58504e2fb59d95afc698a56b98d37433956f7963dccaf374bdcfa35cb2926a015557d75e14739d9ab98c6690ced3a4065097241e1b2eec6a554a86002be21a0fdb7c3448caa1607d98ab6aaec092e82b42101517972a8a65a4e8300734597a098483ae4208ff60baf5573acae68a57b21193a383326219fadc14b94dec9ada0a195b8d16c6abd62a929bf3f2c5f2f2cc0eaf3dff6b10184814cf64ad7ff3ea08f0456cc0851204b60d67a7d50bfd38414d75614f7ae7d418b242f372d3264a0db09758d7bccc348d2a1911ecd50ff841454ce5ef25ffca17977e06c139c67a01765fd7a779296bb0ca1ed777dbd889f902dd55aa0b3c48d7ee92660d33ef9a0d7ba2bdd3cc1fc214c7197219ee7d80ee1d46e772ebf717ca2c757ee07d344a008d864f66f3d92c89b48b1eb17f15bfa20f0e241436aa6dcfc64f242a728efa0c057e72d26d5f4294f9879c1a6b0dfcf29920a70cba3d8209286f4b393d547a0d9c77a075974a0f124e5dd8e5ca1cf94c1f46f5a3c4dc723c7089922ec8780a05d4f7f961633ba4a0a99826d94f5881e7fad4900cf144e7f425daa8cde4f63a0a0f78a5b1dcbfa6d862b48c7242755376435da2a205894f59adcd9649813b4a0654adee5b071668d10f4ee96dbf65d18398c09bcf3de97d6183586386a45e5a0d3f58dd8424944773d96c0dce19edd0ba7e725a385b1783c63f618600591c9a082223a7cfd85b6bd5c2d36b67993a2eb7ddaf72bcc3ae02dd910f61ba5830ba0d96173974b0c7a94cdc39faf2048b5a0962db9c50bfb1b6cc0d91f37fac005a0e3e236c238fdb8dc31fcea806ee8f67b8373b4ae03cf42604f2f5fb797bceca0a46e080637d1816547832479837ca5b582531d1244f70efdc5668a3ede34b6a0cb0d40b79aced22c56921c47aa3a9f7c4ccd612e67eb2dda4d846de3e76733a0612d11f0056e6a682a1823e486a02e43bf54019e491c5b08a27a3b124dba46a08a6ff0369b8ac861e9a5fec2a67a6694365734740775900e4f22e7854f27ffa0a737c1bc2ee286ef088c3a997c2b8d895b9503c2489803e3f9ed36325579d7a07b02205a9c633791d1c1ff550c1eb0aeed8ad92ed2b84a7f89f621a7998d5ea0067c0322e1be9e7f41684f286dbd8317f8f0e4bd1ebe25c97da2b600abbde9a023ea8174901e89aae2d329786c50e441bf719180ecb03402d09b633ecad77ca0e333f1990ebf0433dbf69cd0a308aa572be47d906dc112a0cfaa6ceee2c726a0c66d44101a47e7a5b8879f20bf828f14a3e42579978cb298950312c0a695f6a0559d41caef4b63be4e8cab9b490456b1180fcccc7117d569369d4b70c5f788a0ce57638c7d65caca0804bfcb7264b881c117312ce594e48a56be75652172b7a07ce28514f0da5810857021942d7dd65fd03dc8c8beefafd54ac4b5e1fac814a06d99c1c1304b250f0f7cfaebd3858f9f56f966176808df7b5d8339ebab692aa056f2da3756559f1f1153f5dfe3cfcdd64d53a7a4f4debd7b305bd689704f65a049ece30c64b30000ed5235b1f8c5478981e4dda26d351e2567c7189c1e1d93a0d2b7932b8f97cb352fb2638b7c2377ea11bfb415e78ed16686b266e135b8d0a0014e2b891b4cbf29b1238c641758b2e8aca7573c54c464d7454d80cc3d28e5a00bfb844656be03fe65f852e3ba244e5258686083480414f7a6dc697f8bc9b2a0bfc2c79b35b3e6ffe20387b4f96dcb5023888cfa9318725e15409327e6f220a0116d0b25ba2c4f56c1edda896f0846507fcb00572e340fba9f2e0eb0b92eb1a0bf0e9eb7718404b14c741118f8a35f56992245ff199ff6f91541bd0d921a9ca07937b196fa670656eb19fc62afb2cc43687ecd834ab1155d1ff9ac40ecce0ba01a3320da193030f3707f0d76e4b622931335ee23a24a8f61fab8bf05878c73a07828b4aab15c7107834f945a94fdd5f98c09e716698cde95dc6e65996a003da0a94e396f087408392ee8ece298f740a4970baf2c41840a33ed37e1144ebdada073ea62321a085cf0333f06b92ab965a50d3c97313ff09825d3f63edf7b9f6ea0c48d300d414690c3ebb851ed7e539230933247af7c9bf140c76c3a4fb8c102a003138ff8e3de80298df73201232a3901f93b90e2feacdeb8d3a1011b0deaaba0d81b83a157a601265e923ff63aefaa1b78c9de58472b4c41beb74d9986f50da0a8b7d6c0ded5d99b8604a6b25a0c0b7e3801a02c82a5f70f2c5e2bdac6d61ca0768e6992936f17d6342235a99e795ab40f9615978e1792fbd8b50d920400aea062b219393494d99a1dd2f871515bc71c575f86d9fd722d024b8ae61f689a58a0565a446d072e34dfb510190d5ff05f5c43e48f8fff456018233c540eb90d85a04a141d919d4e292f33788209fc8312a463c2a5032e86a39f39b5ff8615a4e4a05fd87dead419cb235a935fddd589ade18a9b7461049e2274010dff5ed4bdfea05c48bc1f5c36749713b0b26e75359c0ba65fa645c8f213d7b5d114f620c672a0613ad339df17ceea56b25e9f0fede25417ffd6d18ba5b8e36aa62e62ff70a7a05e9563bae3d605a569af4d4f75cb5ad405802933d2244f81302875338382b8a044881ad81ec9b567b514a953d873bf69eb74b431c1bf159c12569b6c8dd921a08281dc2320ca3cb0273d46626d0e03ec25b2080fed330631b4ef4b36b6447ba0ec475b1d01f35d70b62a32a9a861396311c7234928fbe3d3893329fe7aa780a0e98af20ae7d563064d90d94eebb8b939a940e0afe51f0a1cd74326322db457a09bb0130611b48a7834a5289e6642655e053e88c26b79f5fe59018604f4b3d5a0b9ef54bbcbe7895758247f521267c46a4ceb2e78b71cf5c4c8eb99dba90beba01bed9f52d564fb846ea269b92782a4c005ebe526f12ccea6ca6fcbe0197717a0bd6c6249c2a1b5885c81930ca7f170d50dbf2a528c1929d12d832aecccc371a089f504728d11b8211924bb8e9a914e896b377bea34544a5e72d04d1e668078a02d2968dbd7cecc26af0cf80a4c29a76108944bf5f37b0b73bc272e09c9ab34a04e19e07c067be348424cb5132b34ffad6785f18f907ed6876e3c804f6e5ec9a035a7dbaeec86c03d08fe27b91b19c4bc0bdf9dfa4fbe9ad649c658c3f52f27a0980c3e1519ad69c2737f868f1ae8025501dae9d44c85d00507e3cd8eec5400a0b879ccdb23a5ed1d5cf5aef29bf3843d246adc61eece49b8f5863d92cc5a45a0ce1c79c0ce41f4bea25bf81f666bdee2e4c3184296807b084568f036c1f763a091d642e7cc3cf97efb22b1f977cb2f9577ee652eefe26e8ab29e704345c82ba00f39c87b6a94813b643b20627af12d6dd151f9e3cfe7161258311c9a96fc5da0913a8759ec6dfdfc517f7bb15b171a340c5fb27f53d61b877c333c38255928a0699b72161fd15cd89f4bc0753276840a75c9ccd6bad01f37f025e11c21ee31a0cba3aa95c148c58c01c9860a6cff4dca95be38f9c6ea269734089c35983296a0799665a7122f32d7dbead5bab3c1bec3ecc7ab093fe38f2b910ee92a4a30afa0e0874e589ece74cac09b9ea36a2db8dca6d098852ce9ecad5316f9cbe96dc1a0b445e4d717c9577064c03964479c5e67daef9694f610a84f682e4c41a714d2a048eca718cd7dc0d66de2f7e528c5921b40b3d5bd5158b7008147ac462a7ef8a0e62f3bc7d8630245b0cbe82092f176bccbfd76794b5fc36882faa78448da60a09ec844c96ccbbc17b509f8289ef83476ec37641a9a360614dfc57b8649c58ca02aca0a5ea5687451174889efb52dd9249f9c06b4450c00d6a12c58b5a9ff74a0887516be788e76f0e8d7badc1b8da8f14b71bc27c748149ad1fe3ac8227648a09922171f1dafd965ec66915a3dad50132832e14023d425ed3a0567622386d6a075f4361035b930e9110b7f8dc1a70f100190c0fd80704fe7e0c29f5ca5368aa0835bd9e62de2cab9e27e697b4fd5e410a864b2b07dbc450ed02e98d8abd8aca029366743906f406b5aeb8268dc673c80ad3b15e2505718a2ce3c7aa0d9431ba0d01eafef9223b01c248d37ff4571696f1c35469caf6d10a90fe1e809b7eb47a0fd90719120d5064b85fd2ecd8e65a7b6122f1080906d68aeaf0de677bf75d7a0a3b1968077535c8f753e6644ebab21563eb7306bcbab43ad84ad1f84054a95a03d0bbf320107c760ab8c8b0f890116c215ae07088b7ffd112cf83ced56c5c0a0e8175663ef51863b1b8b6d1913c616ae97c807da1ca13d358511734120ea07a024cc52277b23b18f58557d030862343a23822876eb8cfcd66e5dcc44004cb2a01ea56935572aab5dcd4e327d69ea2444f9bc9f01caf25ed7646387376385afa0d740c162c7ee18c89105556fc56177d9f3e812506f027afaea18658f665ba4a002d37201af85bbaf75bfdd1b5da4bba0f5f6ed1c704932c00c3582bbfb9b3fa075664f95fb2871c26e75f62728b270ca42bce4a48dcfb949e177b82dec8f73a0df7037a3eb3e3bc1383d67bfe3d74a81da3b74d4ae40045679ca63fb11f6d4a0fcc3b2acdc9d363dedf1b93215477f4da5b8e0e5e47bb054c5c2a1d4d7a031a01b99698546e69e9a500a30618a1662b0c68dcaed2112953ee5741ad7d0cf81a0aa2984d8b5f75ca02176f24574a7b231c59e2636863527648e07bc5b484b8fa03c13cd7ef412d80ce3f3160367ed83ac7376a58d55e25463d8a8f91b338800a0718ca6d7a4540b1de6441e5e12ff91547377b423c3971b50c959f1c4351089a0ff8f885bb653a1db3e09a3c209438c0caa5269ef9d1ebe226641866f574613a0472899c7ec3dab6102e72fb370a9ae4a2004821b35c8750636e90958c42634a0a6e1935db52827500dac657614c8cf8e1fc4201075cf1cfaf5d60c93f67b5ea0ec51146c68d2eb3a739aeeba429886a18d1c315e29e4afe570afadd0d1e726a00e4020a9c5187635fa53866a4bed37c712aab394927776b80298d71251b705a0f7009e1ae730aba74445f50bc519b9cfbe177237ca8f0b905616c9e0e044e1a0135ad43d01d4e4b4367e79dc0d09e3ba34324ce14bfb693715c650bb09d95aa0a5d963eb1f4ac4d5ae310c885fa7dc5c1d5216a6c69db2e5d5a68111de82cfa0318aca9435e98094466dad4a2de4721293dccd6a7efb15446a855db131d7d4a04ca80210e5f750c2c992b152f7cddf52988327d0b5290d5de09d8f0660e82da017a3c0c87b1621ccab1a3741b1fa5addfe1a5adbd3985a0c588d963991cf87a09ed59cc3d26f66228bc1c2e8e0c45b6658de2e792cb11ad5d12e3964aeff62a0bf2c8e287fbec8c01693b1d00d5dfbb8b298b34c9571f71d586dd2b651f1cba0eb2ab1909675b40005921f457996ff5e90cb8609904140afd9f34f84336479a004e5dccb05dba7b4378e0aa98575bcd2439ba47847ed95ce1aa0a197be837ca0c92aae1cd1ca580580958ec283072c43bdc0d154a500cfb82c2afeca0a8e69a00a95eacfcc9968ef1e29f3a2aec300dee8b3cf6520c28b0cc14bbdaf44bd55a03a9e7be0ef5a8ace4bb983de6ff63d97541cfdd4cd16fbf09c101343764cb3a0026205da9c530506c436c2e35d0ddb5b791e54dbeadbcd69e9aa6bdf60ddd7a0739d167da0b61f7fb7f8e1653b6c97a3a711be3a4b2e0fb0dc59a591bd77b2a034aba6ac8568d0aaa5c330ee791d097c4c7de7ce97b559e208dff77e876e86a0475f0b079685b6ad56dcfe0fea869344e7b4a4a074c44d7c7fda76a6a2761fa04f36726a4a98832d6b9342d7fcc0f111c61e889a19b68d634238e25bb4fea4a06ffbab782d21a92e87e7b30dddac43dcd000e2a9d1e98f010c2cd080c699e4a08cb8a013ffe0be0c6256beb308fca3b86f6a151871d6d8fc5c6bb206ccee2ca0987e8c3ececbab561ad70bbbb71449b2cf4b85fa387e0766165635d0347901a0da2513df6965cea9013def00198accaff49304a7d04fa29a72ada835fa7625a0453277fb535fdcd847f69162989992fe19375d81fdf18f548bd31703df76e2a0db5ec5d5daae6c943a2d2c47de300fe20505ea4ca91fd914e6300b380a9d5aa03b1fcceb41bc9cd2ba8f443e1a71b5c75456849106a040b38418f0ce3b64cba0b07feb2c88ee559b61a1dcc99b3059c41de4f5a24123e2b49049c9946d5853a04abd6228cb97ece24f8ca8f5161a81c1fb21c62a7d9c0f69703cb4bd754e94a0e4784a73ef579abbe922dfa3fd6e39acfda3ffc30df0cb1cbccaf1c36a591ba075b1e75bd96fea235970908f8c2489d11af98ea3cb9751d2d53845bbec65c3a0c9d13650def497f50bc256e029abf903406eebeaf9172fd61c75d9a0b49e7ca03621f64e31cdcc88fcb720715639abfbb1d61bf225ffda6ff6803fef102f30a066d4932c915562b758caea0687d295ec8262e43ab72c5f8816f8426f139da6a0fa8053cbe61553b6fc71c8fc2266f70b74470222302894ca3332218179e54fa02b16d2df6dd83bedad3af288d26595a86b21ad8ccc62d5346ef430879c869da019f5a926bd6c8ace5b7897efcba472e23e11371af1d979f25b3e947713364ea0a1db71ae1fe22e93f19961aea8ceb57413027bb56b4c18380e028cb8179ff3a05eb4c1de32ae7a12751d73be6431b73c6e7ec723d7bac778f795e287ed1580a08ec6bb4320f357f3488af4ad6481d944b3745caa19a2e75217b796b6418481a0df00e4fe6c3efeac54d0c5842e5b4d7b1edf0c04cdc867ac35d5b15bbabcf3a057231fa457257eda28885e048744854008397915566dd614edeb1e226fc704a08fae5edce9b8d6561412ce459671ac6ea8f573c4641f753124bd153a21f980a081456dda48b8518075765407d944221ba27bed5811839d9c6ac684ce0df7c7a0eedf1456d418ff9cdb141d68a60628309a6ffbd3d52147131524701d4a804ba0150f5b09d951831230553e8137fe187d5a524dd706c0950d3101800b62ba27a06ecf5b40dc2521a713eb860368dc45bd7cebb744c3936a47854e7b7db3bc38a045aecaf11e92ce3d95c67216de3afbf3cc5bb3802b000b2470d6913abd683aa0920d8c0c9df11d2787fa132af9318257393e2aeae69935b82948f038f72c44a0d9ddf947ad7b5757ad797f18b4a0cefd216718d99bff6f3d38566ecacb9b4ca0b73a3a92526417fca0d94384290982bc0858deee7675e299c852ebb996442da06c8c2fe9484087aa485175658259c37f182b9b7e11a3c9959c8bdc9da0371da0946fc3aed690a8fb647101d99cbac8bb338410970f94a513eb73596fe659b6a0ad1cfd5304ce2058dcbaac9d9a04cf24c5371533b69c389254f38c05821c28a0ba4a43592ad54cdf14704f9991300e21b506a560f3a9d3032170b312710ed5a0b1e1f605d635dfa943dfecedc6d94fe06fde54037edd612dae9554a5b4dd79a01ac0ad4746e4df54ca198fd9ac8ed667c32fe099a87831e3834b9693a8c181a0ec47218f74a49a2111988cb1b005e5b57564d185d29ece3687a3cae456d64aa0775cd041aad26a6a0a5c59387da12e79855301c2cc7a49adc7a210afada571a07c403db15e7dd2abba3b4a28fd922e0a47cc8ee84ce30130b29835540196e0a045c3a9c828b3b81f833f5941c3fc4022e31299459a090ef731bd727fc1ac70a0e7f5235bcfe5a9622e9597951a2500da3775f7c7a018316066c7eedef64701a032e30ffecd1552f72fc8767de934a4a59b40f26ab40d0ac56c5f8a6aba863aa00f61f21fd3b3c16c7c2a19dbde696b849677c435909f0247a2f4043d18026ca0d4360e1bb469dd6c85560dc45db79a942fefc6eb160cb6a3796214f0a9c0c6a0a51c0d6a3875ad259bebd70ed7f7e3c1714b6e2c74e5229d78d97c85cca77aa08badbc41a3321c1a920d13ceb9de83f0e5a1f9c6af97dfa10fbdd6196b83b0a014d297270b6dc1bb3de52e1d75ef9c2a0d0330b469822a19ee3306e0f71b90a0566e9308a0797c3aae0af01fd60bd99536fe4600325ce2b6101c927d46fe5ba0d70022d698c4d730807308ab5fbf8ca731ca4fbc4e0273930a3bf04ab134a8a0eb6296187e0264002c3e531e9dcefabf7d8ddbc55d253ac258c3a5ec8c7983a0f432b4063dab37266ade4d8d0ee3ad1e81ad456055b210a30cb23e73b1742aa02195d376cdac02f24d147611b44279d3e3716bf783da42dcf3193f22171cc5a0f9d2a93b035c0480612cd839d88c355e134f847d0417256708b0fa8403c652a0781467ab83bfe302cbf6fc06248b7fd57ad9f8c75a87f9f07d6bfab2dee106a0d8dc15316f47b9d45f427489587107769d69c2f9d93bd80663c0b93ce6aafca0b2628722931c5860e77180a2ba2d302cb0367f1532b98bb413e6efaf984a79a0d74b0d97d5b48c264c962012b33f1beff9d75508c1c7ffdd2b1e9c8cfb6da8a048ae69a00b00d1eb6358fb88a7f40e5c1e99b9251fa030c9ecc7ba59e7da7da052c1113371ef4020df0cc85bbd456c7f25e6a742f27d704d8b34a01525fd4da0b2d21d72884f6cb430116b26180a07cc89b4fa9f1900b6e90f55d3f20e4a14a0008d0684f2c02c688d394c9ec79dbbf2e8119e57f37bcac44fe10ba0d0fd5fa065f38b5d3f419b93f6024b25af8e6459f381536f6a7145f3904ded80f38dc1a0aa44837033ac1b53f27d0286f1041d7b7edbb22efb05db23e71b092850bf7fa068462342fd8eef1755758632950a3eabd1c4a22d8474b91437c41b0ccd0b1ea017b8bde35835245765ab35f4e252b3169463a64524a8156e97bb22a2a2fd6ea0692c7cd8d21175b007868b8d5bffc4518a0a776631c2e983a71d0261ddd2c2a012d0f5c9b979725e5539b2779a718367d61744c51b6a64d42b3ea34f999676a04c773ddcd920ca9b5b187c9e4963e940f4bef01f2c00d35353a194244c56e2a09dfd97b5b86019b001cdf3e84bb5788b2e56f59258faf42122d8e519b709bfa05d0eb709ab436020ad82bb733ea48641a6b8409010c7efe25604dacf64eed3a0794f4bcbc66ab7e9f305183cb3630786bd53733b57295f253567d8b0b3afd1a045eb95e774942f0128b1b2c25788de3ac8f02d56c6e4162a57a8da9f9dd7a0a0a2a3edc93a1961a7aaf735e180949ac1f4189e01febf9bbf8ed85d6d26cd34a03e2424813d1d3351c337350b23c596e45bc3237dd3579c414a62f209344194a0bc182570e85e8e152024cfdb70aabb8d526c8450dc93a789b10eb13f57976aa0f2c6d5e5d3f57ae7ddd83843b410469763eb5ab9cf8707a7eb5771b39b5c2ea0d491fa8217f6e4d6b26f42fc1242cf0b916b2cd44eec95d1a1082e3c477d07a0c0c8deb3b0c7d4c391d7fba385d0d75521976822cea437d1ee2837cbd44093a0f9706434dc2dbb4a1dcc6ce357f901fc57c442699ec9ae0c5035db15e864aba067d87b69d43c109ccd5a2858bbae5ada1085a7bdcdb089ad4beab5767e1fe2a00340793658f00ec2bfc51ea01046458f21bc079df619d2342e52beee41f79ba0301cf773e72d3b79b1463f274e1dddfa357a2b87fca8ab6e919182f8156e55a0574f5736d015995af6125f090e152a43d4105395247ae4001c7f264f37a33ea0b207f98430b3f90d11ddf2dc99e8c958f14066aecc6aec7c8c55ff260ec6d5a006dd1442000efef716b0aecf328976c786d7dacbd93a9396f608d1ecb36246a01ef82a5788b6dd1d78f0428ed49bce21f3449aa65abd86f6c9761156ef4b13a07774410590a3a6a9b60efd77e8c8f9ec4a164c9a56ca7808d525982da8eb3ba0cade5a937b8eda754e4d6f5f874241dc2c1fbf48b18006a9787a1d2e56b2c5a000dbeb2cfbb1689c47405f859e874c9f50c9a91c85a54ca3da4ad3e76c21c3a0024f19e2ae5b378f9846fae1790eabea5032ebf9fdb7d62fcdeceea8e453bea09599c1b8b446c728f104c1f6b090178bf9b66099f51221c52780ab3905c2d3a0b7c75202ace156fb7d176566148c206e2d0e3f5bb979b6e7eada4dd8236e62a0c1ee82c6eccf47ca63b57b15f4eb69f3ea8de90e394158f2cafa70229ee7b1a096bbf661dd2659bf7e2b95a1520471ff016cc329408832621c640ccc7f33f5a063c9e9c4e346a636f5a3d460b031dfe4373994d44ecfa46638555d7be904dea0517096350604310c0635de80c27ce07db466b9f1b96e9b14e8a7e0d03b4a54a0a5cea445142c0490a79e1207951b6622c0e7cf6e4a9e598b1c56eb2939cc2da0e704127e28589d5d70aa1bb56943d77ef8ca32823c10fe90c8d9314b50469fa0eb4f15a33b728ea4153a968f43c8c90bfc3c0a26c80607ded7794a16acfe38a0855657779925d2146ca19b0e9aee517651dfb474760892b4085a0bf22ebc75a087e08d71d4d03ad1a47f7635a5a15900518b4c15a17373107ee4f5d257c63ca017cd3f1a70c743340b7c65d37b8c013931c3457de6048948543ce52b7f2c9da00fa527692968893664f379213dd77dc9913c2862af5df89778c39c17cd8088a03f0a93b8485b09e86a6542d322a4cead1522349e5556d89913fdc09f6b6dd8a0c073ed922080efc25131d27240a93d8e5e8d72f53ebe274eacad5afae3a6c4a035f520792861c395217c3584265b8f61a05101c5a92d242943c2292c2b2886a036ab679b95442069f6949c58913faa4e0b79aae45ae68863378063fad6d338a043bccfd593cf7972885f96c17dd367a1b4aa42fb2324a62e65882761e5690fa04aa470ad732a350d286b63d720310eef626d9d69f1768cdf5c1445f399f58da016ad3f36f2079607793d324ffd9484bd582674dbe251934663ddde3b156e96a0fd34b76b4b0041d91a8edc77c51987bbf9d7fd578acf3f5a934c35634a687fa046375c6dca5f9c738f59e14b4b766e119528965714fe4dac3c4b12d0340ed7a051c66326440028dfafbddb6ed6c34154df698db24150a470710560df219888a07fbeaf8151ee39b06016bc4c44084447e807daf60194eb6584738a13818f50a0ea2eff4a7a358b7a42f006d4813c627a58e9406ca681724d1464a0ffadfcb5a097f2b97f97049683db9620b1586e8606074080b8d0b43a29d40db07d77d359a0c4de2a489a48821de7ff9bad5ce58490f0c519a44cff38049dea4e98b04581a074e9aa12f4740dae7f2b0824b29c654e373e19a2ddebf29db5b9ef9daed64fa09ad97486281fe5a13636209b3e99c59c1a3a9bfce07461036cfc8a51e0f435a0faecee4b6000e643d123386f0e4c674206cc84a2bf4af716a0aadeb227d736a0d232bf46a1c30808ad428379c97ff0ee095982030e5575af78412510f2a360a0fb4e624beed317d35e378256644bb562d415455bf8874f3d48aade8bb014f5a0225b51633192d4a6247a9761f3f859ca77c2fa0431621d98337c2014acf196a0370c583a55fddfb082a6437e3bb2e105f48e03f0c5c98dd201512a48ebb7a0a0c2ec6b7aa077726ed2376fbeb6a0f7f7c97a062278e8086dc376682861a672a0ef2b88c2eb017d99e2bcab5cbf759534f120f51e76577619c5c245df344645a0e68fc0ae0e51678933639656bdcd3c18fc44283636826dfe7706194fd90601a0a98de2c63130746d335e9f0d513922f0cd33e5ed06e0ce054395ac316a01a0a01b3f86cf5f684d20433dbdc2022dea9d1fb9e522407378b5559059ea90b8b3a07bfb34615ee15e568612c7717f018eeac0ff611df29608a02d8ecaff67c7cea08468db824a6d01721099803a15d1716ffeb58f0b4860fca7b29c13461ff871a0a37f7fe78f79b59c02496fb8293e88ff5235a00b64b40c380dadf3dc8dfd63a0edd9b3388db0e130f46f8cd5eb5c34a2600ac6ae9bdf0b2335fc71c7284617a0128131198ef3f889a98063c6d38d7985c1f5224a1a866b917799d17f500294a037b5ea45b03367b9d02b4f7c3b2879d34c65a2533d97c8c1372cb63d79c93da002476bb38ded136d82a51890ad7a643ac6b89c8d3460e34b369bfd7baae12ca0bd7d9d549ef84b6daf1790c9878cd0eef031dda3d45680e09c72d7dc213b56a0f5ba997a04bec119c8ebd994ec7452598527e88eb0c647d8f8a808a9edc39fa04ed0a65c65760e5b486fe3140c923e22d760a37f9688dca20f00bf328b4ab2a0f7c1d0a8515816c8a533f174ba5bb15f6e7c5283f0c805f0083a0f36123274a0bc0d4827a23b548cd97e223557da698062149cabdada96ef8ca0c8676d561da02652d4596257630d91786aa080d393ac0adc1f973737e6b208710785e64526a0bd03d7dae5153d92d476231eddd2feb9835074ecda9c20a302eabd65d2fbe9a068e6ff97f7ea76725dc5e66b0d5b1dec4f30a6edfd656e1c11847a230a6e57a0f42c28a77ac6bcec6384728e9d7553f148e17354cdf4c2ec2ce3e8307e0350a01168db556471a1a9d30e0a6b3c5ba906f0eac250d1c8b4a7f3346073071d87a037d0dd253e2c8c871bc84e1f5d078f9ec4d8d21842e27cd8d6b2fd62f5399aa0a9039f5b0a78b6e8dccaf60f55f44ecd350955f628cd463475508d343c5f4ea0bb633743f97109a255f6494da9a87ddede832e5c6402eb9d16d0c7ef211eb1a033232c46b5a203800a63c34f448ed39f5ae91582d3f94fdb425e931b0d8ab7a083d1cad82543729cede04e0467eaf40ba8b2d8a417d3243e36bd72b7bc9332a0e7b3c648b1f375b335a4d6a0a688470b260b16c9c4acd5950b8c5d6dc13886a0f4932f3a5d7e34516d2c6feb913cc7e7caab2e74e3e840ac1406e99ef5f934a09ea9760c87066172b17049551a6549dc863c225dbb29917544883758d9c07ea0a79a14d6491571ebf820007639b4046618bc872b2be0418b3fa29fc67d3531a0f96d3bc9ecd0274550349b3185b220623e54118d6de5234ebcd12b4af96acfa08cd11450cf60a855437344e1c6ac45c61c88ae9d2d39ec59ca9d061f6f4c4ca0f61a38e1b2deadead6d16c90c6adf4ad950503df44e19d0f7fb578fdb81824a05c138649f530912b26bab128d0ef448cff786b24becce662bd5e1b7f6684caa078089a21ce057667f4e86637de03efedaa1c8c280aa1783dda9dae9d71824ca05c24e2b0bfb9f5d2ce8d419f1c2475f6dfac3da82c6a9b2c5317556eee0ebaa0c0e9d57166a3d55290cba8c08a97f44a6eef866ce9a719ab1ba180644f1225a0eae1b50ca6f0e9213a6a32cb4131a9857f990583285b2dd519be7529be72c5a074dbeb7c465f12b84f5daf6af3b9e3bacdbbe07dad6bad7e5fef67f15dd715a06d23aa8570afcb5bfd588164f9107eee160f119f8d1874c2225a3f4a379870a062e1b395f26b05b2ff6b5dfe7cb2ab69e745f354334b734b67d67b12ad6206a0b91f6a6982e54edf854205fe1c85e1e187d46d405c8b7ec9cd0d6460371639a0b399bbf0fa72b2e2f210e364b71d19e5ca8a1d30a6889b20961e9be8b14ae5a005e549c5e4aad4c1e08d986de8bdeb541efcb362449493a3a4da660f71afe1a0453f2552ae22229593dbb06643f83bd62685f14eb23ec10237261d22504481a0130ed001a54cf6caee4f42a94947f34eb6974e2f0420270bbd83468b2508eba00effae246b583f0556b6c138ff519de3ec7aaeb3e85481705c7d0526dcc891a04deb8468245280e168cf766f529f364513c7786d2d07a779be7aa7a70bad55a06404411393e1a296055eb052a35961c097a0eebcecf95cdcd971307c8de802a0f29e3585e82756d17737bb530db24a5dab05b2fcbecc280898bb9a24fa96c5a04ab1d673ec64c27432fb4ff524cbfe3ac12a46f2f6f3899aba3cb84d00acfba00d6f1768a7871fcf30de150161da26c27184cb991db788482a56762f9a5e80a09b7371e00676212c48dd58bf937b645b9017e1f7f7e4f9dea45f73f173ba82a03e0ccbb6d58b682f20cdc58fcae0fa06e939781c776aaa7b41d9fc9832ef0ca0f08a27c6406d13101f590d75d82f4f8b9eab93dd6430b02df9212303fbfa09a06a324b3c2e094a45c0349bce4f918f2a2cefb22df89a103b4d0174b2cd91aaa0df5a5145e9f7fdf32f977078f5a655c7e50f239d0a43fbd26b58bc0d17b884a039685b6db0d1eb59eec5a99a133da44516346da3333ad7acfce06ab8fc709ca01839bca418b96aa48f97f3726260f580a731992a43df8214cd17d7a4140386a0baf5ccbdeab5a8e58a88ae616a55691034008b4655393280360fcd0f0eb438a0ba48018798e88636a3d85d0db3cfbaeb219ab64e7df1d8efb5a7377ff0d7eda0b467aa3bc5a188a9b0be389623f6a0370377b9d8b7569a0ba43b82670a913da06538a9f15f590734bed9fad25f102f70a4d0f2a0cc59c1abf530d060bd27c9a015a5bc3dda76570ea19da9848fa6eeaa634406989befe8480bc6c0ab1ab324a089a6765339ad776d525386303dfbe5df72af09bab96883c63a7b8839014afaa06f294ff9bc50fcfcb3a260b7e400202b82dd907ee25fc816db2d9e71fc293ba09764299afdb7d7b711037b05e2cc18790f1d17863443b0f0414e95d9703be0a0d61fd065198d134325df478677467e585b571746c53a158459f39dc4b5357ea0b53acfde237ff43a779ca220148aaa52bb5af3a5e011d374bb86beddd944c4a0e809e48ae143b449ff118486b79156ac0c5fad5382eb7656cf03fefaeb4fefa0ffd759a9c9ce1e3859c9af147db76ee900c0ca0cc545cd275ca62817dafbeba08dd4f34f71230b5f9288ea33bc22b981f8e02a2aa8e8eb5fbfd6b4c3a1528ba0a9f622c4b31fc54b3d3ca33b20cc234f7847736c7958aa29e6623985246966a07a1bae3393a9d2f7fd2bd032815f059d83170281f82e65cf6198d28d7fb5aba0c240af3788faca1b72e73335e58edadda49e2c035f18ca2d028396f388841ba0f54e86c3c43aa340f8fe2c78922567a0e1a87008d55276475757aa4b544561a0cc9b69987af4dcbe004bdf27b3de01580011c8be51e37e0e13231174a210b7a0fc338e395874bdd7a956c834af38f2efb407ebc1ebbaa1a5881e02148719e5a07503bcba8d3363e5c8605e1eceae70fa7c2fe84a8fea4276b4810a0ce67446a06feb72b281d99864e0d1a3a439efae5772fc50218745501c02a1d34adc0790a0379459bbf300e047fd30e1cccde341e73bc4588f1d92eaec0aadab77efd410a024d7dc020ed0056b92a074a7573978629a696c46f953650d66e0412953addda0207b436c483a50d2128ee6f31dd1c23f43205e3303e6dd1b062a918e4643d7a0cea24293e201b32758a005b3ac5152c80bf06b13e21650ede17c493b886ad5a03d94b5d2d5333382b6b8535775a28933ed49ce5c9f2bcae8840ec2784cb91ca0942dd459b519acd81492690d8c804c6e01e0b81c69ff5b77fd5984379faa7da0e1e38e3d9332c3dd1f3ab317acf110288a94c8223cd49ffe2345450464c218a0cbf4ec35707ea4d7deded1134116f03fb6ba36ea9260d0df103f4e4d1ecb42a08af32b2294ed8b47076b5f3cb2b948be78d4c15e56a8668a9fb8ff654450ada036b6dd6b5b0b63e0999f489ded04fec7a11eb8cfebe280b93868a0407d48dea0c009fbc9c850ecaa850cf1e91b5c37d4d817ec6796b47d4b0ee206b5b3562ba0326d8a9946909b0e0eccbd83cc7c404c1ab25696748b10ecae12e33ca41d37a09dbc9d46b16c04572e8a7e04194fb8e73762fcec670bb150c945b74b770db0a0fdc199867664408c8079357b10971eef4319229e327ddd9b93565a06398027a0a42641980775d28549d0ab5a496113668e178b8c01275fb3845f6bb2bd8692a06eb1abde805f36cd3f969fd7f643101f074722ff5ef900411ea4411b7461c0a0bb5ccce831b2968c54c257ffccced1fc9656161b9e2798ba81047dd25aa9f1a0089a44c85349ff69156a12e117ca804cf4e1ac0deb6b2b367b7d614756acd6a06eb5c52922ae5e2f6599fa68f8d8c474221d40e55029aa849851de91e38b14a057995ca665db9ccbaf3a5e46688b50c8726d027ed0831da9d0d04650eee400a0c6ac9dd20e99db09d8bb530b94b136781f424e65f865c972a032926984be79a09ec722ae2be8cbe7d72155b1fec286aa7fb5efc3b615634e30760654d691f5a0b6cb9876db26570c15e8064fa0b83b2793f1864d2ca1fa9fbea9677f9dbfe5a06c1be26d3b230527a0802c809792e4bd74faa6b682cdcb5be41922d76f6913a0e64ed85cca4b5e962a58ffb6adf5e359e38d1037dae095ffb836c7ba6abef3a0eceef2636ea77479778eb7911bfd82e4c9b2e26dd4aa1fcd51e2fe56f7dcb0a0e0627569e7cf51927e74186fa3af241550099e43275a1663f45757e18decc8a0de75b81d8c2ec4e8e5253d7a80a00c26946a03ab8a1bcf7562c12385e4b17ea07f649d138e945f6863e7af42626e8de7fa11360e54c4db55ddec1e9bb1d496a0e8a0f09ee2f4681a138739581a1b7f31e6b42f01a1e74120f04e97ca66694ca09e79e9d9b035880452e8b40a04afd01490a0a2c08a395157c95e3b73d4b04ba06d290297e0605d424938ae25cb25dc1d26ae2dca1557308e3c563ca12a0c54a099441cf22b4fa60de8cb8891e8d1b0fe8362c623ef0a5f224e0f8da9a4e5f9a03eae448c5c34c7dc5e8d63b627a1f09de83e7ed587ccea5bad019c91fa6d91a041052e422a98ed57dfda15297022ddce2fc3be6ba23ccf0d6b7af88e37cd8da02be754a7536291824ca28c9ebef3e39c682f4be4339d2c1e84b285c50c2befa0ff03184942de7507a0173bafc3f809d0e8c1a6af994a04b2697424669141c9a00d5629d68b40198c1f44d05cc803888f0b00ab7befbdab64049c3f7f864837a03ecf8ebd7aeffe1aa389f08434e9a761d5ea981127be3ecf78692caa9da508a058e41a932df7eb05fa8e02ae124a47d42bbc61d6d69cc28eaa570e55f1dc95a0ca732acec7d45fadc449b0a95544ec3633244cd6c862aefffdd8c12cd99c43a05607697d78d21bc5c7ac00bf9b0040209e3b63afef66f1862298bbd1790a38a0d4092e1151d9ff9c94224fda2a39b50877297b6fccfc66b566b90fa4ac814ca09e283585ea8ee683997b8813639743776889a4aaa2a480b0bfe726c2f24f01a029ba9be9be162361ca42456ced81a9b54b360d8ed19f18d268792bcc154d01a00df149b4ec16be70bcf2840b7f7766e87eda6a2daf7d7667eec1a677b03f00a0534486310132939a0237dd5fa70057c39a705fd0ac2de4d9b0b28a5e41cabaa0840c14482000730900616ee49c0fcd39c59b08504a8d039d2b8daa54cd4a5ea0eff82bf22669198b01177bc6c244b4e056fe960baadb13a4f0cad2b55f764fa01f2b2b082a1ce4ebe506ec070e7933a95710f9959818d1012ab3993277f6e0a00f37135ce0dca74538f602c3e63222e309042405923c62cd360cb27c25fb04a08fd21305cfe0bf7c0e059a5d989fdc10b227377aa57f4780e22cc8973a0450a0a5bdeff4edd4e5fbd0a4ccf1c5eee412971a98f894e7a86f510465c465b83aa074a3008a73e8a761ebe5ed73f37e29583c8b6f8c0431dbd01e0200a81f835ca03609b2ef331020fa72b70f0b5e2cc1b314d05c23fdedfbe17a078c4cbbc218a0010517b17a71338775ca096d5e5e20528e3a2928445bb9faf735f6c720b787a0c38d6c2167bd06cd05e2a7baeaf6105dbd505e645de6bad8815fc133cbfe7fa040e036f711c08fc0101b1cb8c60cf3a33331bc7591ef2e58c4aa2544631ed4a08c9e7030da50576c7190590f1e8a6fdea4e51b2885a72fc093a66da050a111a0e409373115da70d7a7b5bff154360cc22511fd9ae71f704724c4d3c9c4608ba066f678874dd2622176852bfdaa17630c9a32850fb8edcd76c9d07fbeebd610a0597acd7183a15522aab4dd178304000f3449bb8704f06fb280bec0f39d3721a07023b229e34ca4720e51e1582c5cc91f9c30e783ed4ce8f13c01f8443d987ba021eaf0a650e64eaf46f48945b4ec66ba759b20f52a299e8c8ef50f5c273cfaa078b7e9218578039464753639d482a97a27ed7356cc3b6fe4bd7779c5273de9a0fdaed74e550479603b5d1b1714f0b396f24a2f78c8b11370e61448d4b64ae2a044f2bed56ce317e10eeae3a59f8b6c2759efc793e99b84ebeba737baf9f7baa0b506c421225e0b874bc33f9c1674b1e0c62bde4747eef5e372afee3bad7922a0a0cc6c04205345da8e7728df7e43ec63ae537ef7e82221ec4d0b0f486c6ef7a03ff2531d5757533da5e6a7c6dbdd7c0b75e1c7aa4e2a78cbe3d2020b474758a017c7c0db772e42d32ec6ba4030bad3d1e243ce6a78b7a39f953e47d85b3e74a06629d5f7013612d5ca368b7d7e05df4023586d41cbe9015fbba681160c1ce7a00ef3ea39903c011802e12305ed42032478622f4d45ee88bddced21bff5e46ca0de5953d6b8a6d3aebc703d6f2650da3f3b1c5ea011cc7085d61dcb0e80f159a0a543072f2cc9f00a201a4c3536c6b06aee5e825dc020da78f5f6605a71873ca051104922604eda8ab1de352ee960781f944ecdf58c5e481dc61e86b3ddaf58a0eb4f6ed585738915fcd79e133798efc4f5b7c6631ea54738354803b94cf9fda0621f4f0a365ea3c28e7df099241e7619b39e125d40bf0badd31dc8507f15f9a0cd4ad0babc76a50ced7380098898ac7c3b4b86269e5f0fdfaa5b0008b55454a0db7de1be029dc569b9905f02f3fc2f94f9be73379e21eb39bb48fcd687de04a0234cedaeb3a415933e6876cd91816ea656572ded39e1cecc52c334fc2384c2a0827abf079aa0f924a939937069c51285471d36d82456759d0a26e32f154622a0d3176fe859bd0ac5374ac0233e64bfa29424cf9ba8375f516026ac846fba2aa01e9d16cdbb4851a681eaccc301747e93ee43ce6ea9233f8e4f23b91218894fa05de14fe1d3b56b772eee3d14661e22cd4f96825e95ec02aa296219a64ec2d2a0458c8ce9f33831c963dddd2e2be8e5d683faac3d95b511b063a6ef5ddde22ba09f26c33c92747b9388dcfe8dae77cbf574e3df56e60b953fd7026227acea8aa0d44f6f44433a4cc11f31c728e3b856e4d2ffb47372ec20599558f378d6c9e2a05e3ad95f04ab0b761a1a426f306397f6ec3c1434303ace8aee872a851fd902a0b2bfac925c4e4c2b48c276bc33fc7cc63f55301ac5444f8e36b49e2bc9c90ca01080ba2f60f6a4fb5b37b3776fcd79255c33a0ecdaae97a72c1b48db66b6a9a0a82db02e41727ab0175262d938f2fda5f4e2533f59cd6b2f541667bd3dc492a0a38316f20753eabf5522f165874db23a435966f4238b3cd6f3ce4cade0f2a8a0d092ee9d3668241af8afc5da59a88da67cbba3c8af939de818301df3a85c9ca078431d09995818423b8de7618286a879ab62d07cc0de3b95ac97c933ec5ad2a085d612cbd9613a2732ca43f835a1ed90fba7f438b778a27a6ac023e634e52ba0bd8ef9c380e067e4f57e2de1d7b199ddfe100ec105bcc1e40fa7e61e6d7fa4a04ba35b6c72722381993b8107ccf34258f5c5843e5060e584d1d23ae3d680aba06d84c5f957ae0d299bfd2e075f4471f548d4a03a66968a967bd2e8a6aa0bf2a021b5c21824102c60947ef5679368c9b0636dcfeff0eea9db35d829341d5d09a0ff5f819db353929d32889071b272cdbca90f7804d606ef4b00ae80fa47eba8a04d71414287a9d8877d112542ec0bb1527e7d6910bf6f8db9846e4bb7d45bada0595e95caa939f4ffe5034763d027c2836a84a718ffc94fe8f6a4c725068c3ea08f870cc46c96768ac520570e0ece5c69091dfd14c9a886cb589b6eab9a703ba02db636bfa56fdbd08a8ff7e8a7e240b0ff6cf460cd6b496aa18ad142b8f893a0b4408e72c18ecf72061b55dd73ed296e2bde047280a3304ff6b641855f87e9a0b3708d83974805eedbbce55a89e5ba5ab01fd54b9cba6014da649893080263a018caed39354246a562db27ec309fbece213558910ee93648ddb4e5a193adeaa0cae6a0f1bac06f637e3355a392ccfec0d7fa1932562d4b5a6bd76c7565ea53a09149ab137d1b7267d9161a93163c059c5d3296a262b1d97d871295d18bc9d2a0efd41e2ff3556f91f4a14361027333407d07572717f0712a6c99e2546379dca05b1b414513248f0fd7a2042b6af2d1c3993282eca1a4d645c8ca697dd02bf3a0a72e851a9fe9d55e2e866f0d069ed17856da03b05c3ec6ad6b5abc515dbc25a0e12b81d2ef18c8f29c14f69dcf77b1d7dd730a7c3a5bc2f483853c12ee00aca05f7b7d9b86fb5baa0c62b66e72dcde944627cbc6b9fb70a350476cd30583b7a03abfec5d298c0c3af48c3ed84229facc66514317abc87adc4d2932351f2ac3a06e2f4162fbcc257b9b2f22e1000a2b5a9259d4b002ab6c3e4d69350bae3e94a0c5b863a4bb046b9eb2ece5067a6e2ea093221c18604cf283900feffa1bc3daa0d121b31c979dbb567ade1e621a8a482de55de400ebc63afd68a9a4caf59199a04efec0fa0a338e60d9469b89dd9e7287ab2aff254add00a12120a2077aa66ca08cc8e84aa2a3c56c05aa8a4fa45ddcf18b335ececfef7a8c935e0066d861dfa0b930144430a2fd705f838c7bb535a9597cf2123154140083e0912a7884e1eba03c8351b3f7d3d32847f403896e80aab27c5eaafa6dad7eff9e0e8bda213b4da0f15786a08f1ef22d445a16db9054196a4d40b0b66f42dd41face1b9311dc9aa00c7875b561f2673d6618321c6a21332f13d81a7b46b57644f0421eff11f446a09d7a246fafc3ca9c41f7eb3f751a5996c6e8c875c61c439153929fbfd5cb05a0b7ad249519de8a2e8240e36d2cf387ae298fee44d7debabb3e2b24652bf7f5a0672d1cd767a563e3170a4a4afed4fc10b2684a03de32ca8aaeef6455af161aa01a0868feb79a0ef51ef34545d608b1cca3159c9b12a287251c871e597154d2a06962d8be4f048ce82e1199abea6acb5f2c5a8e95a65503a155d291547bf1dda0ede9b4903ba9af440a396e04c205048c6cd2fd8bc268c2e6f7846bfe4d541ea08fc4a6bce1fe308a3cb1b4cd7fd6aebbbad619b9c4080a34876b30cd4f4cb5a09756e79784a2a7aa07f633c5bc9cf76fa647ee4950f7dcd2150f3d73dbc5c6a0e75ae92559b517bfcb413603414cb43fc6e026dac4302fe89a35acf985c5cba0a50678e8f25a442804d943fb374469c9c071876a6710bcf067b93dc4dcd91ea0d465852442bceef9dc4101cbdb0b74ec10e2443b9ead462d3447e985b30edfa0f31642a41445bc37914a10d09b4185e80c15168b53c31cefe38ad4b186a329a05807b52796d0e0dc8ada7f4cd8ac9b5b2d3ee9ebfecede66d4ea7637121192a0e1935435ee90b20770f52f67fdd39c56383dac61531c9501dcf89ee023ca67a04f9598ab722268ecb0b37114882767d659123dcec1e0211baf99b840a9df2aa0e284592ec56a4ffbd44f05d05da5a4c46f1b91057534d83ff6af7a899ff48da07a3d46172721e4475bdf06a34bfe01d9ec68e0095ee3ee06383ff8d0732f1aa025a12cabcd046e4ff014d5e45a6c1a9e5f5e85c401fdfe265118d4e81e4a55a07f0b7990f305d628dd0f97def9cdc14c785c5cbf4dfc8a2d1af182268881c3a00bc9bd1761846eb8c1c32da87b8801422ccea0f008956c6b67d5304ff82ad0a0312a21e7e354a42e340751c70d1b038d63d8f2981b1a124c3a11885424838ba042df36819dbf8514af100eaaa7213673ab40129b1d7c4a7ddf39072f468d2fa08870bf68ebe609e5763bc69b63908d2ea59eae5f6bf1533200619961f2695da0ea9512b99a8e4da0f4fc6fde779eb89435459a6b5226709fadb9c3379689cda09d15b072bffd20712530645a47537326ee472cb557d2ea521a995d12524d49a0ee04843e15ecb5ec0d353f8a9ae471ced8286aa44096f329e9cd7b8b06c546a0fb9ddc955ffc60764e1fa0ecb9b737067c24a6388a108e2935193e22187e32a0e89eb65ead451f6e86a2b1ec9bcbecdd959e7043232c6398c154f286de05fca0b443867b44916d791425fac181d9d9cc0a3b179efb741202cedf97e21d5bbba03203ebc74ad925db2636a96eec57d76a8311c32c28998a9e6a3650bc98b59da0c0f0f75bb884be195a9590abbf448eb57e7ca027f194de176e56a2a1731b4da0201419fd254f2d5b5899ec79e1b33b89deede87d3d79051272f67ae3f1223ea05c12de94266a9d2709fc6dafc8606fb244c77b907df583538e229d767304e2a0f0d732e3475290a6a5272c9e5d0203f916d53d1445cad54136277516ccfbcca0d15db47fbafc4f7e9cb359f69647b08151bf1415623ed79dff588f9026c5dca073bc8e5805194ae8e447d997531974a25939bf4a1e08ff6b3c24cb1b94d794a000f7d28f707c051e974e87e91679577de8b0474dc482b9d1f8436a3ae79ab4a098b2768f49e3f410a50039b6bf36757a950ae485011f25b40678f34c05b6f3a0e4ff1cc8eed7c6f6e57754a634822faa6e04779a4fae943daa570dc1a3edcfa080e13fd090d75705115f3a5b221bf8b21d68426a30f07f6959dead85078efba076ec74eb1e05b4bbac563bd583225d96dd5d457bb824cb54237dafa8beea19a05f7777e7030c930e3e5ca1e9d2a1ec7bea9c966c83d669426bf8480dcb8666a007d5c19832008b52bd23a4ee1b339711fb90313c8616bf46231f823d26f0f1a0fd0bdd7170e23b2bc3baeec748d0beb16b7c83052e28afa24d333489deeff2a0f90dd877864602149cac91dbedbc3928d8431e9c01bacdfc1fe599f2b6940da00bacf4985a4d86274711f8704a52988a825128b2d7d85315190f0e514cb224a0310af5834ac16f474b95ada3db3dc011fe4f1335d264fa216c414ae17a4f56a05ce8df9216bbbf25e3b3fc34afbeba9ada943a2a45c031a3b140ef4f0f764ca0f60a2db91627cb08a08daa8ee4c19805dede8dd9753ba993811cb00842263da0ce6aa7dedabf39f97d7e2bd7ad97f70c2a8741689cd45e5b52d297e0386795a02f9c56e98798f186e8e494ce75c6daeadfcad71ea6ec13546d941e0d309c16a05ed9cd135529113170ca2bb844905609233b2735e261939b09abb624b9ce91a08a91d3617336e1e3b551b7ef22091a77f0089facf07686554aa9b2d6f293ffa0411d64825bc7d671f2d12cde02caf3f24925075a471a7eedb64d520b576d7ca02456581a66534555d93508e24f0ce54586b8ffb24c9bba8a55409f062fc103a0a21e77f1278b7048e0996be2caeba1586fb4f3ae2cb0786d74199b66a906c0a0179ad0f7b8b789b37f494c8f24e55456cbbacec86988eaa9b92ad8c23ab147a0dd9c552150728ab8286399dd421c5d3fe3125f49cf513022299fd7db44e583a0ff120dba6119cd24e1862e8d5633a364ff1000a6340dbd80630a1737ec2374a088dcb1b92aa504cce57bf80313b181fd7a6d99b9aa8552a65c265708f1f89aa04dca126d50c1dae9e3d5ece8e25f89a79afd740abc9eef15ae1aec33cd015ea046b58d1673d2bc7c8e03f6fd45342cd6cd39dade30de2f3c93841a4dfb2660a0f51b946720a615eea55e1057de1a200967c9df3adba10f5af80b2810c0cf0ea0334afa686c106ea41b9efe8d3d68f00f699f438042c7eff769bec2a62c7853a008273ab6827d898ac675d378770860662f737e257186bc6f07104929a9ca50a0a6355c5fae30949445a80d314ea913543e1f33e93a1a2bce6dccaceeb277d5a08caa48b6c165226b6da2f297b6660ff979d44e6b61d6849ee1d64f7e403c16a0a22172609041b83b4bee391d899ccf8a60ef755c0262e5dd57e92a25da698ba05af2b239999e13fac0501994df87c3833e927336ec93399fa32a18eb87bfc2a01f2d9e7c62025c22e38def993e1b99a5e4e654a5bdb6bc090c7d96ce037800a0356c482b393784d18100805f816ad864575261751c015593652a4b4351110aa04423b43b4fc37a411ff8b65d8ee8f4ce945018a7f4a7fb4ee04b9049515561a0877c9e1a848d27943eb319e6e31660e9dda0245956bf7a833e111a2bcc44eea0c6821f12417193541b632e9b438d15767919c16e1262fe6ef6894a80729eafa084789224c8c19011419f8919f395238f829ddf40f0987ec5b1c4f578b5c05aa0f4f7e3c95b5111da79432154ee582193b7b66ff92ffbfbeb4fb7b8e1fc62e4a0b62743f95c3cf1e05b4a3296883ad8af510035bebaf993fbb02e1130bf198aa0399b6a73960791a0001d74f0a2b8173e6c0835fa5bc38b56cfdbc495085754a03e11a6257f2472711650d51adc1a533e0bdf374b45039fdc7c46dfdf88f651a0d1fa93ca9787a531bb8b4c36721d7ba33a4cb382709d4888359ae28e192170a054814245d1fc9321cf72a29cfcaba4588b29672135155ae8b8950d69282cbfa0acf95ce2e09feb15bd32ed75b8527ee3f478e803761693e46306aafcf8e968a0d6f60cb1f7082dd528cc7d0d2324f17a0c7c7c77925459cb53d9113d8b1052a0c0a1565c3009052df580b903f5f6220e6937f108836c60938a7d54c4501f03a0562d6b1d6d89bdde0b78084dfd5045bd420c6587e35a523f30de920031fb52a0cc4c7218ddf91bb8f22ddfef46372f65ff3c6bb57f56682fa06a7ce8482771a0818f939475ae6ed38ff0182e023dd985d3bdd350843d5889b615e50e5635dca0c2702420b212003bfd191ae85ae141ff4fd452d7ffd469fc30f089c9032476a031d1040d747c5c7c6604b7158aa115030d95049125b5ed45afb4124b675907a02ced5bfcc9075d10b98a562dd308592597259a2895f21831fd7aaf2f992aeea0fb1cef371a3e0d83628c6ccc23270325528b9f9928220cbe2fced2790f46a0a0a0d8320ad097b68f00330e0ccca8ac727f24c5baa9091b990f50448be20f7ba06e3fcd43d32f6361967e2c450bd8510f6458b277c60a149ba5f071ebe311b3a0862253d50320352ed90c7327d4c8e3b140873eb09cefa65cef4f0472194a1fa0b1c7343129595fedb27b23381273d7f8ecc940c8a75bf60b3aede2c0b19519a075639aba32daa88b78845deda29d1279d57deb28d6a558d4bc33d9ca6494c7a0b95ed40be4f3acda95d25fcec7ad5ffcd8f11df510ed408dd44f5cf279c66ea0612d088d9500582701a418ec05779240cdec24a0d854701fe61c960c50bac3a037b1b156c3463944dfe2a0591b5e65265e362d00053087b1f4f5b7c106df6ea0ce3ea89ae3f8a17613d510767cbb0eb4e12b1bf967bfda107e27d0b6193cffa0bc2b17b9157e25da4063e7d9695a9afbae72b82e7960eb837f8dfbb65a2c2aa0eb4bb04bc7d6711692d15a8bf2d7334468f955261d5415efecab7c6a1503aba0329601801cf638ecf4690b724dec1811d2713d47f6d69c7d6d4201abfb30fea010f8fc79362dbbf33bdcf59183221cfa440a42c5d5f5e9f768460432942edba09763bf4f45587b0c1ca9212610d6ad667fc1e809ecc17b64b09289111a5915a00708e27339fb814f1797600095fbb7fbeafdfe95df3787521b275a6ec3afe3a08096426d64d60fda4fa0e8f1abf1851fd27d24dd5f5b4b404694499caa8ae3a0ffa9302f045e473d34aa723dbcb6c4ce2447cc2f5ddd03fc854898f84a2abda0aef373f16e5b5f9209015ed01862e9a9164ff7f78ba399d226aa2971421690a0d7ec2f454cb8300add593322af7c7b2270561cc2e8ddf3b6524e5dc4917bb0a0c39934d370b7f314228ab7a7b64f3ce1856a57eac4f6dedb2ddc0d54fd4393a009f59941d6accc751784e099f3059874b377d823e586482b6a48ecd44c06cca03cea591d32f8e75927397b350b14dbbc7996aad62de3be1c5049d71aa17cdaa0bf36a13646349bc8388b6916ccac1732b58fdb766a71380328a2285182c0bda032a659f63d2c2aa094906c1655778a1d035ec8467d9b7bea64e467e734e49da01b4a541a9953bd97a31adf9d1e3955a729d58d37a3e432df87c981c34538a7a040e3e5dc56fe3449530201516849f725872bc2bb4ca6aa92d522f21bd46495a044140271ef53550b23f74b4492d1723a6009a83c8574365ffc5b827e7d8158a00240f51816eae080ac06b72c70ae1091ca5685cca10b36bdf545eec83da730a07b2fd7b378b397bd833fbdd355e461885ca9b2cf90df9146c1695cc4081423a0673234d9ffbb90cca5449301182c863096055fcfe01146f34a5c85396ba0b9a0ca4f11ff38f7cd358f6e012329b71e7d3bd9d90112c4012b125a14a8a8a189a02359b8ce566a8a1cdbb85aceb82bf84abb5789ce6f8a3dfaaacd1ccc1bda90a093ed61c5a469d3a7e17e52952be1eb888181a7b155a387041f19c256d21094a02e613c901fc296487543965d10ddd021bee2eae7492044616a603b749f8b39a02fd8ecf4ed1e94bf70a3f66698acae08c8bdf98cb56021d3553efba860e5e2a04122dd2516827b717db87025d8b5a35b01937f045d485c7665df17191826dba0ce658837050ed869b5052ff1012580f76067abe0150558c97a15f350ab2a1fa061366ff50d237a14ce8c8b04401aa70402b6668f0766a0d1b8ea7df9c10fcaa0606faf503baf534877c7a4c164e819733eec88b126d033e629b0df3b64fc43a04b4b2eb34d019aa2b5454d21a1ee3d4ccb67caafbd24c3b879554fb2d2690ea023bd72c223b40cf4c88bd73b632bfadf34c24dac08a174f8eb8446feb7934aa04ce570e47015d5a800a8766acdb046904a67b8c06026628ae12cf93866f9b3a0768a0d0c290111f1e5b9db0e9bccc13ccd7502c4918387a05ede9ec90d8412a0da3c3e525ba79f62a99b409480bfe7d455e93516d4ea1f10825c3a6d06f662a0cf2f3a3d176e094e8192ab39d63b3d1a1ae3e9985e50f8ef9da3a7910c32d2a08df10242066e4bd2e6c9bd01c9aa298cf97dc9898eccab43881c1cc773f240a0e11ee39d688b90a7e7bd2c0e89b4a2709456bc11837ed342c0afdffb6aae91a0e745be011f85ec24adb6b047a3bdaf1def7a23bf25e7d57b00cd811e5cfc8aa0116581752f5bb41019ec4c692523c43830a462795e72a7de8d976a1cf70652a0fec8b77307d00c838b0207791500e3f747c4d0d111623e303893b5458ccb1fa07d8c7962e9e5bd56885f57b88c6a058581e687f56ae0935f5e3a31143d0e7ea055212aa4774d9ed2c8c93835394262005ce55efcb97115873bbc184b3a4d1aa045c7dfcc493dc8966fdae725cf061dd33ab5cdefb32b11df1d240d5353a2fea09c239ae524f44261c1d0784440e082a23554935135445350063b6a75db9cefa01cffa9c9c1151cf79c5cd2fe8caa6626c5eadd4af62c2874f20503b4cabbffa06766f4878cb4c86070e8e4b6f14cbb6f2ec90c39c2ac0ebf6d829cfe192bc5a08c3f21ede68a236eafe60a1eb16656820dd482eb9cc5951cd2f77f7b8d77e2a0d6845059ef1fdddebd90516ffe71833e7f827ec5d8a1264c35a7b65d422c3ca0dae7b3336892e207d4ea9309b833e8ca500c8f89a9a878c2aa4465aeb7dac4a077ceb0290ad11752d26950e419ec8961d60640f883cdf6de5ab8d153973070a0b1dcbec3181d1bec7b15fbe9b148ac127ccac38a3b008ce6e1bd5ee23853e9a092a4f0fc965ee9054547d64c8887ec9e5d523491dd3d18591c062282ee57c7a049a28df869977c9e4e7bd9f2c55c764ac0bf38a8bbfde75afc81366ea4bedca00a12297db4df47e207c25940f8a8718686cf64d27d101643da3c80e5e56418a09c3b46224d21d7cdee107b90f776ed340e69bcd6576baafb50f142f7aad8d0a003ada82f3b35bb8a7958200b476d23432509fdf52c9af4953b930cc1d27e7ba0a2bc5a1c54aeca9e6855daebb9b8da561623de194dd7409dca6f534ded58daa0ea51009e604d9aaf3141911ef6ece459ce1dbaa5606b0f0d905a2036a9d424a0c6dc8e583990935a50158c29b0f325659f347a84fc263cfc081ca71ce06038a0395d68d722cf7fe5b1b4c53747517b5443ebb85cfa968f3f474b1ea1568c4fa0a2770cda8411981d550ce0dd66190f1ca39e075414892e351c030680a7ad9ca0d0e33b0990478f1afc72f73ef4c7cec19d95339fb537e3095c4d3260357e91a022ef20c9dd2b998b6448f14880a8d43afbd1046c777f51991e2b4c5a946bc0a0ee17f903ce034729dbcf468605d9162d657a95c4f4da039e16be8543187d56a0853afc05bedce5ba09c6c9c8d4a8c84cc7626dd1b72dfbd0d2fe88faecc3f8a077a73ade30e96928a8f783746f0cf7e30fec3810aca8f6363e62b6c22b8b6da0bbcddfde504c8c219ba0fd0996fecd4e85f4cada6643401e74a319c47e8a2ba0396b68848962d8b677d39f6f76c2706b9d9e3162d2bd0b080e23687c66d591a091c127d7e001880027c30c4006fce7d956f816a234cd958741f80e904cec84a04015288753d9ea3cac3f66e6008946f2d0516e3bd4b5e1188f0cf91f7514c6a07c24a5461238cb2895a6ecd366fa291001f76da9023d01af6d1885696185f6a06fbe7051bef6c5ce9ca8ad58928a2d9111b44ecb6dbf70fa55fb5e47cd1600a096f84acfa9e358401c375a9cd088d6409116c9532086a8de9b78c98fa2feb2a09719401131d987b26ed5cffcc6d655bec966966f10578ead51bd8779003119a0cb547f974a9697262d3cd8411ae1d9a814031af8f8a8db86e496ab9c221638a004b20ad4b8c8b26f6e1602a91bf4a0a2c88eaefbbef1fb039848931d6fe4eaa0bb1def9613c099bb6cb7e8801292bb3653458c03d8f8a7abbdcfd0ee73a8bea0e2ff46a8de312939bdb1f609cd5bb5bc6903ee6872489745a5d24cd10e0fb7a0334d51f26cfea9b66800adbad7d03fde396f37f88f14cfe216fa6f7ff5ed44a03c80044e72d17566e87020eba05b4965c2df64d17af3e30216b2385d14b0e0a086668de29dc2e2575269b6568c9ea1e069d888e3920e8788593dc5cbf5b4bba0021c364e77c2f894a28d02363159f2948be4a8b94b867b2e308c110f75b773a0caa2aa65923c6e5addf15511ee39e3d8e66677043eb5c426c325720a4e58e0a0a06faf97238d82e94155aa29eebe6647d20457ae6e42a132f12204da92b9b0a0d0eb731bd3187ee98f2e1ebac0bd797b3fbc997cef86779dff757ae8d444d2a0c0767aa9d3cfebb7ffc32f4a019523ac295ea020128be723284ec9e946612ea096bf73aa072cb50d80978f4078f0334ff36a5e2903a7e2aa44d27ff0946406a0f1685c2589b3560444c905ba4fb869931763e7490e34947210de19687c863ba05ba804f73a21b99301d315cb7d613cf227faf5fe1a4a62e32ab527c66c550ba0b74e4de3e88882af9512326160e55b4e51434e22057f337ceaa9b7c6585c62a0d3f6d04717c35f0a1a9dc76ffc0062adbdb49faf754aa0d251e03be112ebdba0827d41d2c2d06591eead9dcfae61bdaac2da1dac37008f68523a79c9734b8aa05d7afd09f498784b7ccda5ba5b690def2ec27abffc0671d72401b1a12dc0a2a050907122e28feb67e1e4be776a11a985507ff2bc262170b2f9e37dddad05c6a0835b9f78e3ea584c4caf44331fac1e2a0c2c9ee64752784f7d8af11896c1e9a01559f13faa6ef98b4585188de1d46d90c11cdfa7400bd1e4875242f3f21ef6a07f03adf9dd0ca89b63e08af49330af93a9e96f57f64e1682564d3fda47a088a0571a937bdc2fa45108501b6b647e21b6e2ce0d4ed9bb2d59e76d99e26e304da063ef3248d78eb3b2c0e7c2dee9c20adc48726210f302fc100ae476702d9893a0c919ecd6c1cb2d5083cac0c96659e4b4bc28327c1e2385b1ee06e0292fc3e5a00e124e28d3c3bf6f44239f433efb2c82df337a6179bd97a7cc02f1971940fea06e7f7746eb9ea92d62b6bd69be730aeb86a90d6d603aa6497b7862a6d07193a05914cb7c17d03c5708393b9a80be5494ec8a9bf98483295d17478f3e1fa872a0b4c24f3a995cbebe772a88ae8682f5384c6181d35cd45de2e0acb0b4a00e6ea0cd544518a70b823facae9fc2ec31eb33ed8aa8a5c008dda5f3a6c0466c05aba0ae5695cc14eb04c6b79231c123c5be2bae37f86e91a8178a9962a5bb635930a0339f5a55cb47a34d9a816fbcba9a101f71d16267f5188661069aa60febe83ea0d453e5c8500862fa4060e3f291a64e211e72066335961e5afb8b0fb704288aa083f57ea64ec776d735ba180fc59e7ebfda7b981f22dbca257c7713d770cb8aa0d8bd029a2e3ceaf920e45d5ca4f7f20e9b6c6b97b34cefd1ff306ee363400da0ce20a92879fe3d94efad906bacb7531a2de0f135fdf68fa9054b2332028d87a0434f2e253eb77e7ba666a419fc0405422418705ba710b6e0f504c21cffd180a079d0baaf2f847c26122c9051d73d349c284ed9a9ba1d21658879e3fb2886dba08b959c6b96619d25e0ebd45f66a96952fee6841b0e6bc8a4b60dd6ce983162a08b92356bd3a7751df0b283d90b933f72cd1bb6c1a5b6f800dbb37e670abd07a0fb287d4fb503449b7fff2502c92f7567a1d8272003417a575d278ff1e6afc9a054e1e7cb0b791763c7297a0ed4125079756aac0689e09c0f6a452c069228c8a0a3f6f964c27885ea0a4d778476d534206b78f7b700dc1bca0235d56172e92ea050c602c81c9607af847513f411b850bcd8984014ba6d4168f37467f89bd71ca08edab46273da9523b5a2d25a61714baa9011aab3de3752e00736d2da08129ca0fc4c4b43a4d5df0fd613436cbb41d4a233dbf5cb3275613ba94d2a2bf5af3aa0777a7fa5bca8ef211f7b022446d42f4f9edd590855c87fb226a68078adc0b0a0c5f6e6415bc8cfdbb124813e72983d050a9114402d20adc789163b8244f620a0c6185de920d0c957230f13c4971ab1c419846ddb29279d164e9be1e4bf1781a05a16505ab91277b6321890e68b61b512e9a106f0dabd99e3b5717758a0f9f0a0c4e93f9b87b750a1873387ce03147bdd11f75f1dbfe52b313c446428b47b54a01b1f707f60dccd10738ca2a7960cfcb3568895245072363ad6bdaf5897479da074f04f2dac6bb0491c21680204a5df56182329ef0a32f743ad4201ec331a1ba02990707ff415e05323ff77d5a9a182443baa0372bbfef34a6fc550514ec348a0b3cd9fbdda7dea272d630d23973c1e1535416685992f76827b5a2ec35061eda0c775909bd81b2f148d667555f5edffc1f21f7f21440038e0f8d34324118ff2a05f1f64ba505364722bf141ebfc7506aa1f246f1a50c4c12914a0bbd8a68505a0e8b1a8e9e469745c871c663fe4a4fe7f149778a94c2a3e64ca01f73d8dcd17a09fe814833e655be81fbad42dcf73507c23863cd394f4c657f12afc38f57926a0502343406c123ec296292d13c5920ffb5fd3ff97b6c78291dfb67c17fb1b9ba084f2b62194a8cb42b90c6489f3dd4839b18bf2417cc80bba9e4f99d9ff601ca0c099d5d7953c04d9992d68ffde3610b154146d002a72e3c61addd133988028a09271aa1508238c1e396646b18b2849c7a649120815a3cf4442a9db79606ae8a0398079e0d7ba9ba1304984e3fe509ae3be006958f3da68b28f869087b247dda05ece0bf0639946feb3b0e4d889a27676b28f926f7ea1a1ee882058bf271c9aa07addba40fdb75563ad63d20e268d2966007e88876278e7928138acbbfc0cc7a083fe162909deddad6e9a081ae7068301a49991a7f37146131dd18b071bae79a03154d6090c691d99220d0258cc141b447d745f914229eb800b9244c7edbcd5a01b4023c7d7f44f9be586ad448d9883cbe909d7064dae91456e50d212e4064aa0f823bbef04553ec423200c453b9be2ab92cfa8c46055fcf40d05a41b2fae31a08d2cf33204f4b3602a206457c04f6b45ce439e794d4ab2ded71ad9046610a8a00924b6a9763bee43642c5d840e45caec49efcce2ed2a5a1a705a70f3542e10a040333a3646b75959055d17ec844579fdaef31bf4d21a262ce5b6c9650ba5fca0fb7a13ce67ccf755354030eddf62fb23f933fa733441c2fcae3bd8e9aa4523a04f0f573d2116dd7930c608db42b01e7ac0318c94b722c24e7ff9208a1083caa06aa5b9c1e4da3420db42bfbce0ba66d74406c92c5b4db36f1f95a140d67c1fa0a44387810ace6886821688bd81fe1a028803fb15b57183b232cbe7d31fff8fa01fc0738fa1d42aeffc7ff8c6a36e52968d6e6697222f54208cfbc7f7be1cdca08135689bf5e0444dd3ec4e9cefd23a0a7760c72ce45b2ad8f6f5ac807a06a0a03683b9798c8eb7211259a927474f17894243ba0b043e716885aa70f8289162a07da2d576af2dbb7ff7a8c2290285f6e61e9ca3193cb023a115824c3f52fd1aa002236bb45522439b8a8acc8d562a85bd1b9e64ec0d40e75a6f40af38d2af34a00e57a44aa26840bae9f53901d886fb0a892f01e6bf9ed15d0dcc66268a063da032c290bc585f593e25f87c93bb3d9fe0efc8fad8037e841358350d36520493a0b56e7bd09d7bf0c58c2b9bd108e2e72fff9d18179d70109325d63692245170a057e0ca2633d896b7e5d06d659acbb468fd749d32d2ffb7975cc584c3942fd4a00465f88a6fc35c4403089e64d06947d4cdf49d5a9a886a7711481b8618d683a09e879bb4b0885a63543efec44419ef8c08a8f9ec968f92b41fabed82899038a0be7e349d874080167d9ba017dad0f2a243c5e0dbb9de0b1cf38fe279b9f860a0c5879bc2b4250f963bb8188da9a77c5c725e7dd1bb61617f20851a826b65d0a05c4f3dbc84e4428fb037f624efd3da8eb9c1b7f961843e38dea947f1733a3aa03c8a339ce9814534f732e5c6ed467944af58b38a48595564249bd19dbe0afba08feaa35fc87d16f005a4cb056db64c8bb5e8bdc75fb91cdca3bdcb57ef881ca00d9533310c1a0496f69923b8e098a304f99b5c2ce90c668302dab39cabf3f2a0c0a19c5151faf05265e20ee2e962de6241eb1c633785f2bdd5be8f68ae3b0ea0a7c758c0d3416da8df761805c10d1abdcc2b60b369158df78d2c2a84cd5410a091770cfe35240d09d5e7052691c6787a4d71457a3258d2b1a5c4952b43ee90a07f894deec2ef04a9f327ebe8f02f7df096e1d1095179731bdb7343500a9e97a0a832071857a4daaa516f86f61156dc67351e265431f669694b41a83fa90f9ca0916fbe80e8eeaf7cee55b715d141699c61c44b34e76127d0c104fd037cd882a0d428af65a10b97f4c762d707ae65d8cb9d710d3e42222345dab0558c008ac1a0943017ddefa1db7b7f35149d0a2020b3757bb9dd5bb3afdf7fceadb9cc1845a03db28c365425da841a7feb5d782fdbcbcd9be5e77a2d60a6ef3a52208dadf9a0ddaaf55730ee2f4fc99a0410ba41d5eaeace922d000a6bfe7b31b92c943bb4a060cb566c432864ff81f9925675cad22e4888151e19b092c10b04c4f0705044a0b8a50d49a47c741149b1cce3c98f737b8d31d98c5bd15726c9747eedf6c7a6a0c97c348732c18476c1bd781efaf457bc6ebfdb845c1a2ef3277bbe974b3d76a026353587d449d47dff3d8af19e66ef204f35ad4527451b9fb20a3370c32d1fa0367e0bed305a83874fa0ea98f0a25048e1b1f35377bfe4f49b767e274d9586a075b3b121bedb18f89b75dbd5f30da83f8c13d995c2045d3ac930f3efd2f5c3a0f056af0a14fcece2bc183ae6612cab2fba5849dd43e3c6d2d268004adc6f9ba0d605ce1e104d33d0b84f88f12dc4066f6793cdb5ff81998ad6ae9134e9cfc1a04181cd962047185ee6f98e79a93fd1f892a7ba3c54a6f2bde6f65c9151824ba00efeb4b0d65cb0c5ad06b9bcaa84eabde544cde0bd4927bd056bd54b5f22f1a0d337c3ab4dcaeef61736a9e4518b7b2561c4d789a66b3a1dae482d1b2278c4a0cbe69b6f1f361fa60dcadf402ad2e975395c46a1d25fd8acea1064a9dbb00ba0cdd83a298d677554b48a2f09425d99ba6e6fce82dae3104835ad6f0ccc7281a003e7d209f7d13a7a85f44bf7aacf5677235d06519653e7ec8a715a33ae8317a0c63e44438304c557aa63624df68ef412ad4b7a4d782ff19a95f0cc7761642ea08bb69ecdf28c4000232f45b37dc31a8fbafd5776b09497229700d88c26b66ea0495cfabdc100c1b4937c62b85f8f62b3128c8675a92ff6198f8720343e1530a05147e7ff1cbb503c61e4ceb30d10cc543661ed96899c3f0046fd0cb7144474a06df978dd6f550c9fb06ebf96ea5a38c3914a35cf686960a82f6a7a88f17d9da0a7c553d84a9adf8ef482473e75465a956a24f2247e04e5f135e137404c14f9a0f2768d49e89afc821d20fb9a35d947f2b516bd5975a7deed970026c85fe2d7a02344a89ff38c60c23921f3fc76e311214838bff662e3fc9cf36418db116f8ba0f224186c0665dfebe6e1dea42c6fc3eb86839d9e75ed1f47629946e0b8d4f5a02e68d32c7a789b94c1a1bc53f24431a09f67bb4b4dc76cc3c888039c59202fa00dc5770ad81b8ff60a21f5e8e568f0d26778e125b2669deb579e097baf439fa0c7cb928504b21a01a47a8484f1d78cf3b3b9e52722b7b717d0832f99f2079ba0f2af9a20c61f162ec24859152b85c3736771e51f3822d34212b79503cb8b59a0b067d56f400bdae175854af678351aa3fd9e4ff27ef9b984d1f73dd7c02dd5a058eb67b6576b3b226ebca4f3b0c6b60ba167ff72321bd3eb352abef010038da0c99ca5533b5a507aab856ba981b10596a6aadf92439c17fe6c2433bd67a6d9a055d813720a484833c071055cbf56b9102ddcde4ffe89112dd0970ff9ce5adda086ee954060d0f38cf26e9077e7ff25d73c1af93db1e0c2eb9846efb37a415aa08836969778ef01ddda1b93c830ba7519b07a7cebd994e99af4ea0042f661b1a0b436d7758847d374ded34dab1f5856087a8d35b02677edd5643bb5573307b1a0c1026ffe0c7b67a3b639d891775fb450dad4f91dd3feae4f11e962d26c9a31a0186539769581dde1a49f2559474e8dc1105bd95b433ac835e5f97863e73453a0984f9b4956b6072d43183e524339dd3b5ac4b51bd784a78fc44788039f5aa9a098d83d8288fb37f66babb78bf0fee30d6be5ecaad51b51d94cf2227b1f58dca0f114d123f62e19cbfd1b88c594c2e733c8ec258579ff099e02a497bbe21eb8a047e8f7c7b2543641b7f773de0e1c285327873434b29f6a9d830dfbc5e2135ba0b13e21bd1d705d4d43c700b8ad5b6123d5b1e7e3198eba6163f000cc5aeddda00f5af6bbd6557e4877cbfe2e12bebcb0ae9dfff04409c77d9fc59657552d17a04a3a6a5a22fbb59f88e580a9306e6f438fb08c4fcd452458160f144a285c1ca0c320fa6bd824df7ba618fd0fa2f01ad5974055a507d187548f1371c2d3c236a038d2ae4b0657d71425e15dd1945a350a7e4d22596fc9edf749fc248fa0eac7a0a5ce590bce1408becb707414ad774af351c85aab173c071aff2e6cc40c22bfa07f5dd2b9859c14b90e9fc1faf649ef00e76ced3536ba15d4efe8ba80e994c4a0c5fc47c67ccf99ef645d7d16381dde758617e598ca2eaee9de67a115bf6746a0a3f828d28b3e9aa32cb34654e548573836c507534f6b28c8db4485c524eb0aa0646bf3a156242f58e9654df16d2adcb215a4afe0090b25fc2613da45a26512a00993de3ece1e05bd91834d2a766a7ab3d8d5d32bb3c1330c08812df9a90174a0111b90f0657586070596c0273957f0f28fa046c9bf25b17fc5e8170e793bbba068335fa92775d5c67ab6c1cf757986d3254e5767e871a09fb7a521641e69d7a04c638fb90bcafea8c7e2286e56aa05f5169509fc32d25a210b6c8eb14fe9aba03b08dfd8e0a5f5ac5043739bab62ff681195a37e5c98e5bc2272a8bf61c1d0a04c69dd75cfa0e9e6e709d3f20a871fc8c90649c8457f0fea13190206159ccda04780718bc75b3527d59f06659660af15b5f8b9f3ed82e140960e33023ce7aea0b6ced64b692508071f9bdee66e8e666b5b861adc0334c47fe70fe2623c46cca00a5a838a549a42704f45a8b068e5b581e5896bd8ad72eadc931fdb3f809a5ba0e2e84efef29035e51c114bfe822322b7843d7ab824c4faeccce1213343497fa0fd1abb7977b5f4f4a7798f478082b0e4dfaf467a7b6824cf3c772d2b0423cfa0ba1dfcb43c5fcd4880e370497b77ac649fa2ed13852d294531508c98b5b6d0a06ebc54bac3ccf6e2646d4f6c37ddad8bdccddab2d65e72aeaf191ab7991b1ca0012cc18be35c1afeea418de806fe95debbb68a44d81ec7ce9937863963517aa07a9fb778e1f25ffd849d2a9508de10587b8ba92eacbbcda3963837ef5b6ebfa03edfd6aa67ed7f8bb857426f893a765aa84cb71556f7440a29a1f5383acaf2a0dd771c9b14807d22a6b2a878abdff078d36c5a43f1c67e94716f6765692e51a088561d65821117c1d95aed5b8435e60c1efe7709deeac177182a57e3734ca8a016a0d69eadcca041c8d213dce62f4d10ed6e5f6ef6c36dffa3d7547a8956e7a09cbf1b76d2e8214a03d2f4020aad5426557af260305189ac4498f03856a036a0d3e204bad2e49cce5708367d593d076c2b753e76d7cfd2b1e4b4d0b68f2878a048bc6aea74654e693c5a2110923079cbdddf030eecbb05e21cf0cbdb32571ea07fa2d194a171ed5a80a46b2537228dc1a47bae36e72f135f4bdec08531a4e8a0b74da9806d88c230b9e1df371c77041109f1a8e8e4ec726449c6d739e99a84a04e6a9024837089e0571ee2883624325de702181e790c286dfe2b0656f09d16a0de3fdd217320b479231b30726f59c98136ca9ae94a7f532431f473c2957c6da0ac47e2da65156aea936794d8a8583897779f783553253afafa3e4df71eeed5a020ed9a006e7a785cd9f6f5fa0fca7d49af3d94f759905227003f66dfd5a56aa0ab79d4a0849e34e3f508d8ddfc245257dc4564756f7b698363df3d2f5dc2f4a042003dbb3a1c6e206a7a26200c90ef366e3f07a1ba491e139b89d4c43d0f69a0903fb0fe2715ad8a01f5ffa747822eff627d32e2631e959d79cfb632238ad0a0e6540fb6f81be4310575a79f18c6ce7c17573acbe007dd1a5af20adec652a2a099795884015e66ead1492315a644c99fa8da13ae2c190875e936edaa9582b4a0225aab7098a8bd3c9626cd4b774bbee76de9987b65b0b5b49faa7a7d485c88a091695e069f68d1b69d76ad91b379ce1da4882d31d6888716ca945a12ad782fa0dd0d96e4f751fa1169bd164bbe953ffd2c1a427e158728835fb433099c0c9ba0e9fabde2a7374748fb47541ce3bde85b6a02b5eca83e10e636f2aee50fda83a03fe56293817c8e2e60b018eb54efd1acf0ad53174d504a4c473be2ab67b15fa0d115e8c024fbeba533f3eb857ceccf05f3d612efb475483d20b7d49b24b590a0630f87397a2f16020180b9091af724b0b78756a17262a6e6c21e31259a59dea07f4ee8258c9e47bc60196bd23923342fdc9ca58fea49be30445632846ae226a09d748502b0dc7296986c0d3f84104434f0aa62bc0765253914c4b90ae7e3cda01d3b7f36f2b153bdb22c16c8a89c2d1e20d09541de6690a484d9556a09acb7a001048a7bfe3b27ff6b96871f9b9fb1c22de6f7c37bf050af435644023f62daa0312bf1e97e3b8dd12b5a184a0d31f9570b894a0c044fb06427851c8c687e83a0c3918afdc193923d1c631504bb0d5142b9fbb7514e71ee877ff592422ec570a0b4635184e5980c6ac6f250b141dab34252ab886a467f12b124b0f29dbcefb7a01de5a543c7bfb00d24d786ccbf1e4c12d89693fa22ecd6397d1c915a4b557fa0603f0936d08cb3f5b2f1f86a98fcddb708a858b859a308cf3234a715b36a6aa000cef8e2bbbbf63556cf50f30f1e8252158907d4f98a6572dfc21773f333fda044e99dfa3798609cb3298a96c419a2d5ef332402188ab675cb3517fc6f4d27a0a3de90b02346073710c4a82df19f3af9c0aca8f6ed53953c56414135724980a0234bb791e05aaf9ec0f1c0a2490f019100cc0b63b8fddf8bc9eb316638d7e7a0b66f81c0a332e3871927512288443f9b0b7705498513fbaa70a80bee474d00a0d808819296dade3387f100653e4b375213fe0ff70cf3316ae743a7dec65025a09873b1a806beec1fa864332a62c8cb85e08d57e9224cc3ef5e4ec55be8dbc7a0eb47fd3584833f048fb23e1b12b17b3640d00204e129da1a1f3195fb59f3d4a0984442a8f4e180a97184011db79d90534c5b66208f502e95a39ec731f002cba0dc50eeca840f98d51e9d89adab2522385bea21e4b764cf392323fc85145e68a0de6b81d926fab4d10e2ba8334decc3477bb9323ec3675a240076bc5c230a79a008a4a7712f5f789ea5bce00fac9d426588510ffae0613426f664d5f3045c2ca06af412f02337038fae8d8ebf7bdbbba1f39c6a9bc58450b5b7bb8d0d04204aa0759eb22d4d43be41f1dd2dd4a763bcf2ca9f5f1d72ba93e6d3e9df6031dec4a088a5b33c479be4ec6d6c81c7ddefeea204d896d8505e3426fc17ea3df597d6a09ea896a77fd7e4e15d4c45d7ce8eead906f3325ab30e8695d87bd2376305baa01760c806dd3dfe90f91de0bdeefebcca997248e761be78de8211b50b31780aa0fe2f0172d0fa0270a43bb297d3abe4b6a9c4de78ea988c133d88f2a193baf1a0da9a3d3a93706674e28f738dc6ec0291f12cd94f53bed9a443cdf3c65790d2a0911501503ec51a41294ac84a20adc4db03dec8f7af889cae0f8ed8bf79144fa0671f1d37f9f445b831305376f7f2db395f3c89b101cde1075649052c66385fa0e0cdb053b4ab873f3832aabf09c23ce4f31195ec5bf820164379c218088eb2a0d2f6e44d1bcaaf23bf514380e5db61e4da9ee6b3070437eed514919c2e546ea04f97ec90148e67097903694c358af81e01dd958351a32f6de80d31ba9dd3e5a059243fe2e73b821a5f3c0d219d0a8662b6822278479209a15c0868fecc85e4a0c7d1dc6b31be09e6980bc89d095123979c8c973c33c5351eac6845f65b0290a0caca78f27d37e9268517da7a4b186c3076c111939b0e7a99b9247f0c07b650a0a1b8f59f52a49382d68755731d6ab23dbb6a2745765a9d08897cc38bbcfa2aa04f15b054a6a7166c200d917b9c9b0b63d3f14c04dabeb27402747ecc10fb42a06d96c75dc3d19253cb41cf988b292d50b89208d518a170907ddbcf020835f0a0c8c97fa7afcf5d4ecd236fad5c9cf3ee92c7b3f0837abce005ad397ce84d4ba07ca288d2d6160cd440f9398dbd6ec4a37763b04fd1d56ebb9a3bddb93862c9a0515020010e4fc0bfe9070500cb5bc8257dd9aed646dab4c9e97dd41cfd695ca007998046698b6e4f26e6a59022cc84bf211507ae55abe592194db291262819a092e510ea865939dbab11ad872bfd85e20f6cac78604f7651c975639bcfcfc2a086eb46e40a35c1ff4eeac8f669b69c7bfcd65924fb1977ec785327ca7c671ba00a7e0fa4097b873bbc2c435a5be67ad9a2980e5f6b069e02a37297149fbf16a04b4ca3a4157681197c297b98e38e539a24dc9f0378cf31660f6bc40fd6acaba0d9ecc82b91270c1abac8371dc6982a3b51fdfd9f7fab9c9f8358b7cfc21066a057057b29564f961fb9e9882af8e37c109e249754aae076d25b0cd09edae55aa0f05188f73088798c1bc881922cc050e6d45b484700ec2865d7738862984cd9a0263943b111d3312047680df819f13dd993c3867259da6e660f5509618633a0a0ffb35c6539517c3ca59d824ed5a99045135ea795abfd4527505a2b0cecfd89a0b501109d30e9be98589e4e6c1d0cd3ac0600d5f2ead4f7209c271df54700b9a0b8d4411ff7dd21b8e6633436eb5a30ef8fb54207bbbcd5208d8fc88bad7085a0ee69de8bf353bcbf37aab0341974bb61a25fead771a86079ce93a86151a0eea0298aed57bd8ce77b463ee43bef2d56cbf61a9804c1e15131a85641e0a64aeca0c73f2a1f04f605dc723d33e446dd4806ae995cd28acde5ef09894646a7ca6aa087293131cd9828696c203ad678a7d6297791011892d125a5a2cff2041fefd8a0e68776e8a9607db121605a783c7dca113a098e0483b72dc1a8d8dd7634b97fa04bf47db961832b595fd11ebf8f87effb64a4a76d3ad8f13c1d33a4d1e4c1c4a0e0e268d819927d93ed35463637dea80519595cda6a7459a742cfc3ea187ed5a03fe516f16613f54b4694cf7261a007200004e2ce2d2b934c06c25494c1ad0ea0efa5314f26a6f1c6b0bfe983119fc20656d4b4fc00b4d504e667a75d32adffa09e6993171133d6288d81ab9cc3b191d8b7cc3e341c51114264f356c54cb710a04a8f923e3a8022b91d610cf788c8831573ac4f654b4c27860d8fd0528a094ca09cbb124a6823c234486c547be845555429829e1ab73ddffc71b4201b148613a08b152f5688fb35d8eec0a48e998dd7b4e83cd206748c4818360ae8c7660905a02071c2e840cc32751256aee118d2004f75ef9f966ba3a000558a891a7b7527a0c75ec067c2b7b64158b1646dac7b85e818a935fc631eca0ac0ccc9cb621cd7a070ec06df45e5b4dbcfa13638504e5c81d3f9df47d0a4495956cc8a0688301ba0df3fe16099a91df961aa952908770dfd1c407b30af5bc6ea97e090d51a74b2a08113139322f2a4d8b2dcf4dc171ae69b9cf5da54180029a91d62572969e046a08ce34b47f96877316953dc6ec4837b4aa5908fb414f70830e6c14f6054e25fa0881d73404a42c7dccea25cecaf7bb928b2ea97a80e1787a42fd90c0109e9b1a0ec7b19d18720baa19ecdf6be64dc08193b1a3abadcb5e46dde7f7effd1144ca08f46404cc1f980f9aa53debf1ec7ee715a8a83b0dbb9354ab341ad0d16262ea077a8d1f791d4880b83a9773693f12c4109577797b104de2c691aa404cd4defa0e0b8cba9c1df52a2cf97ef52aa71690fd75cc298651061b4812c644457b76ba06fca5be259e8709d3a02c8eaf6ed5e2c638349bf539e8b256b965f6ea3752ba0a3222332731ee0dff804b1029a67930a5cdcda55c96db7dc67498e0c3d3f23a06dabdba2ce012ec158e11b7c64af50a19f73f2c02849b73432e8792099b8c5a00276599a6061e3e069ec55ff21324e52841bcd56feedbf8ea7761b798ef575a092c948d7b602c7e595f54f7d67030110a1049f6c73189fa9bb02afb55cafa9a03926cf30b6da1fc33f1fbac077504a001b5a49e77494c8296581cdbdc8e454a07544c7a95fd1a22b30bc9c5c52576a7744517fda26b4aceb6ab39e7c07afe8a064693c4f69066a4c9966d3e4be609cb3b52ce4f7da05abbfad1b71605c1151a03fa80da361da84d1ffefc087c2f8b003a13a8921f22075565fb85daf9014c2a0c4153445a96a6c09f517b166608d6ed4025cb9b19e8f867911604a32056035a0bdda761a25b21435ab8df23f9efdc713e7a87d6c2cc978f03cb8953f0c27dda055d9647d77f3c7d845c8b3441426206bb16ec8652185e9530d751d070b8080a0a26df5811d78c976d6b2dcd078e2dc71c52d198813d90d1eb885eb3d171362a07024861756c351aa2bec2885b72c397d0b0fe9f9d126f4be794b2adb016c65a08a133abf5ca24635994e1308f9ccb40d506ec6e38c1de37921d03877fb4f89a01ff4e74f743a779f0a749b5a39c66faa35928519c7df2e2275187669edda05a01f60c9767fa888c2ec1fa5bb8ece4fb0fb6fd36c28628e2cea44fdbc02abf1a0b5630680f8d36087784ae979c3449a55ab19eaed1b618230dfad2ba6260e09a08ffdfb14783667332a1b710eb948e69c65a36de4e93a4a2fe446059b47e6f1a08f0df92144cf499a9187d5ae9f374d50a215c6993782248e81664e8080c025a02153261821e215708e96649b801427f1f6fbd16fe34b1ac9afdab033827e77a0c71fc8830f9caa835e59092cd560d587ba990483f9eb7dd2d25ebeaf14a2daa0617084b488dff41ae642255b5d371acbf89add1264759bf7023cadda8c9756a0aa1dd8ecc91faf4fff9b19580cbd41f5db60d386be26436d90b8d42854bbbfa09e278e360d5b1dd0ccf8623cda897cffb0c70701734c62b2ae3080f9b5f7bca05bb628b4e087ad128ca11ef767b6fe092a9faf610bfad0e22554ccf11d0243a07ae710807f14b117f3c482ce877ef5f5893d123e47680fc97bdc17f587c75ea024826d00236b182df19e1c7e962cb4a740282882f0153c7f16f897f6269ed0a085995dd6c93347df9454b38b7cd0147798b946f938aaba06a5bb0718259170a0d242850dbbafa7f82cb51ac80a0cbcd28373f665ed3395696f6e4d03512eeda0eb1420a56f54f97a53badb4557cd3585e9b08e98c075f9cccfdc531911276da04da434ab9db5f071b9023015cbe22dbd698064e8e7881f28f434f94d83ea5fa09d1766bd2ca75d254571f2d5e0ad43b0f8b4febc52e134f628043d0ce7db2fa0ec18cfdd8b04ea9f707548a6072bd901fab6b0e9c1bbf0d26e9fa86c6f6a0aa07d04639211689f1245d324e06038af033d2ae5b9330beab1809520a06358b3a07b66d5726c5656c6edcaf9bbb2c54ea08c27d2d3b547eb3f2b7895176cbb6da0e4d3cb4f351981c1fea16deaf7c9581b986835e6e71f8caeb0e657b5408840a0ef0bd37bad78f985e7992e91e2559c5f423cf758c47354c1c97dac9e6052d8a0ef992fb34966ac0f067fd9949fa86ebef13798c9f63448662e529d33991723a063cd92d5b7c4fb24d796b1cf565a8a61292a131af968b9afaa657142c4e88ba091a4badd888c8aa0571293e6b3fe09a0fd10fe066270b68fa5c9ad5c1fea8ca07e10d9347e20f00f5e1b3efbc9cb54b91635868d4f504f84385781d8198575000000000000000000000000000004000000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cbf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f000000000000000000000000000000058a18d469e7fb5bee5565b2ddf1aee386d7ee230313b62a75058ba9c2109bc60e39a18c1ee8f536863f8c00e8fa2739d98dd8d35a223db3a7dc152b2a4d6f33bd3640ede8ab04c26564e3d36ca57f141e56c4618564ecf7aee92dc04228ba1316072f786872a64283055694184a18343f597f3bea3e9460522d2928a1f5e17d241d5485ead7e891a5d12df9058cbab395e7c25053cd15d0cfd870a67693e3e05600000000000000000000000000000005610dc73641212634282375feb4448198dfa1a870ecb6ccb678c4b31c60be410f49a286918e6edcd396e4cc992dd6a535cc48c94fd0c0a06435221f6c8e848e060c82e6e60176780ceda112b939fb98d92dce94c401c07043730511c21cc9c1054a7820fdbf28a2456ddb054dfb9e09cd168f61f3d27704a86e26f41dc137940d5564132847e206272d97dd22921e3cbc487d87a80595f2a6441adf8acd70520f".from_hex().unwrap();
        let transfer_result = contract.execute(&mut submit_bundle_ext, &submit_bundle_payload);
        assert!(transfer_result.status_code == ExecStatus::Failure);

        for b in &transfers {
            assert_eq!(submit_bundle_ext.balance(&b.get_recipient()), U256::from(0));
        }
        assert_eq!(
            submit_bundle_ext.balance(&*CONTRACT_ADDRESS),
            U256::from(1024)
        );
        assert_eq!(submit_bundle_ext.get_logs().len(), 0);
    }

    #[test]
    fn atb_test_already_submitted_bundle() {
        let mut state = get_temp_state();
        let mut substate = Substate::new();
        let contract = get_contract();
        {
            let mut ext = get_ext_default(&mut state, &mut substate);

            let ring_initialize_payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
            let result = contract.execute(&mut ext, &ring_initialize_payload);
            assert!(result.status_code == ExecStatus::Success);

            // set relayer
            let call_payload =
                "6548e9bca092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
                    .from_hex()
                    .unwrap();
            let call_result = contract.execute(&mut ext, &call_payload);
            assert!(call_result.status_code == ExecStatus::Success);

            let _add_result = ext.add_balance(&*CONTRACT_ADDRESS, &U256::from(10));
        }
        // we create a new token bridge contract here because we
        // need to change the execution context
        let mut submit_bundle_ext = BuiltinExtImpl::new(
            &mut state,
            BuiltinContext {
                sender: MEMBERS[0].address(),
                address: *CONTRACT_ADDRESS,
                tx_hash: *TX_HASH,
                origin_tx_hash: H256::default(),
            },
            &mut substate,
        );
        // assemble the payload
        let block_hash = H256(Blake2b::hash_256("blockHash".as_bytes()));
        let mut transfers = Vec::with_capacity(10);
        for i in 0..10 {
            // generate a unique sourceTransactionHash for each transfer
            let source_transaction_hash =
                H256(Blake2b::hash_256(format!("{}", i).into_bytes().as_slice()));
            let recipient = public_to_address_ed25519(&H256::from_slice(&Blake2b::hash_256(
                format!("{:x}", i).into_bytes().as_slice(),
            )));
            let transfer = get_instance(BigInt::from(1), recipient, source_transaction_hash);
            transfers.push(transfer.unwrap());
        }

        let payload_hash = compute_bundle_hash(block_hash, &transfers);

        // ATB-4.1 in order to test, we pretend that a bundle is already complete
        contract
            .connector
            .set_bundle(&mut submit_bundle_ext, payload_hash, *TX_HASH);

        let call_payload = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef87500000000000000000000000000000080000000000000000000000000000001d000000000000000000000000000000320000000000000000000000000000003d000000000000000000000000000000480000000000000000000000000000005300000000000000000000000000000000a0fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d67bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e90a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a20000000000000000000000000000000aa08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205eda0d12a94f45cf9e0524069746acd51ab53a3190ea520bbcaf58329ab5c8626b4a0d8ef096f4f07a57163bdd28728f2a5d4689cd8c7089618223f567e30a95e33a017063ae510ba37ff55f5fa533c6d953a33f6a5252feaa35d45b260dda8c5f2a0614182bec524d648f7dba8a686037f0f535f52f6d3a3b031ef6fc78bc4c641a0b90da4145ddddbe57b2839bf4ca2ab966a5c1e66db2eb977f26c90621bc1820000000000000000000000000000000a0000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cbf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f0000000000000000000000000000000575312f45d7791c702bfe38f056287fb8b64514cbc77f76f17b421ec3451a25eda66e2fdb4b7a13dd48c9953c12bc8ea7f2f09f1e0dd1c6a01e5b1557964826f6760f529c41c666bb07c0b492ae9700c8f0a46783006ca140905413c5f14703fe1831b6f93aa1efddcb49408edaabbebf688b5580b5f58f41c19a3e0ffe39aec40383882b5f0b02053706cbb8265991d1ce70c140e6480422d8e7a70cb7fff54700000000000000000000000000000005b57dba44d5107751d16daf30a9e186730197e4d12fbeeff57d335c80e7f91e04c386d3fa9f0336a3900a11be4788f71d58a9782525c11fd4e10c4dccdf03340ae50ed9967d016e2ee4b2fade9baaa818291a0047da50aa3dd54e71bba48f20056aaaaa8e915d3cea4c9d84721489d5c568468da5f80ac3e0434b55cef0d23c001a4439c6c23fbff79c27dd7d7edbaae0743b469a48a2ee4a65374606a5d6c203".from_hex().unwrap();
        let transfer_result = contract.execute(&mut submit_bundle_ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);

        let logs = submit_bundle_ext.get_logs();
        assert_eq!(logs.len(), 1);

        // ATB 4.1 check that proper event was emit
        assert_eq!(
            logs[0].topics[0],
            H256(BridgeEventSig::SuccessfulTxHash.hash())
        );
        assert_eq!(logs[0].topics[1], *TX_HASH);
    }

    #[test]
    fn transfer_ring_locked() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let builtin_params = BuiltinParams {
            activate_at: 0,
            deactivate_at: None,
            name: String::from("atb"),
            owner_address: Some(
                "a008d7b29e8d1f4bfab428adce89dc219c4714b2c6bf3fd1131b688f9ad804aa"
                    .parse()
                    .unwrap(),
            ),
            contract_address: Some(
                "0000000000000000000000000000000000000000000000000000000000000200"
                    .parse()
                    .unwrap(),
            ),
        };
        let contract = TokenBridgeContract::new(builtin_params.clone());
        let mut ext = BuiltinExtImpl::new(
            &mut state,
            BuiltinContext {
                sender: "a008d7b29e8d1f4bfab428adce89dc219c4714b2c6bf3fd1131b688f9ad804aa"
                    .parse()
                    .unwrap(),
                address: "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .unwrap(),
                tx_hash: H256::default(),
                origin_tx_hash: H256::default(),
            },
            &mut substate,
        );

        let input_data = "6548e9bca092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
            .from_hex()
            .unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let my_contract_address =
            "0000000000000000000000000000000000000000000000000000000000000200"
                .parse()
                .unwrap();
        let _res = ext.add_balance(&my_contract_address, &U256::from(10));

        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero());

        let mut submit_bundle_params = builtin_params.clone();
        submit_bundle_params.owner_address = Some(address);

        let contract = TokenBridgeContract::new(submit_bundle_params);
        let block_hash = blake2b("blockHash".as_bytes());
        let receipts: [H256; 5] = [
            contract_address(&sender, &U256::from(0)),
            contract_address(&sender, &U256::from(1)),
            contract_address(&sender, &U256::from(2)),
            contract_address(&sender, &U256::from(3)),
            contract_address(&sender, &U256::from(4)),
        ];
        let tx_hashes: [H256; 5] = [
            H256::from(0),
            H256::from(1),
            H256::from(2),
            H256::from(3),
            H256::from(4),
        ];
        let transfers = vec![
            bridge_transfer::get_instance(BigInt::from(1), receipts[0], tx_hashes[0]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[1], tx_hashes[1]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[2], tx_hashes[2]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[3], tx_hashes[3]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[4], tx_hashes[4]).unwrap(),
        ];

        let payload_hash = bridge_utilities::compute_bundle_hash(block_hash, &transfers);
        let mut input_data = Vec::new();
        input_data.extend_from_slice(&bridge_func_sig::BridgeFuncSig::ActionMap.hash());
        input_data.extend_from_slice(&<[u8; 32]>::from(U256::from(payload_hash)));
        let res = contract.execute(&mut ext, input_data.as_slice());
        println!("res = {:?}", res);

        input_data = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef8750000000000000000000000000000008000000000000000000000000000000130000000000000000000000000000001e000000000000000000000000000000240000000000000000000000000000002f0000000000000000000000000000003a0000000000000000000000000000000050fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d00000000000000000000000000000005a08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205ed00000000000000000000000000000005000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cbf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f0000000000000000000000000000000555f9d6f28dbee63cd738631971ce66caf87504f214d75673763da34e7ac15b8eda0133664a514a0ba275215e630c2b565f357dade4fffacb1433c9b1cce9301e441bb58757ad98dfb4c9a7c44315ba44f19c3329ea0e3b86a4733a49c2e2fcf4d9375aa0147713cb6fdbbd7085b56bac20b2597c62551852135e0f6efef48cf0537670a43cc4fc126fff219b028f38bdddcd065163f7c25b0f7f1d389756adaf00000000000000000000000000000005d92ea1d875326f507286266d4ffc5d10d99e9c856f3b9a7cbe95fab56f67220bc8aa88f651d728f15bfcf2d5aa93b8f2825f24d8d434f1e2a214c113c30e9900573de3493600a37e6508849814d73aa4f9415f71949b955f70b8ce7181037c05419c4ae914ef1c9b69433bff38862ebf2324773c583e21207a5697c997ff7d010809b59b1f332d1df4ffac67404ab70e84c932b31c064955909b6c73fadc0b04".from_hex().unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Failure);

        for i in 0..5 {
            assert_eq!(ext.balance(&transfers[i].get_recipient()), U256::from(0));
        }

        assert_eq!(ext.balance(&my_contract_address), U256::from(10));
        assert!(ext.get_logs().is_empty());
    }

    #[test]
    fn test_transfer_invalid_relayer() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let builtin_params = BuiltinParams {
            activate_at: 0,
            deactivate_at: None,
            name: String::from("atb"),
            owner_address: Some(
                "a008d7b29e8d1f4bfab428adce89dc219c4714b2c6bf3fd1131b688f9ad804aa"
                    .parse()
                    .unwrap(),
            ),
            contract_address: Some(
                "0000000000000000000000000000000000000000000000000000000000000200"
                    .parse()
                    .unwrap(),
            ),
        };
        let contract = TokenBridgeContract::new(builtin_params.clone());
        let mut ext = BuiltinExtImpl::new(
            &mut state,
            BuiltinContext {
                sender: "a008d7b29e8d1f4bfab428adce89dc219c4714b2c6bf3fd1131b688f9ad804aa"
                    .parse()
                    .unwrap(),
                address: "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .unwrap(),
                tx_hash: H256::default(),
                origin_tx_hash: H256::default(),
            },
            &mut substate,
        );

        let input_data = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let my_contract_address =
            "0000000000000000000000000000000000000000000000000000000000000200"
                .parse()
                .unwrap();
        let _res = ext.add_balance(&my_contract_address, &U256::from(10));

        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero());

        let mut submit_bundle_params = builtin_params.clone();
        submit_bundle_params.owner_address = Some(address);

        let contract = TokenBridgeContract::new(submit_bundle_params);
        let block_hash = blake2b("blockHash".as_bytes());

        let receipts: [H256; 10] = [
            contract_address(&sender, &U256::from(0)),
            contract_address(&sender, &U256::from(1)),
            contract_address(&sender, &U256::from(2)),
            contract_address(&sender, &U256::from(3)),
            contract_address(&sender, &U256::from(4)),
            contract_address(&sender, &U256::from(5)),
            contract_address(&sender, &U256::from(6)),
            contract_address(&sender, &U256::from(7)),
            contract_address(&sender, &U256::from(8)),
            contract_address(&sender, &U256::from(9)),
        ];
        let tx_hashes: [H256; 10] = [
            H256::from(0),
            H256::from(1),
            H256::from(2),
            H256::from(3),
            H256::from(4),
            H256::from(5),
            H256::from(6),
            H256::from(7),
            H256::from(8),
            H256::from(9),
        ];
        let transfers = vec![
            bridge_transfer::get_instance(BigInt::from(1), receipts[0], tx_hashes[0]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[1], tx_hashes[1]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[2], tx_hashes[2]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[3], tx_hashes[3]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[4], tx_hashes[4]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[5], tx_hashes[5]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[6], tx_hashes[6]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[7], tx_hashes[7]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[8], tx_hashes[8]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[9], tx_hashes[9]).unwrap(),
        ];

        let payload_hash = bridge_utilities::compute_bundle_hash(block_hash, &transfers);

        let mut input_data = Vec::new();
        input_data.extend_from_slice(&bridge_func_sig::BridgeFuncSig::ActionMap.hash());
        input_data.extend_from_slice(&<[u8; 32]>::from(U256::from(payload_hash)));
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let input = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef87500000000000000000000000000000080000000000000000000000000000001d000000000000000000000000000000320000000000000000000000000000003d000000000000000000000000000000480000000000000000000000000000005300000000000000000000000000000000a0fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d67bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e90a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a20000000000000000000000000000000aa08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205eda0d12a94f45cf9e0524069746acd51ab53a3190ea520bbcaf58329ab5c8626b4a0d8ef096f4f07a57163bdd28728f2a5d4689cd8c7089618223f567e30a95e33a017063ae510ba37ff55f5fa533c6d953a33f6a5252feaa35d45b260dda8c5f2a0614182bec524d648f7dba8a686037f0f535f52f6d3a3b031ef6fc78bc4c641a0b90da4145ddddbe57b2839bf4ca2ab966a5c1e66db2eb977f26c90621bc1820000000000000000000000000000000a0000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cbf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f0000000000000000000000000000000575312f45d7791c702bfe38f056287fb8b64514cbc77f76f17b421ec3451a25eda66e2fdb4b7a13dd48c9953c12bc8ea7f2f09f1e0dd1c6a01e5b1557964826f6760f529c41c666bb07c0b492ae9700c8f0a46783006ca140905413c5f14703fe1831b6f93aa1efddcb49408edaabbebf688b5580b5f58f41c19a3e0ffe39aec40383882b5f0b02053706cbb8265991d1ce70c140e6480422d8e7a70cb7fff54700000000000000000000000000000005b57dba44d5107751d16daf30a9e186730197e4d12fbeeff57d335c80e7f91e04c386d3fa9f0336a3900a11be4788f71d58a9782525c11fd4e10c4dccdf03340ae50ed9967d016e2ee4b2fade9baaa818291a0047da50aa3dd54e71bba48f20056aaaaa8e915d3cea4c9d84721489d5c568468da5f80ac3e0434b55cef0d23c001a4439c6c23fbff79c27dd7d7edbaae0743b469a48a2ee4a65374606a5d6c203".from_hex().unwrap();
        let tx_res = contract.execute(&mut ext, input.as_slice());
        assert!(tx_res.status_code == ExecStatus::Failure);

        for i in 0..10 {
            assert_eq!(ext.balance(&transfers[i].get_recipient()), U256::from(0));
        }

        assert_eq!(ext.balance(&my_contract_address), U256::from(10));
        assert!(ext.get_logs().is_empty());
    }

    #[test]
    fn test_transfer_less_than_minimum_required_validators() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let builtin_params = BuiltinParams {
            activate_at: 0,
            deactivate_at: None,
            name: String::from("atb"),
            owner_address: Some(
                "a008d7b29e8d1f4bfab428adce89dc219c4714b2c6bf3fd1131b688f9ad804aa"
                    .parse()
                    .unwrap(),
            ),
            contract_address: Some(
                "0000000000000000000000000000000000000000000000000000000000000200"
                    .parse()
                    .unwrap(),
            ),
        };
        let contract = TokenBridgeContract::new(builtin_params.clone());
        let mut ext = BuiltinExtImpl::new(
            &mut state,
            BuiltinContext {
                sender: "a008d7b29e8d1f4bfab428adce89dc219c4714b2c6bf3fd1131b688f9ad804aa"
                    .parse()
                    .unwrap(),
                address: "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .unwrap(),
                tx_hash: H256::default(),
                origin_tx_hash: H256::default(),
            },
            &mut substate,
        );

        let input_data = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let input_data = "6548e9bca092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
            .from_hex()
            .unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let my_contract_address =
            "0000000000000000000000000000000000000000000000000000000000000200"
                .parse()
                .unwrap();
        let _res = ext.add_balance(&my_contract_address, &U256::from(10));

        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero());

        let mut submit_bundle_params = builtin_params.clone();
        submit_bundle_params.owner_address = Some(address);

        let contract = TokenBridgeContract::new(submit_bundle_params);
        let block_hash = blake2b("blockHash".as_bytes());
        let receipts: [H256; 5] = [
            contract_address(&sender, &U256::from(0)),
            contract_address(&sender, &U256::from(1)),
            contract_address(&sender, &U256::from(2)),
            contract_address(&sender, &U256::from(3)),
            contract_address(&sender, &U256::from(4)),
        ];
        let tx_hashes: [H256; 5] = [
            H256::from(0),
            H256::from(1),
            H256::from(2),
            H256::from(3),
            H256::from(4),
        ];
        let transfers = vec![
            bridge_transfer::get_instance(BigInt::from(1), receipts[0], tx_hashes[0]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[1], tx_hashes[1]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[2], tx_hashes[2]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[3], tx_hashes[3]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[4], tx_hashes[4]).unwrap(),
        ];

        let payload_hash = bridge_utilities::compute_bundle_hash(block_hash, &transfers);
        let mut input_data = Vec::new();
        input_data.extend_from_slice(&bridge_func_sig::BridgeFuncSig::ActionMap.hash());
        input_data.extend_from_slice(&<[u8; 32]>::from(U256::from(payload_hash)));
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let input_data = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef8750000000000000000000000000000008000000000000000000000000000000130000000000000000000000000000001e00000000000000000000000000000024000000000000000000000000000000290000000000000000000000000000002e0000000000000000000000000000000050fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d00000000000000000000000000000005a08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205ed00000000000000000000000000000005000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000002f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cb0000000000000000000000000000000255f9d6f28dbee63cd738631971ce66caf87504f214d75673763da34e7ac15b8eda0133664a514a0ba275215e630c2b565f357dade4fffacb1433c9b1cce9301e00000000000000000000000000000002d92ea1d875326f507286266d4ffc5d10d99e9c856f3b9a7cbe95fab56f67220bc8aa88f651d728f15bfcf2d5aa93b8f2825f24d8d434f1e2a214c113c30e9900".from_hex().unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Failure);

        for tx in transfers {
            assert_eq!(ext.balance(&tx.get_recipient()), U256::from(0));
        }

        assert_eq!(ext.balance(&my_contract_address), U256::from(10));
        assert!(ext.get_logs().is_empty());
    }

    #[test]
    fn test_transfer_insufficient_validator_signatures() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let builtin_params = BuiltinParams {
            activate_at: 0,
            deactivate_at: None,
            name: String::from("atb"),
            owner_address: Some(
                "a008d7b29e8d1f4bfab428adce89dc219c4714b2c6bf3fd1131b688f9ad804aa"
                    .parse()
                    .unwrap(),
            ),
            contract_address: Some(
                "0000000000000000000000000000000000000000000000000000000000000200"
                    .parse()
                    .unwrap(),
            ),
        };
        let contract = TokenBridgeContract::new(builtin_params.clone());
        let mut ext = BuiltinExtImpl::new(
            &mut state,
            BuiltinContext {
                sender: "a008d7b29e8d1f4bfab428adce89dc219c4714b2c6bf3fd1131b688f9ad804aa"
                    .parse()
                    .unwrap(),
                address: "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .unwrap(),
                tx_hash: H256::default(),
                origin_tx_hash: H256::default(),
            },
            &mut substate,
        );

        let input_data = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let input_data = "6548e9bca092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
            .from_hex()
            .unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let my_contract_address =
            "0000000000000000000000000000000000000000000000000000000000000200"
                .parse()
                .unwrap();
        let _res = ext.add_balance(&my_contract_address, &U256::from(10));

        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero());

        let mut submit_bundle_params = builtin_params.clone();
        submit_bundle_params.owner_address = Some(address);

        let contract = TokenBridgeContract::new(submit_bundle_params);
        let block_hash = blake2b("blockHash".as_bytes());
        let receipts: [H256; 5] = [
            contract_address(&sender, &U256::from(0)),
            contract_address(&sender, &U256::from(1)),
            contract_address(&sender, &U256::from(2)),
            contract_address(&sender, &U256::from(3)),
            contract_address(&sender, &U256::from(4)),
        ];
        let tx_hashes: [H256; 5] = [
            H256::from(0),
            H256::from(1),
            H256::from(2),
            H256::from(3),
            H256::from(4),
        ];
        let transfers = vec![
            bridge_transfer::get_instance(BigInt::from(1), receipts[0], tx_hashes[0]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[1], tx_hashes[1]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[2], tx_hashes[2]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[3], tx_hashes[3]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[4], tx_hashes[4]).unwrap(),
        ];

        let payload_hash = bridge_utilities::compute_bundle_hash(block_hash, &transfers);
        let mut input_data = Vec::new();
        input_data.extend_from_slice(&bridge_func_sig::BridgeFuncSig::ActionMap.hash());
        input_data.extend_from_slice(&<[u8; 32]>::from(U256::from(payload_hash)));
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let input_data = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef8750000000000000000000000000000008000000000000000000000000000000130000000000000000000000000000001e000000000000000000000000000000240000000000000000000000000000002b000000000000000000000000000000320000000000000000000000000000000050fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d00000000000000000000000000000005a08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205ed00000000000000000000000000000005000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000003f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cb1c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cb00000000000000000000000000000003f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cb1c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cb00000000000000000000000000000003f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cb1c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cb".from_hex().unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Failure);

        for tx in transfers {
            assert_eq!(ext.balance(&tx.get_recipient()), U256::from(0));
        }

        assert_eq!(ext.balance(&my_contract_address), U256::from(10));
        assert!(ext.get_logs().is_empty());
    }

    #[test]
    fn test_transfer_out_of_bounds_list_meta() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let builtin_params = BuiltinParams {
            activate_at: 0,
            deactivate_at: None,
            name: String::from("atb"),
            owner_address: Some(
                "a008d7b29e8d1f4bfab428adce89dc219c4714b2c6bf3fd1131b688f9ad804aa"
                    .parse()
                    .unwrap(),
            ),
            contract_address: Some(
                "0000000000000000000000000000000000000000000000000000000000000200"
                    .parse()
                    .unwrap(),
            ),
        };
        let contract = TokenBridgeContract::new(builtin_params.clone());
        let mut ext = BuiltinExtImpl::new(
            &mut state,
            BuiltinContext {
                sender: "a008d7b29e8d1f4bfab428adce89dc219c4714b2c6bf3fd1131b688f9ad804aa"
                    .parse()
                    .unwrap(),
                address: "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .unwrap(),
                tx_hash: H256::default(),
                origin_tx_hash: H256::default(),
            },
            &mut substate,
        );

        let input_data = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let input_data = "6548e9bca092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
            .from_hex()
            .unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let my_contract_address =
            "0000000000000000000000000000000000000000000000000000000000000200"
                .parse()
                .unwrap();
        let _res = ext.add_balance(&my_contract_address, &U256::from(10));

        let sender = Address::from_slice(b"cd1722f3947def4cf144679da39c4c32bdc35681");
        let address = contract_address(&sender, &U256::zero());

        let mut submit_bundle_params = builtin_params.clone();
        submit_bundle_params.owner_address = Some(address);

        let contract = TokenBridgeContract::new(submit_bundle_params);
        let block_hash = blake2b("blockHash".as_bytes());
        let receipts: [H256; 5] = [
            contract_address(&sender, &U256::from(0)),
            contract_address(&sender, &U256::from(1)),
            contract_address(&sender, &U256::from(2)),
            contract_address(&sender, &U256::from(3)),
            contract_address(&sender, &U256::from(4)),
        ];
        let tx_hashes: [H256; 5] = [
            H256::from(0),
            H256::from(1),
            H256::from(2),
            H256::from(3),
            H256::from(4),
        ];
        let transfers = vec![
            bridge_transfer::get_instance(BigInt::from(1), receipts[0], tx_hashes[0]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[1], tx_hashes[1]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[2], tx_hashes[2]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[3], tx_hashes[3]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[4], tx_hashes[4]).unwrap(),
        ];

        let payload_hash = bridge_utilities::compute_bundle_hash(block_hash, &transfers);
        let mut input_data = Vec::new();
        input_data.extend_from_slice(&bridge_func_sig::BridgeFuncSig::ActionMap.hash());
        input_data.extend_from_slice(&<[u8; 32]>::from(U256::from(payload_hash)));
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let input_data = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef8757272720000000000000000000000008000000000000000000000000000000130000000000000000000000000000001e000000000000000000000000000000240000000000000000000000000000002f0000000000000000000000000000003a0000000000000000000000000000000050fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d00000000000000000000000000000005a08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205ed00000000000000000000000000000005000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cbf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f0000000000000000000000000000000555f9d6f28dbee63cd738631971ce66caf87504f214d75673763da34e7ac15b8eda0133664a514a0ba275215e630c2b565f357dade4fffacb1433c9b1cce9301e441bb58757ad98dfb4c9a7c44315ba44f19c3329ea0e3b86a4733a49c2e2fcf4d9375aa0147713cb6fdbbd7085b56bac20b2597c62551852135e0f6efef48cf0537670a43cc4fc126fff219b028f38bdddcd065163f7c25b0f7f1d389756adaf00000000000000000000000000000005d92ea1d875326f507286266d4ffc5d10d99e9c856f3b9a7cbe95fab56f67220bc8aa88f651d728f15bfcf2d5aa93b8f2825f24d8d434f1e2a214c113c30e9900573de3493600a37e6508849814d73aa4f9415f71949b955f70b8ce7181037c05419c4ae914ef1c9b69433bff38862ebf2324773c583e21207a5697c997ff7d010809b59b1f332d1df4ffac67404ab70e84c932b31c064955909b6c73fadc0b04".from_hex().unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Failure);

        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!((*res.return_data).is_empty(), true);

        for tx in transfers {
            assert_eq!(ext.balance(&tx.get_recipient()), U256::from(0));
        }

        assert_eq!(ext.balance(&my_contract_address), U256::from(10));
        assert!(ext.get_logs().is_empty());
    }

    #[test]
    fn test_transfer_to_same_address_twice_in_one_bundle() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let builtin_params = BuiltinParams {
            activate_at: 0,
            deactivate_at: None,
            name: String::from("atb"),
            owner_address: Some(
                "a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
                    .parse()
                    .unwrap(),
            ),
            contract_address: Some(
                "0000000000000000000000000000000000000000000000000000000000000200"
                    .parse()
                    .unwrap(),
            ),
        };
        let contract = TokenBridgeContract::new(builtin_params.clone());
        let mut ext = BuiltinExtImpl::new(
            &mut state,
            BuiltinContext {
                sender: "a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
                    .parse()
                    .unwrap(),
                address: "0000000000000000000000000000000000000000000000000000000000000200"
                    .parse()
                    .unwrap(),
                tx_hash: H256::default(),
                origin_tx_hash: H256::default(),
            },
            &mut substate,
        );

        let input_data = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let input_data = "6548e9bca092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
            .from_hex()
            .unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let my_contract_address =
            "0000000000000000000000000000000000000000000000000000000000000200"
                .parse()
                .unwrap();
        let _res = ext.add_balance(&my_contract_address, &U256::from(10));

        let mut submit_bundle_params = builtin_params.clone();
        submit_bundle_params.owner_address = Some(H256::from(
            "a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
                .from_hex()
                .unwrap()
                .as_slice(),
        ));

        let contract = TokenBridgeContract::new(submit_bundle_params);

        let receipts: [H256; 10] = [
            "a08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1b"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "a0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205ed"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "a0d12a94f45cf9e0524069746acd51ab53a3190ea520bbcaf58329ab5c8626b4"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "a0d8ef096f4f07a57163bdd28728f2a5d4689cd8c7089618223f567e30a95e33"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "a017063ae510ba37ff55f5fa533c6d953a33f6a5252feaa35d45b260dda8c5f2"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "a0614182bec524d648f7dba8a686037f0f535f52f6d3a3b031ef6fc78bc4c641"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "a0b90da4145ddddbe57b2839bf4ca2ab966a5c1e66db2eb977f26c90621bc182"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
        ];
        let tx_hashes: [H256; 10] = [
            "0fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb8"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "92cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(), // send to the same addr more than once
            "31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d6"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "7bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e9"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "0a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
            "bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a2"
                .from_hex()
                .unwrap()
                .as_slice()
                .into(),
        ];
        let transfers = vec![
            bridge_transfer::get_instance(BigInt::from(1), receipts[0], tx_hashes[0]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[1], tx_hashes[1]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[2], tx_hashes[2]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[3], tx_hashes[3]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[4], tx_hashes[4]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[5], tx_hashes[5]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[6], tx_hashes[6]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[7], tx_hashes[7]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[8], tx_hashes[8]).unwrap(),
            bridge_transfer::get_instance(BigInt::from(1), receipts[9], tx_hashes[9]).unwrap(),
        ];

        assert_eq!(receipts[2], receipts[3]);

        let input_data = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef87500000000000000000000000000000080000000000000000000000000000001d000000000000000000000000000000320000000000000000000000000000003d000000000000000000000000000000480000000000000000000000000000005300000000000000000000000000000000a0fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925eceb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d67bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e90a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a20000000000000000000000000000000aa08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205eda0d12a94f45cf9e0524069746acd51ab53a3190ea520bbcaf58329ab5c8626b4a0d8ef096f4f07a57163bdd28728f2a5d4689cd8c7089618223f567e30a95e33a017063ae510ba37ff55f5fa533c6d953a33f6a5252feaa35d45b260dda8c5f2a0614182bec524d648f7dba8a686037f0f535f52f6d3a3b031ef6fc78bc4c641a0b90da4145ddddbe57b2839bf4ca2ab966a5c1e66db2eb977f26c90621bc1820000000000000000000000000000000a0000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cbf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f00000000000000000000000000000005a325a291ccfbfed5882f356a8b9986d3eb016dbb124a4cdbc2fd575b81986d69b97b1adbfd788c05a31743c7a1992b75ab5242eee971691b68947729d5218c7374cd0085821abd375365c7f7ed45eeb6ea213f5efae97d34fbf54419f322e70d769f8cc80d2dee54f1b9021fdf5e36f6936626e2cec650c099a31f2b4776487bdf2fc3b8f3d36f9fda86669c52e7f7524f56f1aed7e7efeb5b8164c644033e13000000000000000000000000000000057d1ccaf21a86703e6bd2e426c6ca2275c167b7476714d7726a9981efb25ece05a5adc095b37eb608f7639c75a26d130b9278ba13806bf5a98b1187d174ee4702b7ce08aa40e7e1d515cdfba320f22d95617beeb3dda2992a3cb6488e4a4ff909df32a2bc7b42d425364afb922f91eb6fa9cb14074d7eb87a1edc6e01afaa310e5f6a6db2811e43f065db2ef3c3c8d83ffda4a8d8a9c3ed51d98322edf6d62e0c".from_hex().unwrap();
        let res = contract.execute(&mut ext, input_data.as_slice());
        assert!(res.status_code == ExecStatus::Success);

        let my_contract_address =
            "0000000000000000000000000000000000000000000000000000000000000200"
                .parse()
                .unwrap();
        let block_hash = blake2b("blockHash".as_bytes());
        let payload_hash = bridge_utilities::compute_bundle_hash(block_hash, &transfers);

        for i in 0..10 {
            if i == 2 || i == 3 {
                assert_eq!(ext.balance(&transfers[i].get_recipient()), U256::from(2));
            } else {
                assert_eq!(ext.balance(&transfers[i].get_recipient()), U256::from(1));
            }
        }

        assert_eq!(ext.balance(&my_contract_address), U256::from(0));
        assert_eq!(ext.get_logs().len(), 11);

        for idx in 0..ext.get_logs().len() {
            assert_eq!(ext.get_logs()[idx].address, my_contract_address);
            if idx == 10 {
                assert_eq!(
                    ext.get_logs()[idx].topics[0],
                    bridge_event_sig::BridgeEventSig::ProcessedBundle
                        .hash()
                        .into()
                );
                assert_eq!(ext.get_logs()[idx].topics[1], block_hash);
                assert_eq!(ext.get_logs()[idx].topics[2], payload_hash);
            } else {
                assert_eq!(
                    ext.get_logs()[idx].topics[0],
                    bridge_event_sig::BridgeEventSig::Distributed.hash().into()
                );
                assert_eq!(
                    ext.get_logs()[idx].topics[1],
                    transfers[idx].get_src_transaction_hash()
                );
                assert_eq!(
                    ext.get_logs()[idx].topics[2],
                    transfers[idx].get_recipient()
                );
                assert_eq!(
                    BigInt::from_bytes_be(
                        Sign::Plus,
                        &<[u8; 32]>::from(ext.get_logs()[idx].topics[3])
                    ),
                    transfers[idx].get_transfer_value()
                );
            }
        }
    }

    #[test]
    fn test_transfer_huge_list_offset() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        let mut transfers: Vec<BridgeTransfer> = Vec::new();
        for i in 0..10 {
            // generate a unique sourceTransactionHash for each transfer
            let source_transaction_hash = H256::from(Blake2b::hash_256(i.to_string().as_bytes()));
            transfers.push(
                get_instance(
                    BigInt::from(1),
                    public_to_address_ed25519(&H256::from_slice(&Blake2b::hash_256(
                        format!("{:x}", i).as_bytes(),
                    ))),
                    source_transaction_hash,
                )
                .unwrap(),
            );
        }
        // setup
        let from_setup = setup_for_test(&mut ext, &transfers);
        let submit_bundle_context = from_setup.submit_bundle_context;
        ext.change_context(submit_bundle_context);
        let payload_hash = from_setup.payload_hash;
        let mut call_payload = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef87500000000000000000000000000000080000000000000000000000000000001d000000000000000000000000000000320000000000000000000000000000003d000000000000000000000000000000480000000000000000000000000000005300000000000000000000000000000000a0fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d67bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e90a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a20000000000000000000000000000000aa08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205eda0d12a94f45cf9e0524069746acd51ab53a3190ea520bbcaf58329ab5c8626b4a0d8ef096f4f07a57163bdd28728f2a5d4689cd8c7089618223f567e30a95e33a017063ae510ba37ff55f5fa533c6d953a33f6a5252feaa35d45b260dda8c5f2a0614182bec524d648f7dba8a686037f0f535f52f6d3a3b031ef6fc78bc4c641a0b90da4145ddddbe57b2839bf4ca2ab966a5c1e66db2eb977f26c90621bc1820000000000000000000000000000000a0000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cbf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f0000000000000000000000000000000575312f45d7791c702bfe38f056287fb8b64514cbc77f76f17b421ec3451a25eda66e2fdb4b7a13dd48c9953c12bc8ea7f2f09f1e0dd1c6a01e5b1557964826f6760f529c41c666bb07c0b492ae9700c8f0a46783006ca140905413c5f14703fe1831b6f93aa1efddcb49408edaabbebf688b5580b5f58f41c19a3e0ffe39aec40383882b5f0b02053706cbb8265991d1ce70c140e6480422d8e7a70cb7fff54700000000000000000000000000000005b57dba44d5107751d16daf30a9e186730197e4d12fbeeff57d335c80e7f91e04c386d3fa9f0336a3900a11be4788f71d58a9782525c11fd4e10c4dccdf03340ae50ed9967d016e2ee4b2fade9baaa818291a0047da50aa3dd54e71bba48f20056aaaaa8e915d3cea4c9d84721489d5c568468da5f80ac3e0434b55cef0d23c001a4439c6c23fbff79c27dd7d7edbaae0743b469a48a2ee4a65374606a5d6c203".from_hex().unwrap();

        call_payload[50] = 0xff; // make the list offset here too big
        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Failure);

        // VERIFICATION failure
        let mut merged_payload = BridgeFuncSig::ActionMap.hash().to_vec();
        merged_payload.extend_from_slice(payload_hash.as_ref());
        let result = contract.execute(&mut ext, &merged_payload);
        assert_eq!(*result.return_data, [0u8; 32]);

        assert!(transfer_result.status_code == ExecStatus::Failure);
        // check that nothing has been changed from the failed transfer
        for b in &transfers {
            assert_eq!(ext.balance(&b.get_recipient()), U256::from(0));
        }
        assert_eq!(ext.balance(&*CONTRACT_ADDRESS), U256::from(10));
        assert_eq!(ext.get_logs().len(), 0);
    }

    #[test]
    fn test_transfer_huge_list_length() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        let mut transfers: Vec<BridgeTransfer> = Vec::new();
        for i in 0..10 {
            // generate a unique sourceTransactionHash for each transfer
            let source_transaction_hash = H256::from(Blake2b::hash_256(i.to_string().as_bytes()));
            transfers.push(
                get_instance(
                    BigInt::from(1),
                    public_to_address_ed25519(&H256::from_slice(&Blake2b::hash_256(
                        format!("{:x}", i).as_bytes(),
                    ))),
                    source_transaction_hash,
                )
                .unwrap(),
            );
        }
        // setup
        let from_setup = setup_for_test(&mut ext, &transfers);
        let submit_bundle_context = from_setup.submit_bundle_context;
        ext.change_context(submit_bundle_context);
        let payload_hash = from_setup.payload_hash;
        let mut call_payload = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef87500000000000000000000000000000080000000000000000000000000000001d000000000000000000000000000000320000000000000000000000000000003d000000000000000000000000000000480000000000000000000000000000005300000000000000000000000000000000a0fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d67bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e90a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a20000000000000000000000000000000aa08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205eda0d12a94f45cf9e0524069746acd51ab53a3190ea520bbcaf58329ab5c8626b4a0d8ef096f4f07a57163bdd28728f2a5d4689cd8c7089618223f567e30a95e33a017063ae510ba37ff55f5fa533c6d953a33f6a5252feaa35d45b260dda8c5f2a0614182bec524d648f7dba8a686037f0f535f52f6d3a3b031ef6fc78bc4c641a0b90da4145ddddbe57b2839bf4ca2ab966a5c1e66db2eb977f26c90621bc1820000000000000000000000000000000a0000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005f0f3f194b9cdf222f5d0bba78cbc95d7eb121f10439e06574d4d9ff323e1bb911c43b516f7751475bc994ea42e26142ffa944e452924ba2a46b0df70a76505cbf80ac5265101380c0ce5fbe3620eb6a74fb5beaa59d5facf25817b6869c9c3ccf2da0c38b25bfc48daef5eee360ab31e88ede3912e7873a67322d184450ce6c5e08a21395db2e863588a90e2405de27f777aa878f1f3a9ce20e7a8971762b57f0000000000000000000000000000000575312f45d7791c702bfe38f056287fb8b64514cbc77f76f17b421ec3451a25eda66e2fdb4b7a13dd48c9953c12bc8ea7f2f09f1e0dd1c6a01e5b1557964826f6760f529c41c666bb07c0b492ae9700c8f0a46783006ca140905413c5f14703fe1831b6f93aa1efddcb49408edaabbebf688b5580b5f58f41c19a3e0ffe39aec40383882b5f0b02053706cbb8265991d1ce70c140e6480422d8e7a70cb7fff54700000000000000000000000000000005b57dba44d5107751d16daf30a9e186730197e4d12fbeeff57d335c80e7f91e04c386d3fa9f0336a3900a11be4788f71d58a9782525c11fd4e10c4dccdf03340ae50ed9967d016e2ee4b2fade9baaa818291a0047da50aa3dd54e71bba48f20056aaaaa8e915d3cea4c9d84721489d5c568468da5f80ac3e0434b55cef0d23c001a4439c6c23fbff79c27dd7d7edbaae0743b469a48a2ee4a65374606a5d6c203".from_hex().unwrap();

        call_payload[146] = 0xff; // make the list length here too big
        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Failure);

        // VERIFICATION failure
        let mut merged_payload = BridgeFuncSig::ActionMap.hash().to_vec();
        merged_payload.extend_from_slice(payload_hash.as_ref());
        let result = contract.execute(&mut ext, &merged_payload);
        assert_eq!(*result.return_data, [0u8; 32]);

        assert!(transfer_result.status_code == ExecStatus::Failure);
        // check that nothing has been changed from the failed transfer
        for b in &transfers {
            assert_eq!(ext.balance(&b.get_recipient()), U256::from(0));
        }
        assert_eq!(ext.balance(&*CONTRACT_ADDRESS), U256::from(10));
        assert_eq!(ext.get_logs().len(), 0);
    }

    #[test]
    fn test_transfer_fail_length() {
        let members: Vec<Ed25519KeyPair> = {
            let mut members = Vec::with_capacity(1);
            members.push(Ed25519KeyPair::from("65d4658520b82571957ea0d867bee7cd217b6e597f51832cc2124dfca453e2f27079c71220d9a436177b78ccad239a8821ec9ae0df9c38b2a81e109ad780e54a7079c71220d9a436177b78ccad239a8821ec9ae0df9c38b2a81e109ad780e54a".from_hex().unwrap()));
            members
        };

        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        let payload = "1664500e0000000000000000000000000000001000000000000000000000000000000001a07f2ce133262d01af9c8f8eff5147f906f1212b6205090c6688933c33c4c68c"
            .from_hex()
            .unwrap();
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Success);

        let call_payload: Vec<u8> =
            "6548e9bca07f2ce133262d01af9c8f8eff5147f906f1212b6205090c6688933c33c4c68c"
                .from_hex()
                .unwrap();
        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);

        let _ = ext.add_balance(&*CONTRACT_ADDRESS, &U256::from(10));

        let submit_bundle_context = BuiltinContext {
            sender: members[0].address(),
            address: *CONTRACT_ADDRESS,
            tx_hash: *TX_HASH,
            origin_tx_hash: H256::default(),
        };
        ext.change_context(submit_bundle_context);

        let call_payload2 = "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef87500000000000000000000000000000080000000000000000000000000000000b0000000000000000000000000000000e0000000000000000000000000000001000000000000000000000000000000013000000000000000000000000000000160000000000000000000000000000000010fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb800000000000000000000000000000001a08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1b0000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000017079c71220d9a436177b78ccad239a8821ec9ae0df9c38b2a81e109ad780e54a000000000000000000000000000000012c2365c701e53e2346b8fb96f7b086c5288519a3e08d014a0238ea58bb61c90f000000000000000000000000000000013d5f8ca4c910a3705709203e018ddced2372207d2a8503f98b26749f4a84a307"
            .from_hex()
            .unwrap();
        let mut i = 1;
        for _ in 1..404 {
            let transfer_result2 = contract.execute(&mut ext, &call_payload2[0..i]);
            assert!(transfer_result2.status_code == ExecStatus::Failure);
            i = i + 1;
        }
        let mut input = call_payload2[0..404].to_vec();
        input.extend_from_slice(&[3u8; 151]);
        let transfer_result2 = contract.execute(&mut ext, &input);
        assert!(transfer_result2.status_code == ExecStatus::Success);
    }

    #[test]
    fn test_transfer_fail_length2() {
        let members: Vec<Ed25519KeyPair> = {
            let mut members = Vec::with_capacity(5);
            members.push(Ed25519KeyPair::from("be2822febc82262ce08be53dbbba9ec7c4ef62adb5e737f230393ad6c7eb1552b127fbcdbce0325ce8b4db479c5a41c63d225705c5322a595b3dd57b0aff72b4b127fbcdbce0325ce8b4db479c5a41c63d225705c5322a595b3dd57b0aff72b4".from_hex().unwrap()));
            members.push(Ed25519KeyPair::from("d3714ad425f98fbf5734fc1f396792465e8ba35c73a5163ede34c8e2e2271b5372fbf884a7b0b05b102c7c4e99bfa7e86cfd0803ec0bce88c403799cd84750d072fbf884a7b0b05b102c7c4e99bfa7e86cfd0803ec0bce88c403799cd84750d0".from_hex().unwrap()));
            members.push(Ed25519KeyPair::from("ecdd9d559a8f10b53b756ec51c03b7946637d8b48edfb7234897a53c74f67ab23682c7de8e9c13b51b6f8ff384b5d55476294518d6d02d32814ed0aae3e9b53e3682c7de8e9c13b51b6f8ff384b5d55476294518d6d02d32814ed0aae3e9b53e".from_hex().unwrap()));
            members.push(Ed25519KeyPair::from("93eb63fb18c316a83312baa7cd24a154f52c5b99057faeaccf3ef9d93d0e40c92d3dfc52d2736aa064b8cf9768500eeb80a259461a3adf84dd0c4ab37e22fe672d3dfc52d2736aa064b8cf9768500eeb80a259461a3adf84dd0c4ab37e22fe67".from_hex().unwrap()));
            members.push(Ed25519KeyPair::from("581c0b1ad85c51f80d9ca9f119a93fcc457a4715caa6a079b0071e6a9fa7e76784a38745c5d1da633a25f749939c9957d4871b483b006a8979c37be1435d037e84a38745c5d1da633a25f749939c9957d4871b483b006a8979c37be1435d037e".from_hex().unwrap()));
            members
        };

        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        let payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a0b9f0ab581229026e75c66149704bb03005bd442cccb99bbba0feeae423ed3da0c57b994b23a33518e40cf7a8b28c23e4efe62cba7e61c00ecfed449cd07de9a00a1d0334269857d60f0209924a0685271f22cb7ea5a77d375a486fcab64307a04c9740e152b683f0c807bd8196958d4f1ca7d88011216c465e00915bc40182a07b99eed3f38f50a3b86a45c30cd5cbc2945a004c5356bc980d56f9d3f62d97"
            .from_hex()
            .unwrap();
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Success);

        let call_payload: Vec<u8> =
            "6548e9bca0b9f0ab581229026e75c66149704bb03005bd442cccb99bbba0feeae423ed3d"
                .from_hex()
                .unwrap();
        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);

        let _ = ext.add_balance(&*CONTRACT_ADDRESS, &U256::from(10));

        let submit_bundle_context = BuiltinContext {
            sender: members[0].address(),
            address: *CONTRACT_ADDRESS,
            tx_hash: *TX_HASH,
            origin_tx_hash: H256::default(),
        };
        ext.change_context(submit_bundle_context);

        let call_payload2= "46d1cc292a40cefa06ce721343497e5e6700747efd7655092eac48681c72f3e49f2ef87500000000000000000000000000000080000000000000000000000000000001d000000000000000000000000000000320000000000000000000000000000003d000000000000000000000000000000480000000000000000000000000000005300000000000000000000000000000000a0fd923ca5e7218c4ba3c3801c26a617ecdbfdaebb9c76ce2eca166e7855efbb892cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db31237cdb79ae1dfa7ffb87cde7ea8a80352d300ee5ac758a6cddd19d671925ec581348337b0f3e148620173daaa5f94d00d881705dcbf0aa83efdaba61d2ede1eb8649214997574e20c464388a172420d25403682bbbb80c496831c8cc1f8f0d70b201352f24bf1c9770b99f8f71201821411cf414377c9b8c2dbcee61db87d67bee3bbfb37286d6a41378082e08c12af0084f0b1b92f77983f4c3394e91b5e90a420b072ce72f6a6833576ffa74ea21dcca4ce7c025dbee7b1dae478cba6f29f95f6b30745ba7cbab07ccc59fdc83be45649c4c964909b7675ff0b57b15f585bcfd554527e31708adfbfdcaa46092238b452331f9c438a3f8b2d891648252a20000000000000000000000000000000aa08896b9366f09e5efb1fa2ed9f3820b865ae97adbc6f364d691eb17784c9b1ba0e1b3419782ad92ec2dffed6d3f36cb99f4be8f7280ffe3a874b02bffa9a861a03d6a912166c954916057eb6a07d3e8bf451fccaaba8bf7fff62cae2a13c740a00091fc4b5c2f2c11f1801e505206359d8b029954790ddc0ab7c89438b58876a0a4ebb6fac47a0fe6ed7e40b87ee5440693965c44b9e5d23156f1b893b205eda0d12a94f45cf9e0524069746acd51ab53a3190ea520bbcaf58329ab5c8626b4a0d8ef096f4f07a57163bdd28728f2a5d4689cd8c7089618223f567e30a95e33a017063ae510ba37ff55f5fa533c6d953a33f6a5252feaa35d45b260dda8c5f2a0614182bec524d648f7dba8a686037f0f535f52f6d3a3b031ef6fc78bc4c641a0b90da4145ddddbe57b2839bf4ca2ab966a5c1e66db2eb977f26c90621bc1820000000000000000000000000000000a0000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000005b127fbcdbce0325ce8b4db479c5a41c63d225705c5322a595b3dd57b0aff72b472fbf884a7b0b05b102c7c4e99bfa7e86cfd0803ec0bce88c403799cd84750d03682c7de8e9c13b51b6f8ff384b5d55476294518d6d02d32814ed0aae3e9b53e2d3dfc52d2736aa064b8cf9768500eeb80a259461a3adf84dd0c4ab37e22fe6784a38745c5d1da633a25f749939c9957d4871b483b006a8979c37be1435d037e000000000000000000000000000000056d27c6193828b8ccbeab3a974f3ce0fb6fd864b367f8ba5eddb23f972baa9240de675ce8fb70b23b6197f715665483155b823b666dd59562e66b6dd0c0153f0d1191e70f018ffd4b5273363d76acc6cf42fed6c72154f5b134f555079543d4d51a3037b465d578656f096462cdecf846c7e9ba603a9730839534c6ac9c33101c0cea5f516d8780b86d1b4695510b2cff805472d7bbbaee301d86c331011d2f2300000000000000000000000000000005194c4c4fb20cb7cb3fe3cfb550e597bc49ae9655da9e18cd0a88f5d13daf3b0f75b7d56ba92f2d53b1ed6d0244f1a04625adbd688a38bd08b870b6864e46820621674b34abe96de7acce0f12b792d599669741933c37c53df7a39c29ecbe970c0ded636fd68f813563bf47a5178df84f05bb20a277afc23c9868d7b716541f032fc41a90879bb7038d49186b38015c7eb64d58c354b5d7d046824c8f8e48800f".from_hex()
            .unwrap();
        let mut i = 1;
        for _ in 1..1508 {
            let transfer_result2 = contract.execute(&mut ext, &call_payload2[0..i]);
            assert!(transfer_result2.status_code == ExecStatus::Failure);
            i = i + 1;
        }
    }

    #[test]
    fn test_ring_locked() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        let payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9"
            .from_hex()
            .unwrap();
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Success);

        let call_payload = "1a286d590000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9"
            .from_hex()
            .unwrap();
        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);
        assert_eq!(
            (*transfer_result.return_data).to_vec(),
            DataWord::one().data
        );

        contract.connector.set_ring_locked(&mut ext, false);

        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);
        assert_eq!(
            (*transfer_result.return_data).to_vec(),
            DataWord::zero().data
        );
    }

    #[test]
    fn test_min_threshold() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        let payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9"
            .from_hex()
            .unwrap();
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Success);

        let call_payload = "6c44b2270000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9"
            .from_hex()
            .unwrap();
        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);
        assert_eq!(
            (*transfer_result.return_data).to_vec(),
            DataWord::new_with_int(3).data
        );

        contract.connector.set_min_thresh(&mut ext, 5);

        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);
        assert_eq!(
            (*transfer_result.return_data).to_vec(),
            DataWord::new_with_int(5).data
        );

        contract.connector.set_min_thresh(&mut ext, 10);

        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);
        assert_eq!(
            (*transfer_result.return_data).to_vec(),
            DataWord::new_with_int(10).data
        );
    }

    #[test]
    fn test_member_count() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        let payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9"
            .from_hex()
            .unwrap();
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Success);

        let call_payload = "11aee3800000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9"
            .from_hex()
            .unwrap();
        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);
        assert_eq!(
            (*transfer_result.return_data).to_vec(),
            DataWord::new_with_int(5).data
        );

        contract.connector.set_member_count(&mut ext, 10);

        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);
        assert_eq!(
            (*transfer_result.return_data).to_vec(),
            DataWord::new_with_int(10).data
        );
    }

    #[test]
    fn test_ring_map() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        let payload = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9"
            .from_hex()
            .unwrap();
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Success);

        let encode_bytes = "b22fce40".from_hex().unwrap();
        let mut call_payload = encode_bytes.clone();
        call_payload.extend_from_slice(generate_keypair().address().as_ref());

        let transfer_result = contract.execute(&mut ext, &call_payload);
        assert!(transfer_result.status_code == ExecStatus::Success);

        let transfer_result = contract.execute(&mut ext, &encode_bytes);
        assert!(transfer_result.status_code == ExecStatus::Failure);
    }

    #[test]
    fn test_fail_ring_initialize() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        // address null
        let payload = "1664500e".from_hex().unwrap();
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Failure);

        contract.connector.set_ring_locked(&mut ext, true);

        // failed to initialize due to locked ring
        let payload2 = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9"
            .from_hex()
            .unwrap();

        let result2 = contract.execute(&mut ext, &payload2);
        assert!(result2.status_code == ExecStatus::Failure);
    }

    #[test]
    fn tset_add_ring_member() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        // address null - fail
        let payload = "06c8dcde".from_hex().unwrap();
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Failure);

        // add new member - fail
        let mut payload2 = payload.clone();
        payload2.extend_from_slice(generate_keypair().address().as_ref());
        let result2 = contract.execute(&mut ext, &payload2);
        assert!(result2.status_code == ExecStatus::Failure);

        // lock the ring
        contract.connector.set_ring_locked(&mut ext, true);

        // add new member - success
        let mut payload3 = payload.clone();
        payload3.extend_from_slice(generate_keypair().address().as_ref());
        let result3 = contract.execute(&mut ext, &payload3);
        assert!(result3.status_code == ExecStatus::Success);
    }

    #[test]
    fn tset_remove_ring_member() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        // address null - fail
        let payload = "67a3914e".from_hex().unwrap();
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Failure);

        // remove member - fail
        let mut payload2 = payload.clone();
        payload2.extend_from_slice(generate_keypair().address().as_ref());
        let result2 = contract.execute(&mut ext, &payload2);
        assert!(result2.status_code == ExecStatus::Failure);

        // initialize ring
        let ring = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
        let _ = contract.execute(&mut ext, &ring);

        // remove member - fail, member does not exist
        let mut payload3 = payload.clone();
        payload3.extend_from_slice(generate_keypair().address().as_ref());
        let result3 = contract.execute(&mut ext, &payload3);
        assert!(result3.status_code == ExecStatus::Failure);

        // remove member - success, member exists
        let payload4 = "67a3914ea092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
            .from_hex()
            .unwrap();
        let result4 = contract.execute(&mut ext, &payload4);
        assert!(result4.status_code == ExecStatus::Success);
    }

    #[test]
    fn tset_remove_ring_member_not_owner() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        // address null - fail
        let payload = "67a3914e".from_hex().unwrap();
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Failure);

        // remove member - fail
        let mut payload2 = payload.clone();
        payload2.extend_from_slice(generate_keypair().address().as_ref());
        let result2 = contract.execute(&mut ext, &payload2);
        assert!(result2.status_code == ExecStatus::Failure);

        // initialize ring
        let ring = "1664500e0000000000000000000000000000001000000000000000000000000000000005a092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42fa074789a508ca56a869ed32cdc0161d62d470ad71921c0963b3a7d05ec52824ea035cc9e6ff01735084251f369488e781a1e16189760b4196159d9ea90ca7750a011f8c41611b12ef05d29f7afbbfb183d61c1d4ea8e8ae9daf55e4bfad4e53ba01e120efeedb5500248eae8b8700df4c2941b49f1b76e367b50ec48f8d094d9".from_hex().unwrap();
        let _ = contract.execute(&mut ext, &ring);

        // remove member - fail, member does not exist
        let mut payload3 = payload.clone();
        payload3.extend_from_slice(generate_keypair().address().as_ref());
        let result3 = contract.execute(&mut ext, &payload3);
        assert!(result3.status_code == ExecStatus::Failure);

        ext.change_context(BuiltinContext {
            sender: Address::zero(),
            address: *CONTRACT_ADDRESS,
            tx_hash: *TX_HASH,
            origin_tx_hash: H256::default(),
        });
        // failure, member exists but sender is no longer owner
        let payload4 = "67a3914ea092c2138fd623b3933c749c7ca26c0269ab377b20cde2cbdd3f0b9a0b59b42f"
            .from_hex()
            .unwrap();
        let result4 = contract.execute(&mut ext, &payload4);
        assert!(result4.status_code == ExecStatus::Failure);
    }

    #[test]
    fn test_set_relayer() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_default(&mut state, &mut substate);

        // address null - fail
        let null_input = "6548e9bc".from_hex().unwrap();
        let res = contract.execute(&mut ext, &null_input);
        assert!(res.status_code == ExecStatus::Failure);

        // address valid
        let mut payload = null_input.clone();
        payload.extend_from_slice(generate_keypair().address().as_ref());
        let result = contract.execute(&mut ext, &payload);
        assert!(result.status_code == ExecStatus::Success);

        // caller not owner - fail
        let address = generate_keypair().address();
        ext.change_context(BuiltinContext {
            sender: address,
            address: *CONTRACT_ADDRESS,
            tx_hash: *TX_HASH,
            origin_tx_hash: H256::default(),
        });
        let mut payload2 = null_input.clone();
        payload2.extend_from_slice(generate_keypair().address().as_ref());
        let result2 = contract.execute(&mut ext, &payload2);
        assert!(result2.status_code == ExecStatus::Failure);
    }

    #[test]
    fn test_fallback_tx() {
        let mut substate = Substate::new();
        let mut state = get_temp_state();
        let contract = get_contract();
        let mut ext = get_ext_zero_sender(&mut state, &mut substate);

        assert!(!contract.connector.get_initialized(&mut ext));
        let result = contract.execute(&mut ext, &"".from_hex().unwrap());
        assert!(result.status_code == ExecStatus::Success);
        assert!(contract.connector.get_initialized(&mut ext));
    }

}
