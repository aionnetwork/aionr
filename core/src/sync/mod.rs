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

mod event;
mod handler;
mod route;
mod storage;
#[cfg(test)]
mod test;

use std::collections::BTreeMap;
use std::ops::Index;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use rustc_hex::ToHex;
use client::BlockChainClient;
use client::BlockId;
use client::BlockStatus;
use client::ChainNotify;
use transaction::UnverifiedTransaction;
use aion_types::H256;
use futures::Future;
use futures::Stream;
use rlp::UntrustedRlp;
use tokio::runtime::TaskExecutor;
use tokio::timer::Interval;

// chris
use std::thread;
use p2p::handler::external::Handler;
use p2p::Node;
use p2p::ChannelBuffer;
use p2p::Config;
use p2p::register;
use p2p::enable as p2p_start;
use p2p::reset as p2p_shutdown;
use p2p::get_nodes;
use p2p::get_nodes_count;
use p2p::get_all_nodes;
use p2p::get_all_nodes_count;
use p2p::get_local_node;
use p2p::states::STATE::HANDSHAKEDONE;
use p2p::states::STATE::CONNECTED;
use p2p::states::STATE::ALIVE;
use p2p::Mgr;

use sync::route::VERSION;
use sync::route::ACTION;
use sync::handler::status;
use sync::handler::bodies;
use sync::handler::headers;
use sync::handler::broadcast;
use sync::handler::import;
use sync::storage::ActivePeerInfo;
use sync::storage::PeerInfo;
use sync::storage::SyncState;
use sync::storage::SyncStatus;
use sync::storage::SyncStorage;
use sync::storage::TransactionStats;

const STATUS_REQ_INTERVAL: u64 = 2;
const BLOCKS_BODIES_REQ_INTERVAL: u64 = 50;
const BLOCKS_IMPORT_INTERVAL: u64 = 50;
const STATICS_INTERVAL: u64 = 15;
const BROADCAST_TRANSACTIONS_INTERVAL: u64 = 50;

#[derive(Clone)]
struct SyncMgr;

