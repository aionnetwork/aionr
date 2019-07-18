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

use client::{BlockId, BlockImportError, BlockStatus};
use types::error::{BlockError, ImportError};
use header::{Seal,SealType};
use views::BlockView;
use aion_types::{H256, U256};

use p2p::{Mode, P2pMgr};
use super::super::storage::SyncStorage;
pub struct ImportHandler;
use super::blocks_headers_handler::BlockHeadersHandler;

impl ImportHandler {
    pub fn import_staged_blocks(hash: &H256) {
        let mut blocks_to_import = Vec::new();
        if let Ok(mut staged_blocks) = SyncStorage::get_staged_blocks().lock() {
            if staged_blocks.contains_key(&hash) {
                if let Some(blocks_staged) = staged_blocks.remove(hash) {
                    blocks_to_import.extend(blocks_staged);
                }
            }
        }

        if blocks_to_import.len() > 0 {
            let client = SyncStorage::get_block_chain();
            let mut enable_import = true;
            for block in blocks_to_import.iter() {
                let block_view = BlockView::new(block);
                let header_view = block_view.header_view();
                let number = header_view.number();
                let hash = header_view.hash();

                if enable_import {
                    SyncStorage::insert_requested_time(hash);
                    match client.import_block(block.clone()) {
                        Ok(_)
                        | Err(BlockImportError::Import(ImportError::AlreadyInChain))
                        | Err(BlockImportError::Import(ImportError::AlreadyQueued)) => {
                            trace!(target: "sync", "Staged block #{} imported...", number);
                        }
                        Err(e) => {
                            enable_import = false;
                            warn!(target: "sync", "Failed to import staged block #{}, due to {:?}", number, e);
                        }
                    }
                }
            }
        }
    }

