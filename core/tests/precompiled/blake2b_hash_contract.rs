//    use super::Blake2bHashContract;
//    use precompiled::builtin::{BuiltinParams, BuiltinExtImpl, BuiltinContext, BuiltinContract};
//    use state::{State, Substate};
//    use tests::helpers::get_temp_state;
//    use vms::ExecStatus;
//    use bytes::to_hex;
//    use aion_types::{Address, H256};
//
//    fn get_ext_default<'a>(
//        state: &'a mut State<::state_db::StateDB>,
//        substate: &'a mut Substate,
//    ) -> BuiltinExtImpl<'a, ::state_db::StateDB>
//    {
//        BuiltinExtImpl::new(
//            state,
//            BuiltinContext {
//                sender: Address::zero(),
//                address: Address::zero(),
//                tx_hash: H256::zero(),
//                origin_tx_hash: H256::default(),
//            },
//            substate,
//        )
//    }
//
//    fn get_contract() -> Blake2bHashContract {
//        Blake2bHashContract::new(BuiltinParams {
//            activate_at: 920000,
//            deactivate_at: None,
//            name: String::from("blake2b"),
//            owner_address: None,
//            contract_address: None,
//        })
//    }

//    #[test]
//    fn test_blake256() {
//        let contract = get_contract();
//        let state = &mut get_temp_state();
//        let substate = &mut Substate::new();
//        let mut ext = get_ext_default(state, substate);
//
//        let input = "a0010101010101010101010101".as_bytes();
//        let result = contract.execute(&mut ext, &input);
//        assert!(result.status_code == ExecStatus::Success);
//        let ret_data = result.return_data;
//        assert_eq!((*ret_data).len(), 32);
//        let expected = "aa6648de0988479263cf3730a48ef744d238b96a5954aa77d647ae965d3f7715";
//        assert_eq!(to_hex(&*ret_data), expected);
//    }
//
//    #[test]
//    fn test_blake256_2() {
//        let contract = get_contract();
//        let state = &mut get_temp_state();
//        let substate = &mut Substate::new();
//        let mut ext = get_ext_default(state, substate);
//
//        let input = "1".as_bytes();
//        let result = contract.execute(&mut ext, &input);
//        assert!(result.status_code == ExecStatus::Success);
//        let ret_data = result.return_data;
//        assert_eq!((*ret_data).len(), 32);
//        let expected = "92cdf578c47085a5992256f0dcf97d0b19f1f1c9de4d5fe30c3ace6191b6e5db";
//        assert_eq!(to_hex(&*ret_data), expected);
//    }

// uncomment if uncomment box_syntax in root crate
//    #[test]
//    fn test_blake256_3() {
//        // long input
//        let contract = get_contract();
//        let state = &mut get_temp_state();
//        let substate = &mut Substate::new();
//        let mut ext = get_ext_default(state, substate);
//
//        let input = box [0u8; 2 * 1024 * 1024];
//        let result = contract.execute(&mut ext, input.as_ref());
//        assert!(result.status_code == ExecStatus::Success);
//        let ret_data = result.return_data;
//        assert_eq!(ret_data.mem.len(), 32);
//        let expected = "9852d74e002f23d14ba2638b905609419bd16e50843ac147ccf4d509ed2c9dfc";
//        assert_eq!(to_hex(&ret_data.mem), expected);
//    }

//    #[test]
//    fn test_blake256_invalid_length() {
//        let contract = get_contract();
//        let state = &mut get_temp_state();
//        let substate = &mut Substate::new();
//        let mut ext = get_ext_default(state, substate);
//        let input = "".as_bytes();
//        let result = contract.execute(&mut ext, &input);
//        assert!(result.status_code == ExecStatus::Failure);
//    }

// uncomment if uncomment box_syntax in root crate
//    #[test]
//    fn test_blake256_larger_length() {
//        let contract = get_contract();
//        let state = &mut get_temp_state();
//        let substate = &mut Substate::new();
//        let mut ext = get_ext_default(state, substate);
//        let input = box [0u8; 2 * 1024 * 1024 + 1];
//        let result = contract.execute(&mut ext, input.as_ref());
//        assert!(result.status_code == ExecStatus::Failure);
//    }