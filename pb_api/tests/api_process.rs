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
//
//#![warn(unused_extern_crates)]
//
//extern crate protobuf;
//extern crate aion_pb_apiserver;
//extern crate aion_types;
//extern crate rand;
//#[macro_use]
//extern crate lazy_static;
//
//use std::sync::Mutex;
//use rand::random;
//use aion_types::{H256, U256};
//use protobuf::{parse_from_bytes, Message, ProtobufEnum};
//use aion_pb_apiserver::message::*;
//use aion_pb_apiserver::pb_api_util;
//use aion_pb_apiserver::api_process::{get_api_version, ApiProcess};
//
//lazy_static! {
//    static ref MSG: Vec<u8> = {
//        let msg: Vec<u8> = b"test message".to_vec();
//        msg
//    };
//    static ref HASH: Vec<u8> = {
//        let mut hash = Vec::new();
//        for _ in 0..pb_api_util::HASH_LEN {
//            hash.push(random::<u8>());
//        }
//        hash
//    };
//    static ref SOCKID: Vec<u8> = {
//        let mut id = Vec::new();
//        for _ in 0..5 {
//            id.push(random::<u8>());
//        }
//        id
//    };
//    //static ref API: Mutex<ApiProcess> = { Mutex::new(api_process_instance()) };
//}
//
//static MSG_HASH_LEN: usize = 8;
//static RSP_HEADER_NOHASH_LEN: usize = 3;
////static REQ_HEADER_NOHASH_LEN: usize = 4;
//static RSP_HEADER_LEN: usize = RSP_HEADER_NOHASH_LEN + MSG_HASH_LEN;
//
//fn send_request(s: i32, f: i32, req_body: Vec<u8>) -> Vec<u8> {
//    let req_body = if req_body.is_empty() {
//        MSG.clone()
//    } else {
//        req_body
//    };
//    let req = vec![
//        vec![get_api_version()],
//        vec![s as u8],
//        vec![f as u8],
//        vec![1],
//        HASH.clone(),
//        req_body.to_vec(),
//    ]
//    .into_iter()
//    .flat_map(|v| v.into_iter())
//    .collect::<Vec<_>>();
//    API.lock().unwrap().process(&req, &SOCKID)
//}
//
//fn strip_header(rsp: Vec<u8>) -> Vec<u8> {
//    let hash_hash = rsp[2] == 1;
//    let body_len = rsp.len() - if hash_hash {
//        RSP_HEADER_LEN
//    } else {
//        RSP_HEADER_NOHASH_LEN
//    };
//    if hash_hash {
//        return rsp.as_slice()[RSP_HEADER_LEN..RSP_HEADER_LEN + body_len].to_vec();
//    } else {
//        return rsp.as_slice()[RSP_HEADER_NOHASH_LEN..RSP_HEADER_NOHASH_LEN + body_len].to_vec();
//    }
//}
//
//fn u256_to_vec(u: U256) -> Vec<u8> {
//    let u: [u8; 32] = u.into();
//    u.to_vec()
//}
//
//#[test]
//fn test_process_get_balance() {
//    let mut req = req_getBalance::new();
//    let address: H256 = "a035b4bc8f3603daa72133fe21c302855c45889567411f96188cf1765d3b74fb"
//        .parse()
//        .unwrap();
//    let balance: U256 = vec![
//        0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 72, 122, 154, 48,
//        69, 57, 68, 0, 0,
//    ]
//    .as_slice()
//    .into();
//    req.set_address(address.0.to_vec());
//    let rsp = send_request(
//        Servs::s_chain.value(),
//        Funcs::f_getBalance.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_success.value() as u8, rsp[1]);
//    let rslt = parse_from_bytes::<rsp_getBalance>(&strip_header(rsp)).unwrap();
//    assert_eq!(rslt.get_balance().to_vec(), u256_to_vec(balance));
//    let rsp = send_request(
//        Servs::s_hb.value(),
//        Funcs::f_getBalance.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_fail_service_call.value() as u8, rsp[1]);
//}
//
//#[test]
//fn test_process_get_nonce() {
//    let mut req = req_getNonce::new();
//    let address: H256 = "a035b4bc8f3603daa72133fe21c302855c45889567411f96188cf1765d3b74fb"
//        .parse()
//        .unwrap();
//    let nonce = U256::zero();
//    req.set_address(address.0.to_vec());
//    let rsp = send_request(
//        Servs::s_chain.value(),
//        Funcs::f_getNonce.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_success.value() as u8, rsp[1]);
//    let rslt = parse_from_bytes::<rsp_getNonce>(&strip_header(rsp)).unwrap();
//    assert_eq!(rslt.get_nonce().to_vec(), u256_to_vec(nonce));
//    let rsp = send_request(
//        Servs::s_hb.value(),
//        Funcs::f_getNonce.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_fail_service_call.value() as u8, rsp[1]);
//}
//
//#[test]
//fn test_process_get_transaction() {
//    let mut req = req_getTransactionByHash::new();
//    let txhash: H256 = "44244ed949dd189daf9172eb6c53a28777949068bd49ab761d0af26186fc8de1"
//        .parse()
//        .unwrap();
//    let blockhash: H256 = "d008ecacaa4c7a38ce6081b4b1dff2891e028c7eaaa5cce70865c5251fcd4212"
//        .parse()
//        .unwrap();
//    req.set_txHash(txhash.0.to_vec());
//    let rsp = send_request(
//        Servs::s_chain.value(),
//        Funcs::f_getTransactionByHash.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_success.value() as u8, rsp[1]);
//    let rslt = parse_from_bytes::<rsp_getTransaction>(&strip_header(rsp)).unwrap();
//    assert_eq!(rslt.get_blocknumber(), 1);
//    assert_eq!(rslt.get_blockhash(), &blockhash.0);
//    let rsp = send_request(
//        Servs::s_hb.value(),
//        Funcs::f_getTransactionByHash.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_fail_service_call.value() as u8, rsp[1]);
//}
//
//#[test]
//fn test_process_blocknumber() {
//    let rsp = send_request(Servs::s_chain.value(), Funcs::f_blockNumber.value(), vec![]);
//    assert_eq!(Retcode::r_success.value() as u8, rsp[1]);
//    let rslt = parse_from_bytes::<rsp_blockNumber>(&strip_header(rsp)).unwrap();
//    assert_eq!(rslt.get_blocknumber(), 1);
//    let rsp = send_request(Servs::s_hb.value(), Funcs::f_blockNumber.value(), vec![]);
//    assert_eq!(Retcode::r_fail_service_call.value() as u8, rsp[1]);
//}
//
//#[test]
//fn test_process_get_block_by_number() {
//    let mut req = req_getBlockByNumber::new();
//    let blockhash: H256 = "d008ecacaa4c7a38ce6081b4b1dff2891e028c7eaaa5cce70865c5251fcd4212"
//        .parse()
//        .unwrap();
//    req.set_blockNumber(1);
//    let rsp = send_request(
//        Servs::s_chain.value(),
//        Funcs::f_getBlockByNumber.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_success.value() as u8, rsp[1]);
//    let rslt = parse_from_bytes::<rsp_getBlock>(&strip_header(rsp)).unwrap();
//    assert_eq!(rslt.get_blockNumber(), 1);
//    assert_eq!(rslt.get_hash(), &blockhash.0);
//    let rsp = send_request(
//        Servs::s_hb.value(),
//        Funcs::f_getBlockByNumber.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_fail_service_call.value() as u8, rsp[1]);
//}
//
//#[test]
//fn test_process_block_details() {
//    let mut req = req_getBlockDetailsByNumber::new();
//    req.set_blkNumbers(vec![0, 1]);
//    let rsp = send_request(
//        Servs::s_admin.value(),
//        Funcs::f_getBlockDetailsByNumber.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_success.value() as u8, rsp[1]);
//    let rslt = parse_from_bytes::<rsp_getBlockDetailsByNumber>(&strip_header(rsp)).unwrap();
//    let blkdtl = rslt.get_blkDetails().to_vec();
//    assert_eq!(2, blkdtl.len());
//    assert_eq!(0, blkdtl[0].get_blockNumber());
//    assert_eq!(1, blkdtl[1].get_blockNumber());
//    let rsp = send_request(
//        Servs::s_hb.value(),
//        Funcs::f_getBlockDetailsByNumber.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_fail_service_call.value() as u8, rsp[1]);
//}
//
//#[test]
//fn test_process_get_syncinfo() {
//    let rsp = send_request(Servs::s_net.value(), Funcs::f_syncInfo.value(), vec![]);
//    assert_eq!(Retcode::r_success.value() as u8, rsp[1]);
//    let rslt = parse_from_bytes::<rsp_syncInfo>(&strip_header(rsp)).unwrap();
//    assert_eq!(false, rslt.get_syncing());
//    assert_eq!(0, rslt.get_networkBestBlock());
//    assert_eq!(1, rslt.get_chainBestBlock());
//    assert_eq!(24, rslt.get_maxImportBlocks());
//    let rsp = send_request(Servs::s_hb.value(), Funcs::f_syncInfo.value(), vec![]);
//    assert_eq!(Retcode::r_fail_service_call.value() as u8, rsp[1]);
//}
//
//#[test]
//fn test_process_get_active_nodes() {
//    let rsp = send_request(
//        Servs::s_net.value(),
//        Funcs::f_getActiveNodes.value(),
//        vec![],
//    );
//    assert_eq!(Retcode::r_success.value() as u8, rsp[1]);
//    let rslt = parse_from_bytes::<rsp_getActiveNodes>(&strip_header(rsp)).unwrap();
//    let nodeslt = rslt.get_node().to_vec();
//    assert_eq!(0, nodeslt.len());
//    let rsp = send_request(Servs::s_hb.value(), Funcs::f_getActiveNodes.value(), vec![]);
//    assert_eq!(Retcode::r_fail_service_call.value() as u8, rsp[1]);
//}
//
//#[test]
//fn test_process_send_raw_transation() {
//    let mut req = req_rawTransaction::new();
//    req.set_encodedTx(SIGNED_TX.to_vec());
//    let rsp = send_request(
//        Servs::s_tx.value(),
//        Funcs::f_rawTransaction.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_tx_Recved.value() as u8, rsp[1]);
//    let rslt = parse_from_bytes::<rsp_sendTransaction>(&strip_header(rsp)).unwrap();
//    let txhash: H256 = "ad0b26f279ccbd692b928f1668e2688dc9dcd399672467ef9087804367ac6657"
//        .parse()
//        .unwrap();
//    assert_eq!(rslt.get_txHash(), &txhash.0);
//    let rsp = send_request(
//        Servs::s_hb.value(),
//        Funcs::f_rawTransaction.value(),
//        req.write_to_bytes().unwrap(),
//    );
//    assert_eq!(Retcode::r_fail_service_call.value() as u8, rsp[1]);
//}
//
//static SIGNED_TX: &[u8] = &[
//    248, 157, 128, 160, 160, 84, 52, 10, 49, 82, 209, 0, 6, 182, 108, 66, 72, 207, 167, 62, 87, 37,
//    5, 98, 148, 8, 28, 71, 108, 14, 103, 239, 90, 210, 83, 52, 100, 128, 136, 0, 5, 122, 192, 110,
//    51, 107, 14, 131, 14, 87, 224, 136, 0, 0, 0, 2, 143, 166, 174, 0, 1, 184, 96, 139, 197, 196,
//    229, 89, 154, 250, 199, 203, 14, 252, 176, 1, 5, 64, 1, 125, 218, 62, 128, 135, 11, 181, 67,
//    179, 86, 134, 123, 42, 140, 172, 191, 246, 230, 102, 121, 51, 112, 78, 77, 50, 154, 159, 56,
//    168, 90, 143, 31, 212, 25, 73, 243, 207, 187, 121, 165, 170, 82, 0, 3, 173, 201, 225, 71, 194,
//    204, 192, 239, 21, 51, 2, 93, 140, 2, 160, 67, 65, 161, 9, 227, 215, 93, 82, 55, 36, 145, 125,
//    227, 148, 209, 73, 7, 127, 192, 32, 3,
//];