impl SyncMgr {
    fn enable(executor: &TaskExecutor, max_peers: u32) {
        let status_req_task =
            Interval::new(Instant::now(), Duration::from_secs(STATUS_REQ_INTERVAL))
                .for_each(move |_| {
                    let active_nodes = get_nodes(ALIVE.value());
                    for node in active_nodes.iter() {
                        trace!(target: "sync", "Sync status req sent...");
                        status::send(node.node_hash);
                    }
                    Ok(())
                })
                .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(status_req_task);

        let blocks_bodies_req_task = Interval::new(
            Instant::now(),
            Duration::from_millis(BLOCKS_BODIES_REQ_INTERVAL),
        )
        .for_each(move |_| {
            bodies::send();
            Ok(())
        })
        .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(blocks_bodies_req_task);

        let blocks_import_task = Interval::new(
            Instant::now(),
            Duration::from_millis(BLOCKS_IMPORT_INTERVAL),
        )
        .for_each(move |_| {
            import::import_blocks();
            Ok(())
        })
        .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(blocks_import_task);

        let broadcast_transactions_task = Interval::new(
            Instant::now(),
            Duration::from_millis(BROADCAST_TRANSACTIONS_INTERVAL),
        )
        .for_each(move |_| {
            broadcast::propagate_transactions();
            Ok(())
        })
        .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(broadcast_transactions_task);

        let statics_task = Interval::new(Instant::now(), Duration::from_secs(STATICS_INTERVAL))
            .for_each(move |_| {
                // let connected_nodes = get_nodes(CONNECTED.value());
                // for node in connected_nodes.iter() {
                //     if node.mode == Mode::BACKWARD || node.mode == Mode::FORWARD {
                //         if node.target_total_difficulty < SyncStorage::get_network_total_diff() {
                //             remove_peer(node.node_hash);
                //         }
                //     } else if node.last_request_timestamp
                //         + Duration::from_secs(STATICS_INTERVAL * 12)
                //         < SystemTime::now()
                //     {
                //         info!(target: "sync", "Disconnect with idle node: {}@{}.", node.get_node_id(), node.get_ip_addr());
                //         remove_peer(node.node_hash);
                //     }
                // }

                let chain_info = SyncStorage::get_chain_info();
                let block_number_last_time = SyncStorage::get_synced_block_number_last_time();
                let block_number_now = chain_info.best_block_number;
                let sync_speed = (block_number_now - block_number_last_time) / STATICS_INTERVAL;
                let mut active_nodes = get_nodes(ALIVE.value());
                let active_nodes_count = active_nodes.len();

                info!(target: "sync", "");
                info!(target: "sync", "{:=^127}", "  sync  ");
                info!(target: "sync", "  local num:{:>11}, hash:{:>20}", chain_info.best_block_number, chain_info.best_block_hash);
                info!(target: "sync", "network num:{:>11}, hash:{:>20}", SyncStorage::get_network_best_block_number(), SyncStorage::get_network_best_block_hash());
                info!(target: "sync", " staged num:{:>11}", SyncStorage::get_max_staged_block_number());
                info!(target: "sync", "       sync:{:>11} blks/sec", sync_speed);
                info!(target: "sync",
                    "total/connected/active peers: {}/{}/{}",
                    get_all_nodes_count(),
                    get_nodes_count(CONNECTED.value()),
                    active_nodes_count,
                );

                if active_nodes_count > 0 {
                    info!(target: "sync", "{:-^127}","");
                    info!(target: "sync","              td         bn          bh                         addr                 rev      conn  seed      lst-req          m");
                    info!(target: "sync", "{:-^127}","");
                    active_nodes.sort_by(|a, b| {
                        if a.target_total_difficulty != b.target_total_difficulty {
                            b.target_total_difficulty.cmp(&a.target_total_difficulty)
                        } else {
                            b.best_block_num.cmp(&a.best_block_num)
                        }
                    });
                    let mut count: u32 = 0;
                    for node in active_nodes.iter() {
                        if let Ok(_) = node.last_request_timestamp.elapsed() {
                            info!(target: "sync",
                                "{:>16}{:>11}{:>12}{:>24}{:>25}{:>10}{:>6}{:>12}{:>11}",
                                format!("{}",node.target_total_difficulty),
                                node.best_block_num,
                                format!("{}",node.best_hash),
                                node.get_display_ip_addr(),
                                String::from_utf8_lossy(&node.revision).trim(),
                                match node.ip_addr.is_server{
                                    true => "Outbound",
                                    _=>"Inbound"
                                },
                                match node.is_from_boot_list{
                                    true => "Y",
                                    _ => ""
                                },
                                node.last_request_num,
                                format!("{}",node.mode)
                            );
                            count += 1;
                            if count ==  max_peers {
                                break;
                            }
                        }
                    }
                    info!(target: "sync", "{:-^127}","");
                }

                if block_number_now + 8 < SyncStorage::get_network_best_block_number()
                    && block_number_now - block_number_last_time < 2
                {
                    SyncStorage::get_block_chain().clear_queue();
                    SyncStorage::get_block_chain().clear_bad();
                    SyncStorage::clear_downloaded_headers();
                    SyncStorage::clear_downloaded_blocks();
                    SyncStorage::clear_downloaded_block_hashes();
                    SyncStorage::clear_requested_blocks();
                    SyncStorage::clear_headers_with_bodies_requested();
                    SyncStorage::set_synced_block_number(
                        SyncStorage::get_chain_info().best_block_number,
                    );
                    // let abnormal_mode_nodes_count =
                    //     get_nodes_count_with_mode(Mode::BACKWARD)
                    //         + get_nodes_count_with_mode(Mode::FORWARD);
                    // if abnormal_mode_nodes_count > (active_nodes_count / 5)
                    //     || active_nodes_count == 0
                    // {
                    //     info!(target: "sync", "Abnormal status, reseting network...");
                    //     reset();

                    //     SyncStorage::clear_imported_block_hashes();
                    //     SyncStorage::clear_staged_blocks();
                    //     SyncStorage::set_max_staged_block_number(0);
                    // }
                }

                // if block_number_now + 8 < SyncStorage::get_network_best_block_number()
                //     && block_number_now - block_number_last_time < 2
                // {
                //     SyncStorage::get_block_chain().clear_queue();
                //     SyncStorage::get_block_chain().clear_bad();
                //     SyncStorage::clear_downloaded_headers();
                //     SyncStorage::clear_downloaded_blocks();
                //     SyncStorage::clear_downloaded_block_hashes();
                //     SyncStorage::clear_requested_blocks();
                //     SyncStorage::clear_headers_with_bodies_requested();
                //     SyncStorage::set_synced_block_number(
                //         SyncStorage::get_chain_info().best_block_number,
                //     );
                //     let abnormal_mode_nodes_count =
                //         get_nodes_count_with_mode(Mode::BACKWARD)
                //             + get_nodes_count_with_mode(Mode::FORWARD);
                //     if abnormal_mode_nodes_count > (active_nodes_count / 5)
                //         || active_nodes_count == 0
                //     {
                //         info!(target: "sync", "Abnormal status, reseting network...");
                //         reset();

                //         SyncStorage::clear_imported_block_hashes();
                //         SyncStorage::clear_staged_blocks();
                //         SyncStorage::set_max_staged_block_number(0);
                //     }
                // }
                // ------
                // FIX: abnormal reset will be triggered in chain reorg.
                //   block_number_now is the local best block
                //   network_best_block_number is the network best block
                //   block_number_last_time is the local best block set last time where these codes are executed
                //   when doing a deep chain reorg, if the BACKWARD and FORWARD syncing can't finish within one data batch, block_number_now acctually won't change
                //   so we will have block_number_now == block_number_last_time, and block_number_now smaller than network_best_block_number.
                //   This condition triggers reset but it's not abnormal.
                // PoC disabled it. We should fix it.
                // ------

                SyncStorage::set_synced_block_number_last_time(block_number_now);
                SyncStorage::set_sync_speed(sync_speed as u16);

                if SyncStorage::get_network_best_block_number()
                    <= SyncStorage::get_synced_block_number()
                {
                    // full synced
                    SyncStorage::clear_staged_blocks();
                }

                Ok(())
            })
            .map_err(|e| error!("interval errored; err={:?}", e));
        executor.spawn(statics_task);
    }

