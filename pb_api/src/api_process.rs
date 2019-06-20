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

#![allow(dead_code)]
#![allow(non_snake_case)]
use super::LOG_TARGET;
use acore::transaction::local_transactions::TxIoMessage;
use aion_rpc::traits::Pb;
use aion_rpc::types::{
    Block, BlockTransactions, SimpleReceipt, Transaction
};
use aion_types::{H256, U256};
use crossbeam::queue::MsQueue;
use io::{IoHandler, IoService};
use message::*;
use parking_lot::RwLock;
use pb_api_util::*;
use protobuf::{Message, ProtobufEnum};
use rustc_hex::ToHex;
use std::collections::HashMap;
use std::sync::Arc;
use tx_pending_status::TxPendingStatus;

const API_VER: u8 = 2;
const API_REQHEADER_LEN: usize = 4;
const TX_HASH_LEN: usize = 32;
const ACCOUNT_CREATE_LIMIT: usize = 100;

macro_rules! api_try {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(_) => {
                return to_return_header(
                    get_api_version(),
                    Retcode::r_fail_function_exception.value(),
                );
            }
        }
    };
}
#[derive(Debug, Clone)]
pub struct SimpleEntry {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Clone)]
pub struct ApiProcess {
    client: Arc<Pb>,
    msg_id_mapping: Arc<RwLock<HashMap<H256, SimpleEntry>>>,
    pending_status: Arc<MsQueue<TxPendingStatus>>,
    io_service: Arc<IoService<TxIoMessage>>,
}

impl ApiProcess {
    pub fn new(ethclient: Arc<Pb>, io_service: IoService<TxIoMessage>) -> Self {
        let pending_status = Arc::new(MsQueue::new());
        let msg_id_mapping = Arc::new(RwLock::new(HashMap::new()));
        let io_service = Arc::new(io_service);
        let _ = io_service.register_handler(Arc::new(TxIoHandler {
            pending_status: pending_status.clone(),
            msg_id_mapping: msg_id_mapping.clone(),
        }));

        ApiProcess {
            client: ethclient,
            msg_id_mapping,
            pending_status,
            io_service,
        }
    }