    pub fn import_blocks() {
        let mut blocks_to_import = Vec::new();
        let client = SyncStorage::get_block_chain();
        let mut bws = Vec::new();

        if let Ok(ref mut downloaded_blocks) = SyncStorage::get_downloaded_blocks().try_lock() {
            while let Some(bw) = downloaded_blocks.pop_front() {
                bws.push(bw);
            }
        } else {
            warn!(target: "sync", "import_block fail to get downloaded blocks.");
        }

        for bw in bws.iter() {
            if let Some(mut node) = P2pMgr::get_node(bw.node_id_hash) {
                blocks_to_import.clear();
                let mut max_block_number = 1;
                for block in bw.blocks.iter() {
                    let status = client.block_status(BlockId::Hash(block.header.hash()));
                    if status == BlockStatus::Unknown {
                        blocks_to_import.push(block.rlp_bytes(Seal::With));
                    } else if status == BlockStatus::Bad {
                        warn!(target: "sync", "Bad block {}, {:?}, got from node: {}@{}, mode: {}", block.header.number(), block.header.hash(), node.get_node_id(), node.get_ip_addr(), node.mode);
                        // node.mode = Mode::BACKWARD;
                        // P2pMgr::update_node_with_mode(node.node_hash, &node);
                        break;
                    } else if max_block_number < block.header.number() {
                        max_block_number = block.header.number();
                    }
                }

                if let Some(mut node) = P2pMgr::get_node(bw.node_id_hash) {
                    if blocks_to_import.is_empty() {
                        match node.mode {
                            Mode::BACKWARD => {
                                info!(target: "sync", "Node: {}, the fork point #{} found, switched from BACKWARD mode to FORWARD mode", node.get_node_id(), max_block_number);
                                node.mode = Mode::FORWARD;
                                node.synced_block_num = max_block_number;
                            }
                            Mode::FORWARD => {
                                if SyncStorage::get_synced_block_number_last_time()
                                    == SyncStorage::get_synced_block_number()
                                {
                                    SyncStorage::set_synced_block_number_last_time(
                                        max_block_number,
                                    );
                                    SyncStorage::set_synced_block_number(max_block_number);
                                } else {
                                    // } else {
                                    // ------
                                    // FIX:
                                    //   synced_block_number is set when new block imported successfully
                                    //   synced_block_number_last_time is set to the local best block
                                    //   when doing a deep chain reorg, steps will be:
                                    //     1. BACKWARD syncing till the fork point
                                    //     2. FORWARD syncing to the highest (total difficulty) block
                                    //   During step 2, if it can't be done within one syncing batch, synced_block_number will not be equal to synced_block_number_last_time. In this case, we can't switch to NORMAL because the FORWARD syncing is not finished yet.
                                    // ------
                                    // } else if SyncStorage::get_synced_block_number() >= SyncStorage::get_network_best_block_number() {
                                    info!(target: "sync", "Node: {}, the best block #{} found, switched from FORWARD mode to NORMAL mode", node.get_node_id(), max_block_number);
                                    node.mode = Mode::NORMAL;
                                }
                                node.synced_block_num = max_block_number;
                            }
                            _ => {
                                if max_block_number > node.synced_block_num {
                                    if node.synced_block_num + 32
                                        > SyncStorage::get_network_best_block_number()
                                    {
                                        node.mode = Mode::NORMAL;
                                    } else {
                                        let (
                                            normal_nodes_count,
                                            _,
                                            _,
                                            lightning_nodes_count,
                                            thunder_nodes_count,
                                        ) = P2pMgr::get_nodes_count_all_modes();
                                        if normal_nodes_count == 0 {
                                            node.mode = Mode::NORMAL;
                                        } else if node.target_total_difficulty
                                            >= SyncStorage::get_network_total_diff()
                                            && node.synced_block_num + 500
                                                < SyncStorage::get_network_best_block_number()
                                            && thunder_nodes_count >= 1
                                            && lightning_nodes_count
                                                < (thunder_nodes_count + normal_nodes_count) / 5
                                        {
                                            // attempt to jump
                                            node.mode = Mode::LIGHTNING;
                                        } else if thunder_nodes_count < normal_nodes_count * 10 {
                                            node.mode = Mode::THUNDER;
                                        } else {
                                            node.mode = Mode::NORMAL;
                                        }
                                    }
                                }
                            }
                        }
                        P2pMgr::update_node_with_mode(node.node_hash, &node);
                        if node.mode == Mode::NORMAL || node.mode == Mode::THUNDER {
                            if SyncStorage::get_synced_block_number() + 8
                                < SyncStorage::get_network_best_block_number()
                            {
                                BlockHeadersHandler::get_headers_from_node(&mut node);
                            }
                        }
                        continue;
                    }

                    let mut offset = 0;
                    for block in blocks_to_import.iter() {
                        offset += 1;
                        let block_view = BlockView::new(block);
                        let (hash, number, parent, difficulty, seal_type) = {
                            let header_view = block_view.header_view();
                            (
                                header_view.hash(),
                                header_view.number(),
                                header_view.parent_hash(),
                                header_view.difficulty(),
                                header_view.seal_type().unwrap_or_default(),
                            )
                        };

                        debug!(target: "sync", "hash: {}, number: {}, parent: {}, node id: {}, mode: {}, synced_block_number: {}",
                        hash, number, parent, node.get_node_id(), node.mode, node.synced_block_num);

                        let result = client.import_block(block.clone());
                        SyncStorage::insert_requested_time(hash);
                        match result {
                            Ok(_)
                            | Err(BlockImportError::Import(ImportError::AlreadyInChain))
                            | Err(BlockImportError::Import(ImportError::AlreadyQueued)) => {
                                let block_id = BlockId::Hash(hash);
                                let status = client.block_status(block_id);
                                // if status == BlockStatus::Unknown || status == BlockStatus::Bad {
                                //     continue;
                                // }

                                match seal_type {
                                    SealType::PoW => {
                                        node.current_pow_total_difficulty =
                                            node.current_pow_total_difficulty + difficulty;
                                    }
                                    SealType::PoS => {
                                        node.current_pos_total_difficulty =
                                            node.current_pos_total_difficulty + difficulty;
                                    }
                                }
                                // TODO-UNITY: add overflow check
                                node.current_total_difficulty = node.current_pow_total_difficulty
                                    * node.current_pos_total_difficulty;

                                node.synced_block_num = number;
                                if result.is_err() {
                                    if status == BlockStatus::InChain {
                                        if let Some((
                                            current_total_difficulty,
                                            current_pow_total_difficulty,
                                            current_pos_total_difficulty,
                                        )) = client.block_total_difficulty(block_id)
                                        {
                                            node.current_total_difficulty =
                                                current_total_difficulty;
                                            node.current_pow_total_difficulty =
                                                current_pow_total_difficulty;
                                            node.current_pos_total_difficulty =
                                                current_pos_total_difficulty;
                                        }
                                    }
                                    info!(target: "sync", "AlreadyStored block #{}, {:?} received from node {}", number, hash, node.get_node_id());
                                } else {
                                    debug!(target: "sync", "Best block #{}, {:?} imported from node {}", number, hash, node.get_node_id());
                                }

                                node.reset_repeated();

                                match node.mode {
                                    Mode::BACKWARD => {
                                        info!(target: "sync", "Node: {}, found the fork point #{}, with status {:?}, switched to FORWARD mode", node.get_node_id(), number, status);
                                        node.mode = Mode::FORWARD;
                                        P2pMgr::update_node_with_mode(node.node_hash, &node);
                                        // break;
                                    }
                                    Mode::FORWARD => {
                                        info!(target: "sync", "Node: {}, found the best block #{} with status {:?}, switched to NORMAL mode", node.get_node_id(), number, status);
                                        node.mode = Mode::NORMAL;
                                        // info!(target: "sync", "Node: {}, found the best block #{} with status {:?}, switched to NORMAL mode", node.get_node_id(), number, status);
                                        // node.mode = Mode::NORMAL;
                                        // ------
                                        // FIX: Same as above FIX
                                        // ------
                                        // if number >= SyncStorage::get_network_best_block_number() {
                                        //   info!(target: "sync", "Node: {}, found the best block #{} with status {:?}, switched to NORMAL mode", node.get_node_id(), number, status);
                                        //   node.mode = Mode::NORMAL;
                                        // }
                                        P2pMgr::update_node_with_mode(node.node_hash, &node);
                                        // break;
                                    }
                                    _ => {
                                        if node.synced_block_num + 32
                                            > SyncStorage::get_network_best_block_number()
                                        {
                                            node.mode = Mode::NORMAL;
                                        } else {
                                            let (
                                                normal_nodes_count,
                                                _,
                                                _,
                                                lightning_nodes_count,
                                                thunder_nodes_count,
                                            ) = P2pMgr::get_nodes_count_all_modes();
                                            if normal_nodes_count == 0 {
                                                node.mode = Mode::NORMAL;
                                            } else if node.target_total_difficulty
                                                >= SyncStorage::get_network_total_diff()
                                                && node.synced_block_num + 500
                                                    < SyncStorage::get_network_best_block_number()
                                                && thunder_nodes_count >= 1
                                                && lightning_nodes_count
                                                    < (thunder_nodes_count + normal_nodes_count) / 5
                                            {
                                                // attempt to jump
                                                node.mode = Mode::LIGHTNING;
                                            } else if thunder_nodes_count < normal_nodes_count * 10
                                            {
                                                node.mode = Mode::THUNDER;
                                            } else {
                                                node.mode = Mode::NORMAL;
                                            }
                                        }
                                        P2pMgr::update_node_with_mode(node.node_hash, &node);
                                    }
                                }
                            }
                            Err(BlockImportError::Block(BlockError::UnknownParent(_))) => {
                                if number == 1 {
                                    error!(target: "sync", "Invalid genesis !!!");

                                    break;
                                }

                                if number > SyncStorage::get_synced_block_number() {
                                    // put into staging...
                                    if let Ok(mut staged_blocks) =
                                        SyncStorage::get_staged_blocks().lock()
                                    {
                                        if staged_blocks.len() < 32
                                            && !staged_blocks.contains_key(&parent)
                                        {
                                            let blocks_to_stage =
                                                blocks_to_import.clone().split_off(offset - 1);
                                            let max_staged_block_number =
                                                number + blocks_to_stage.len() as u64 - 1;
                                            info!(target: "sync", "Staged blocks from {} to {} with parent: {}", number, max_staged_block_number, parent);
                                            debug!(target: "sync", "cache size: {}", staged_blocks.len());

                                            let mut staged_block_hashes = Vec::new();
                                            for block in blocks_to_import.iter() {
                                                let block_view = BlockView::new(block);
                                                let hash = block_view.header_view().hash();
                                                staged_block_hashes.push(hash);
                                            }

                                            SyncStorage::insert_staged_block_hashes(
                                                staged_block_hashes,
                                            );

                                            staged_blocks.insert(parent, blocks_to_stage);

                                            if max_staged_block_number
                                                > SyncStorage::get_max_staged_block_number()
                                            {
                                                SyncStorage::set_max_staged_block_number(
                                                    max_staged_block_number,
                                                );
                                            }
                                        } else {
                                            node.synced_block_num =
                                                client.chain_info().best_block_number;
                                            node.mode = Mode::THUNDER;
                                            P2pMgr::update_node_with_mode(node.node_hash, &node);
                                        }
                                        break;
                                    }
                                } else {
                                    node.current_total_difficulty = U256::from(0);
                                    node.current_pow_total_difficulty = U256::from(0);
                                    node.current_pos_total_difficulty = U256::from(0);
                                    node.synced_block_num = number;

                                    if node.target_total_difficulty
                                        < SyncStorage::get_network_total_diff()
                                    {
                                        P2pMgr::remove_peer(node.node_hash);
                                    }
                                    match node.mode {
                                        Mode::LIGHTNING | Mode::THUNDER => {
                                            warn!(target: "sync", "Unknown block: #{}, node {} run in NORMAL mode now.", number, node.get_node_id());
                                            node.mode = Mode::NORMAL;
                                        }
                                        Mode::FORWARD | Mode::NORMAL => {
                                            warn!(target: "sync", "Unknown block: #{}, node {} run in BACKWARD mode now.", number, node.get_node_id());
                                            node.mode = Mode::BACKWARD;
                                            node.last_request_num = 0;
                                        }
                                        Mode::BACKWARD => {
                                            warn!(target: "sync", "Unknown block: #{}, node {} run in BACKWARD mode.", number, node.get_node_id());
                                        }
                                    }
                                    P2pMgr::update_node_with_mode(node.node_hash, &node);
                                    break;
                                }
                            }
                            Err(e) => {
                                if !node.is_over_repeated_threshold() {
                                    warn!(target: "sync", "Got bad block #{}, {:?}", number, hash);

                                    node.mode = Mode::BACKWARD;
                                    client.clear_bad();
                                    node.inc_repeated();
                                    P2pMgr::update_node_with_mode(node.node_hash, &node);
                                } else {
                                    warn!(target: "sync", "Bad block {:?} {:?}, remove peer node: {}@{}", hash, e, node.get_node_id(), node.get_ip_addr());
                                    P2pMgr::remove_peer(node.node_hash);
                                }
                                break;
                            }
                        }
                    }

                    if node.mode == Mode::NORMAL || node.mode == Mode::THUNDER {
                        node.synced_block_num = SyncStorage::get_synced_block_number();

                        if node.synced_block_num + 8 < SyncStorage::get_network_best_block_number()
                        {
                            BlockHeadersHandler::get_headers_from_node(&mut node);
                        }
                    }
                }
            }
        }
    }
}