    fn handle(node: &mut Node, req: ChannelBuffer) {
        if node.state_code & HANDSHAKEDONE.value() != HANDSHAKEDONE.value() {
            return;
        }

        match VERSION::from(req.head.ver) {
            VERSION::V0 => {
                trace!(target: "sync", "version {0} module {1} action{2}",
                    req.head.ver,
                    req.head.ctrl,
                    req.head.action
                );
                match ACTION::from(req.head.action) {
                    ACTION::STATUSREQ => {
                        status::receive_req(node);
                    }
                    ACTION::STATUSRES => {
                        status::receive_res(node, req);
                    }
                    ACTION::BLOCKSHEADERSREQ => {
                        headers::handle_blocks_headers_req(node, req);
                    }
                    ACTION::BLOCKSHEADERSRES => {
                        headers::handle_blocks_headers_res(node, req);
                    }
                    ACTION::BLOCKSBODIESREQ => {
                        bodies::receive_req(node, req);
                    }
                    ACTION::BLOCKSBODIESRES => {
                        bodies::receive_res(node, req);
                    }
                    ACTION::BROADCASTTX => {
                        broadcast::receive_tx(node, req);
                    }
                    ACTION::BROADCASTBLOCK => {
                        broadcast::receive_block(node, req);
                    }
                    _ => {
                        trace!(target: "sync", "UNKNOWN received.");
                    }
                }
            }
            VERSION::V1 => {
                trace!(target: "sync", "Ver 1 package received.");
            }
        };
    }

    fn disable() { SyncStorage::reset(); }
}

pub struct Sync {
    /// Network service
    config: Config,
    /// starting block number.
    starting_block_number: u64,
}

impl Sync {
    pub fn new(client: Arc<BlockChainClient>, config: Config) -> Arc<Sync> {
        let chain_info = client.chain_info();
        // starting block number is the local best block number during kernel startup.
        let starting_block_number = chain_info.best_block_number;

        SyncStorage::init(client);
        Arc::new(Sync {
            config,
            starting_block_number,
        })
    }

    pub fn start_network(&self) {
        let executor = SyncStorage::get_executor();
        
        // chris
        // register(Handler {
        //     callback: SyncMgr::handle,
        // });
        // p2p_start(self.config.clone());
        let p2p_mgr = Mgr::new(self.config.clone());
        p2p_mgr.run();
        thread::sleep(Duration::from_secs(10));
        p2p_mgr.shutdown(); 

        SyncMgr::enable(&executor, self.config.max_peers);
    }

    pub fn stop_network(&self) {
        SyncMgr::disable();
        
        // chris
        // original is p2p::disable which internally calls reset() with unuse atomic boolean
        // TODO: update proper ways to clear up threads and connections on p2p layer
        // p2p_shutdown();
    }
}

pub trait SyncProvider: Send + ::std::marker::Sync {
    /// Get sync status
    fn status(&self) -> SyncStatus;

    /// Get peers information
    fn peers(&self) -> Vec<PeerInfo>;

    /// Get the enode if available.
    fn enode(&self) -> Option<String>;

    /// Returns propagation count for pending transactions.
    fn transactions_stats(&self) -> BTreeMap<H256, TransactionStats>;

    /// Get active nodes
    fn active(&self) -> Vec<ActivePeerInfo>;
}

impl SyncProvider for Sync {
    /// Get sync status
    fn status(&self) -> SyncStatus {
        // TODO:  only set start_block_number/highest_block_number.
        SyncStatus {
            state: SyncState::Idle,
            protocol_version: 0,
            network_id: 256,
            start_block_number: self.starting_block_number,
            last_imported_block_number: None,
            highest_block_number: { Some(SyncStorage::get_network_best_block_number()) },
            blocks_received: 0,
            blocks_total: 0,
            num_peers: { get_nodes_count(ALIVE.value()) },
            num_active_peers: 0,
        }
    }