    pub fn process(&self, request: &Vec<u8>, _socketId: &Vec<u8>) -> Vec<u8> {
        if request.is_empty() || request.len() < get_api_header_len() {
            return to_return_header(get_api_version(), Retcode::r_fail_header_len.value());
        }
        let msghash = get_api_msg_hash(request);
        if request[0] < get_api_version() {
            return if msghash.is_empty() {
                to_return_header(get_api_version(), Retcode::r_fail_api_version.value())
            } else {
                to_return_header_with_hash(
                    get_api_version(),
                    Retcode::r_fail_api_version.value(),
                    msghash,
                )
            };
        }

        let service = request[1] as i32;
        let message = request[2] as i32;

        match Funcs::from_i32(message) {
            Some(Funcs::f_getBalance) => {
                debug!(target: LOG_TARGET, "process message: f_getBalance");
                if service != Servs::s_chain.value() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_service_call.value(),
                    );
                }
                let data = parse_msg_req(request, &msghash);
                let mut req = req_getBalance::new();
                api_try!(req.merge_from_bytes(&data));
                let address = req.get_address().into();
                let balance: U256 = self.client.balance(address).into();
                let mut rsp = rsp_getBalance::new();
                rsp.set_balance(u256_to_vec(balance));
                let retheader = to_return_header(get_api_version(), Retcode::r_success.value());
                let retbody = api_try!(rsp.write_to_bytes());
                combine_ret_msg(retheader, retbody)
            }
            Some(Funcs::f_getTransactionByHash) => {
                debug!(
                    target: LOG_TARGET,
                    "process message: f_getTransactionByHash"
                );
                if service != Servs::s_chain.value() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_service_call.value(),
                    );
                }
                let data = parse_msg_req(request, &msghash);
                let mut req = req_getTransactionByHash::new();
                api_try!(req.merge_from_bytes(&data));
                let txhash = req.get_txHash().to_vec();
                if txhash.is_empty() || txhash.len() != get_tx_hash_len() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_function_arguments.value(),
                    );
                }
                let tx = self
                    .client
                    .transaction_by_hash(H256::from(txhash.as_slice()).into());
                if tx.is_none() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_function_call.value(),
                    );
                }
                let tx = tx.unwrap();
                let rsp = get_rsp_getTransaction(tx);
                let retheader = to_return_header(get_api_version(), Retcode::r_success.value());
                let retbody = api_try!(rsp.write_to_bytes());
                combine_ret_msg(retheader, retbody)
            }
            Some(Funcs::f_getNonce) => {
                debug!(target: LOG_TARGET, "process message: f_getNonce");
                if service != Servs::s_chain.value() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_service_call.value(),
                    );
                }
                let data = parse_msg_req(request, &msghash);
                let mut req = req_getNonce::new();
                api_try!(req.merge_from_bytes(&data));
                let address = req.get_address().into();
                let nonce: U256 = self.client.nonce(address).into();
                let mut rsp = rsp_getNonce::new();
                rsp.set_nonce(u256_to_vec(nonce));
                let retheader = to_return_header(get_api_version(), Retcode::r_success.value());
                let retbody = api_try!(rsp.write_to_bytes());
                combine_ret_msg(retheader, retbody)
            }
            Some(Funcs::f_blockNumber) => {
                debug!(target: LOG_TARGET, "process message: f_blockNumber");
                if service != Servs::s_chain.value() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_service_call.value(),
                    );
                }

                let blocknumber: U256 = self.client.blocknumber().into();
                let mut rsp = rsp_blockNumber::new();
                rsp.set_blocknumber(blocknumber.into());
                let retheader = to_return_header(get_api_version(), Retcode::r_success.value());
                let retbody = api_try!(rsp.write_to_bytes());
                combine_ret_msg(retheader, retbody)
            }
            Some(Funcs::f_getBlockByNumber) => {
                debug!(target: LOG_TARGET, "process message: f_getBlockByNumber");
                if service != Servs::s_chain.value() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_service_call.value(),
                    );
                }
                let data = parse_msg_req(request, &msghash);
                let mut req = req_getBlockByNumber::new();
                api_try!(req.merge_from_bytes(&data));
                let num = req.get_blockNumber() as i64;
                let block = self.client.block_by_number(num, false);
                create_block_msg(block)
            }
            Some(Funcs::f_getBlockDetailsByNumber) => {
                //TODO:
                debug!(
                    target: LOG_TARGET,
                    "process message: f_getBlockDetailsByNumber"
                );
                if service != Servs::s_admin.value() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_service_call.value(),
                    );
                }
                let data = parse_msg_req(request, &msghash);
                let mut req = req_getBlockDetailsByNumber::new();
                let best_block_number: u64 = self.client.blocknumber().into();
                api_try!(req.merge_from_bytes(&data));
                let mut nlknum: Vec<u64> = req
                    .get_blkNumbers()
                    .into_iter()
                    .map(|x| *x)
                    .filter(|x| *x <= best_block_number)
                    .collect();
                nlknum.sort();
                if nlknum.len() > 1000 {
                    nlknum.resize(1000, 0);
                }
                let mut rsp = rsp_getBlockDetailsByNumber::new();
                let blks = nlknum
                    .into_iter()
                    .map(|num| {
                        (
                            self.client.block_by_number(num as i64, true),
                            self.client.block_receipt(num as i64),
                        )
                    })
                    .collect::<Vec<_>>();
                if blks.is_empty() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_function_arguments.value(),
                    );
                }
                let blks_detail = blks
                    .into_iter()
                    .filter(|(blk, _)| blk.is_some())
                    .map(|(blk, br)| create_t_block_detail(blk.unwrap(), br))
                    .collect::<Vec<_>>();
                rsp.set_blkDetails(blks_detail.into());
                let retheader = to_return_header(get_api_version(), Retcode::r_success.value());
                let retbody = api_try!(rsp.write_to_bytes());
                combine_ret_msg(retheader, retbody)
            }
            Some(Funcs::f_syncInfo) => {
                //TODO:
                debug!(target: LOG_TARGET, "process message: f_syncInfo");
                if service != Servs::s_net.value() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_service_call.value(),
                    );
                }
                let sync = self.client.get_sync();
                let mut rsp = rsp_syncInfo::new();
                rsp.set_chainBestBlock(sync.chain_best_number);
                rsp.set_networkBestBlock(sync.network_best_number);
                rsp.set_maxImportBlocks(sync.max_import_block);
                rsp.set_startingBlock(sync.starting_block);
                rsp.set_syncing(sync.syncing);
                let retheader = to_return_header(get_api_version(), Retcode::r_success.value());
                let retbody = api_try!(rsp.write_to_bytes());
                combine_ret_msg(retheader, retbody)
            }
            Some(Funcs::f_getActiveNodes) => {
                //TODO:
                debug!(target: LOG_TARGET, "process message: f_getActiveNodes");
                if service != Servs::s_net.value() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_service_call.value(),
                    );
                }

                let nodes = self.client.get_active_nodes();
                let pl = nodes
                    .into_iter()
                    .map(|n| {
                        let mut node = t_Node::new();
                        node.set_blockNumber(n.highest_block_number);
                        node.set_nodeId(n.id);
                        node.set_remote_p2p_ip(n.ip);
                        node
                    })
                    .collect::<Vec<_>>();
                let mut rsp = rsp_getActiveNodes::new();
                rsp.set_node(pl.into());
                let retheader = to_return_header(get_api_version(), Retcode::r_success.value());
                let retbody = api_try!(rsp.write_to_bytes());
                combine_ret_msg(retheader, retbody)
            }
            Some(Funcs::f_signedTransaction) | Some(Funcs::f_rawTransaction) => {
                debug!(target: LOG_TARGET, "process message: f_signedTransaction");
                if service != Servs::s_tx.value() {
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_service_call.value(),
                    );
                }
                let data = parse_msg_req(request, &msghash);
                let mut req = req_rawTransaction::new();
                api_try!(req.merge_from_bytes(&data));
                let encodedTx = req.get_encodedTx().to_vec();
                if encodedTx.is_empty() {
                    error!(
                        target: LOG_TARGET,
                        "rawTransaction exception: [null encodedTx]"
                    );
                    return to_return_header(
                        get_api_version(),
                        Retcode::r_fail_function_arguments.value(),
                    );
                }
                let result: Option<H256> = self.client.pb_send_transaction(encodedTx.into());
                if result.is_none() {
                    return to_return_header_with_hash(
                        get_api_version(),
                        Retcode::r_fail_sendTx_null_rep.value(),
                        msghash,
                    );
                }
                let entry = SimpleEntry {
                    key: msghash.clone(),
                    value: _socketId.clone(),
                };
                let result = result.unwrap();
                debug!(target: LOG_TARGET, "msgIdMapping.put: {:?}", result);
                {
                    self.msg_id_mapping
                        .write()
                        .insert(result.clone().into(), entry);
                }
                let mut rsp = rsp_sendTransaction::new();
                rsp.set_txHash(result.0.to_vec());
                let retheader = to_return_header_with_hash(
                    get_api_version(),
                    Retcode::r_tx_Recved.value(),
                    msghash,
                );
                let retbody = api_try!(rsp.write_to_bytes());
                combine_ret_msg(retheader, retbody)
            }
            _ => to_return_header(get_api_version(), Retcode::r_fail_function_call.value()),
        }
    }

    pub fn take_tx_status(&self) -> TxPendingStatus { self.pending_status.pop() }

    pub fn shut_down(&self) { self.pending_status.push(TxPendingStatus::default()); }
}
// =====================================================================================
// ============================== msg util =============================================
// =====================================================================================

fn get_api_header_len() -> usize { API_REQHEADER_LEN }

pub fn get_api_version() -> u8 { API_VER }

fn parse_msg_req(request: &Vec<u8>, msghash: &Vec<u8>) -> Vec<u8> {
    let headerlen = if msghash.is_empty() {
        get_api_header_len()
    } else {
        get_api_header_len() + msghash.len()
    };

    request.as_slice()[headerlen..request.len()].to_vec()
}

pub fn to_rsp_msg(msg_hash: &Vec<u8>, tx_code: i32, error: &String) -> Vec<u8> {
    to_return_header_with_hash_and_error(get_api_version(), tx_code, &msg_hash, error.as_bytes())
}

pub fn to_rsp_msg_with_result(
    msg_hash: &Vec<u8>,
    tx_code: i32,
    error: &String,
    result: &Vec<u8>,
) -> Vec<u8>
{
    to_return_header_with_hash_error_and_result(
        get_api_version(),
        tx_code,
        &msg_hash,
        error.as_bytes(),
        result.as_slice(),
    )
}

fn get_tx_hash_len() -> usize { TX_HASH_LEN }

fn get_rsp_getTransaction(tx: Transaction) -> rsp_getTransaction {
    let mut rsp = rsp_getTransaction::new();
    let blockhash = tx.block_hash.map_or_else(|| H256::from(0), |a| a.into());
    let blocknumber = tx.block_number.map_or_else(|| U256::from(0), |a| a.into());
    let txindex = tx
        .transaction_index
        .map_or_else(|| U256::from(0), |a| a.into());
    let txto = tx.to.map_or_else(|| H256::from(0), |a| a.into());

    rsp.set_blockhash(blockhash.0.to_vec());
    rsp.set_blocknumber(blocknumber.into());
    rsp.set_data(tx.input.into_vec());
    rsp.set_from(tx.from.0.to_vec());
    rsp.set_nonce(u256_to_vec(tx.nonce.into()));
    rsp.set_nrgConsume(tx.gas.into());
    rsp.set_nrgPrice(tx.gas_price.into());
    let timestamp = tx.timestamp.into_vec();
    rsp.set_timeStamp(U256::from(timestamp.as_slice()).into());
    rsp.set_to(txto.to_vec());
    rsp.set_txHash(tx.hash.0.to_vec());
    rsp.set_txIndex(txindex.into());
    rsp.set_value(u256_to_vec(tx.value.into()));
    rsp
}