    /// Get sync peers
    fn peers(&self) -> Vec<PeerInfo> {
        let mut peer_info_list = Vec::new();
        let peer_nodes = get_all_nodes();
        for peer in peer_nodes.iter() {
            let peer_info = PeerInfo {
                id: Some(peer.get_node_id()),
            };
            peer_info_list.push(peer_info);
        }
        peer_info_list
    }

    fn enode(&self) -> Option<String> { Some(get_local_node().get_node_id()) }

    fn transactions_stats(&self) -> BTreeMap<H256, TransactionStats> { BTreeMap::new() }

    fn active(&self) -> Vec<ActivePeerInfo> {
        let ac_nodes = get_nodes(ALIVE.value());
        ac_nodes
            .into_iter()
            .map(|node| {
                ActivePeerInfo {
                    highest_block_number: node.best_block_num,
                    id: node.node_id.to_hex(),
                    ip: node.ip_addr.ip.to_hex(),
                }
            })
            .collect()
    }
}

impl ChainNotify for Sync {
    fn new_blocks(
        &self,
        imported: Vec<H256>,
        _invalid: Vec<H256>,
        enacted: Vec<H256>,
        _retracted: Vec<H256>,
        sealed: Vec<H256>,
        _proposed: Vec<Vec<u8>>,
        _duration: u64,
    )
    {
        if get_all_nodes_count() == 0 {
            return;
        }

        if !imported.is_empty() {
            let min_imported_block_number = SyncStorage::get_synced_block_number() + 1;
            let mut max_imported_block_number = 0;
            let client = SyncStorage::get_block_chain();
            for hash in imported.iter() {
                let block_id = BlockId::Hash(*hash);
                if client.block_status(block_id) == BlockStatus::InChain {
                    if let Some(block_number) = client.block_number(block_id) {
                        if max_imported_block_number < block_number {
                            max_imported_block_number = block_number;
                        }
                    }
                }
            }

            // The imported blocks are not new or not yet in chain. Do not notify in this case.
            if max_imported_block_number < min_imported_block_number {
                return;
            }

            let synced_block_number = SyncStorage::get_synced_block_number();
            if max_imported_block_number <= synced_block_number {
                let mut hashes = Vec::new();
                for block_number in max_imported_block_number..synced_block_number + 1 {
                    let block_id = BlockId::Number(block_number);
                    if let Some(block_hash) = client.block_hash(block_id) {
                        hashes.push(block_hash);
                    }
                }
                if hashes.len() > 0 {
                    SyncStorage::remove_imported_block_hashes(hashes);
                }
            }

            SyncStorage::set_synced_block_number(max_imported_block_number);

            for block_number in min_imported_block_number..max_imported_block_number + 1 {
                let block_id = BlockId::Number(block_number);
                if let Some(blk) = client.block(block_id) {
                    let block_hash = blk.hash();
                    import::import_staged_blocks(&block_hash);
                    if let Some(time) = SyncStorage::get_requested_time(&block_hash) {
                        info!(target: "sync",
                            "New block #{} {}, with {} txs added in chain, time elapsed: {:?}.",
                            block_number, block_hash, blk.transactions_count(), SystemTime::now().duration_since(time).expect("importing duration"));
                    }
                }
            }
        }

        if enacted.is_empty() {
            for hash in enacted.iter() {
                debug!(target: "sync", "enacted hash: {:?}", hash);
                import::import_staged_blocks(&hash);
            }
        }

        if !sealed.is_empty() {
            debug!(target: "sync", "Propagating blocks...");
            SyncStorage::insert_imported_block_hashes(sealed.clone());
            broadcast::propagate_blocks(sealed.index(0), SyncStorage::get_block_chain());
        }
    }

    fn start(&self) {
        info!(target: "sync", "starting...");
    }

    fn stop(&self) {
        info!(target: "sync", "stopping...");
    }

    fn broadcast(&self, _message: Vec<u8>) {}

    fn transactions_received(&self, transactions: &[Vec<u8>]) {
        if transactions.len() == 1 {
            let transaction_rlp = transactions[0].clone();
            if let Ok(tx) = UntrustedRlp::new(&transaction_rlp).as_val() {
                let transaction: UnverifiedTransaction = tx;
                let hash = transaction.hash();
                let sent_transaction_hashes_mutex = SyncStorage::get_sent_transaction_hashes();
                let mut lock = sent_transaction_hashes_mutex.lock();

                if let Ok(ref mut sent_transaction_hashes) = lock {
                    if !sent_transaction_hashes.contains_key(hash) {
                        sent_transaction_hashes.insert(hash.clone(), 0);
                        SyncStorage::insert_received_transaction(transaction_rlp);
                    }
                }
            }
        }
    }
}