fn create_t_block_detail(block: Block, br: Vec<SimpleReceipt>) -> t_BlockDetail {
    let blockhash = block.hash.map_or_else(|| H256::from(0), |a| a.into());
    let blocknumber = block.number.unwrap_or(0u64);
    let blocksize = block.size.map_or_else(|| U256::from(0), |a| a.into());
    let total_difficulty = block
        .total_difficulty
        .map_or_else(|| U256::from(0), |a| a.into());
    let mut blockdetail = t_BlockDetail::new();
    blockdetail.set_blockNumber(blocknumber.into());
    blockdetail.set_difficulty(u256_to_vec(block.difficulty.into()));
    blockdetail.set_extraData(block.extra_data.into());
    blockdetail.set_hash(blockhash.0.to_vec());
    blockdetail.set_logsBloom(block.logs_bloom.0.to_vec());
    blockdetail.set_minerAddress(block.miner.0.to_vec());
    blockdetail.set_nonce(block.nonce.map_or_else(|| Vec::new(), Into::into));
    blockdetail.set_nrgConsumed(block.gas_used.into());
    blockdetail.set_nrgLimit(block.gas_limit.into());
    blockdetail.set_parentHash(block.parent_hash.0.to_vec());
    blockdetail.set_timestamp(block.timestamp.into());
    blockdetail.set_txTrieRoot(block.transactions_root.0.to_vec());
    blockdetail.set_receiptTrieRoot(block.receipts_root.0.to_vec());
    blockdetail.set_stateRoot(block.state_root.0.to_vec());
    blockdetail.set_size(blocksize.into());
    blockdetail.set_solution(block.solution.map_or_else(|| Vec::new(), Into::into));
    blockdetail.set_totalDifficulty(u256_to_vec(total_difficulty));
    let txs = match block.transactions {
        BlockTransactions::Full(txs) => txs,
        _ => vec![],
    };
    let txs = txs
        .iter()
        .zip(br.iter())
        .into_iter()
        .map(|(tx, re)| create_t_tx_detail(tx.clone(), re.clone()))
        .collect::<Vec<_>>();
    blockdetail.set_tx(txs.into());
    blockdetail
}

fn create_t_tx_detail(tx: Transaction, re: SimpleReceipt) -> t_TxDetail {
    let to = tx.to.map_or_else(|| H256::from(0), |a| a.into());
    let index = tx
        .transaction_index
        .map_or_else(|| U256::from(0), |a| a.into());
    let mut txdetail = t_TxDetail::new();
    txdetail.set_data(tx.input.into_vec());
    txdetail.set_to(to.0.to_vec());
    txdetail.set_from(tx.from.0.to_vec());
    txdetail.set_nonce(u256_to_vec(tx.nonce.into()));
    txdetail.set_value(u256_to_vec(tx.value.into()));
    txdetail.set_nrgConsumed(tx.gas.into());
    txdetail.set_nrgPrice(tx.gas_price.into());
    txdetail.set_txHash(tx.hash.0.to_vec());
    txdetail.set_txIndex(index.into());

    let tles = re
        .logs
        .iter()
        .map(|log| {
            let mut tle = t_LgEle::new();
            tle.set_data(log.data.clone().into_vec());
            tle.set_address(log.address.clone().0.to_vec());
            let topics: Vec<String> = log
                .topics
                .clone()
                .into_iter()
                .map(|t| t.0.to_vec().to_hex())
                .collect();
            tle.set_topics(topics.into());
            tle
        })
        .collect::<Vec<_>>();
    txdetail.set_logs(tles.into());
    txdetail
}

fn create_block_msg(block: Option<Block>) -> Vec<u8> {
    if block.is_none() {
        return to_return_header(
            get_api_version(),
            Retcode::r_fail_function_arguments.value(),
        );
    }
    let block = block.unwrap();
    let blockhash = block.hash.map_or_else(|| H256::from(0), |a| a.into());
    let blocknumber = block.number.unwrap_or(0u64);
    let blocksize = block.size.map_or_else(|| U256::from(0), |a| a.into());
    let total_difficulty = block
        .total_difficulty
        .map_or_else(|| U256::from(0), |a| a.into());
    let mut rsp = rsp_getBlock::new();
    rsp.set_parentHash(block.parent_hash.0.to_vec());
    rsp.set_minerAddress(block.miner.0.to_vec());
    rsp.set_stateRoot(block.state_root.0.to_vec());
    rsp.set_txTrieRoot(block.transactions_root.0.to_vec());
    rsp.set_difficulty(u256_to_vec(block.difficulty.into()));
    rsp.set_extraData(block.extra_data.into());
    rsp.set_nrgConsumed(block.gas_used.into());
    rsp.set_nrgLimit(block.gas_limit.into());
    rsp.set_hash(blockhash.0.to_vec());
    rsp.set_logsBloom(block.logs_bloom.0.to_vec());
    rsp.set_nonce(block.nonce.map_or_else(|| Vec::new(), Into::into));
    rsp.set_receiptTrieRoot(block.receipts_root.0.to_vec());
    rsp.set_timestamp(block.timestamp.into());
    rsp.set_blockNumber(blocknumber.into());
    rsp.set_solution(block.solution.map_or_else(|| Vec::new(), Into::into));
    rsp.set_size(blocksize.into());
    rsp.set_totalDifficulty(u256_to_vec(total_difficulty));
    let txs: Vec<H256> = match block.transactions {
        BlockTransactions::Hashes(txs) => txs.into_iter().map(|h| h.into()).collect(),
        _ => vec![],
    };
    let txs: Vec<Vec<u8>> = txs.into_iter().map(|h| h.0.to_vec()).collect();
    rsp.set_txHash(txs.into());
    let retbody = api_try!(rsp.write_to_bytes());
    let retheader = to_return_header(get_api_version(), Retcode::r_success.value());
    combine_ret_msg(retheader, retbody)
}

//=========================================================================
//============================= tx io handler =============================
//=========================================================================

struct TxIoHandler {
    msg_id_mapping: Arc<RwLock<HashMap<H256, SimpleEntry>>>,
    pending_status: Arc<MsQueue<TxPendingStatus>>,
}

use io::IoContext;
use std::thread::sleep;
use std::time::Duration;

impl IoHandler<TxIoMessage> for TxIoHandler {
    fn message(&self, _io: &IoContext<TxIoMessage>, net_message: &TxIoMessage) {
        sleep(Duration::from_millis(1000));
        let (state, tx_hash, result, error) = match *net_message {
            TxIoMessage::Included {
                txhash,
                ref result,
            } => {
                debug!(
                    target: LOG_TARGET,
                    "update tx  txhash:[{:?}], status: [{}]",
                    txhash,
                    "Included"
                );
                (3, txhash, result.clone(), "".into())
            }
            TxIoMessage::Dropped {
                txhash,
                ref error,
            } => {
                debug!(
                    target: LOG_TARGET,
                    "update tx  txhash:[{:?}], status: [{}], error:[{}]",
                    txhash,
                    "Dropped",
                    error
                );
                (0, txhash, vec![0], error.clone())
            }
        };

        let entry_option = {
            let mut msg_id_mapping = self.msg_id_mapping.read();

            match msg_id_mapping.get(&tx_hash) {
                Some(entry) => Some(entry.clone()),
                None => None,
            }
        };

        match entry_option {
            Some(entry) => {
                let tx_status = TxPendingStatus {
                    tx_hash: tx_hash.clone(),
                    socket_id: entry.value,
                    msg_hash: entry.key,
                    tx_result: result,
                    error,
                    state,
                };
                self.pending_status.push(tx_status);
                let mut msg_id_mapping = self.msg_id_mapping.write();
                msg_id_mapping.remove(&tx_hash);
                debug!(target: LOG_TARGET, "msgIdMapping remove tx:{:?}", tx_hash);
            }
            None => {
                debug!(target: LOG_TARGET, "msg_id_mapping is none");
            }
        }
    }
}

//=========================================================================
//============================= utils =====================================
//=========================================================================

fn u256_to_vec(u: U256) -> Vec<u8> {
    let u: [u8; 32] = u.into();
    u.to_vec()
}
