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
use std::sync::Arc;
use std::collections::HashMap;

use parking_lot::RwLock;

use client::{BlockId, BlockChainClient};
use client::BlockStatus;
use header::Seal;
use views::BlockView;
use sync::storage::SyncStorage;
use sync::node_info::{NodeInfo, Mode};

// pub fn import_staged_blocks(hash: &H256) {
//    let mut blocks_to_import = Vec::new();
//    if let Ok(mut staged_blocks) = SyncStorage::get_staged_blocks().lock() {
//        if staged_blocks.contains_key(&hash) {
//            if let Some(blocks_staged) = staged_blocks.remove(hash) {
//                blocks_to_import.extend(blocks_staged);
//            }
//        }
//    }
//
//    if blocks_to_import.len() > 0 {
//        let client = SyncStorage::get_block_chain();
//        let mut enable_import = true;
//        for block in blocks_to_import.iter() {
//            let block_view = BlockView::new(block);
//            let header_view = block_view.header_view();
//            let number = header_view.number();
//            let hash = header_view.hash();
//
//            if enable_import {
//                SyncStorage::insert_requested_time(hash);
//                match client.import_block(block.clone()) {
//                    Ok(_)
//                    | Err(BlockImportError::Import(ImportError::AlreadyInChain))
//                    | Err(BlockImportError::Import(ImportError::AlreadyQueued)) => {
//                        trace!(target: "sync", "Staged block #{} imported...", number);
//                    }
//                    Err(e) => {
//                        enable_import = false;
//                        warn!(target: "sync", "Failed to import staged block #{}, due to {:?}", number, e);
//                    }
//                }
//            }
//        }
//    }
// }

pub fn import_blocks(
    client: Arc<BlockChainClient>,
    storage: Arc<SyncStorage>,
    nodes_info: Arc<RwLock<HashMap<u64, RwLock<NodeInfo>>>>,
)
{
    // Get downloaded blocks
    let mut downloaded_blocks = Vec::new();
    let mut blocks_wrappers = storage.downloaded_blocks().lock();
    while let Some(blocks_wrapper) = blocks_wrappers.pop_front() {
        downloaded_blocks.push(blocks_wrapper);
    }
    drop(blocks_wrappers);

    // Import blocks for each batch
    for blocks_wrapper in downloaded_blocks {
        // Filter blocks to import
        let mut blocks_to_import = Vec::new();
        for block in blocks_wrapper.blocks {
            let status = client.block_status(BlockId::Hash(block.header.hash()));
            if status == BlockStatus::Unknown {
                blocks_to_import.push(block.rlp_bytes(Seal::With));
            } else if status == BlockStatus::Bad {
                // TODO: need p2p to provide log infomation
                // warn!(target: "sync", "Bad block {}, {:?}, got from node: {}@{}, mode: {}", block.header.number(), block.header.hash(), node.get_node_id(), node.get_ip_addr(), node.mode);
                // Stop this batch when got bad block
                break;
            }
        }

        if blocks_to_import.is_empty() {
            continue;
        }

        // Import blocks
        let mut last_imported_number: u64 = 0;
        for block in blocks_to_import {
            // TODO: need p2p to provide log infomation
            // let block_view = BlockView::new(&block);
            // let (hash, number, parent, difficulty) = {
            //     let header_view = block_view.header_view();
            //     (
            //         header_view.hash(),
            //         header_view.number(),
            //         header_view.parent_hash(),
            //         header_view.difficulty(),
            //     )
            // };
            // debug!(target: "sync", "hash: {}, number: {}, parent: {}, node id: {}, mode: {}, synced_block_number: {}", hash, number, parent, node.get_node_id(), node.mode, node.synced_block_num);

            let block_view = BlockView::new(&block);
            last_imported_number = block_view.header_view().number();

            let _result = client.import_block(block.to_vec());
            // TODO:
            // Parse result with different sync modes.
            // Add repeat threshold mechanism to remove peer
        }

        let node_hash = blocks_wrapper.node_hash;
        if let Some(node_info) = nodes_info.read().get(&node_hash) {
            let mut info = node_info.write();
            let node_best_number = info.best_block_number;
            if last_imported_number + 32 >= node_best_number {
                if info.mode != Mode::NORMAL {
                    debug!(target:"sync", "switch to NORMAL mode: last imported {}, node best: {}, node hash: {}", &last_imported_number, &node_best_number, &node_hash);
                    info.mode = Mode::NORMAL;
                }
            } else {
                if info.mode != Mode::THUNDER {
                    debug!(target:"sync", "switch to THUNDER mode: last imported {}, node best: {}, node hash: {}", &last_imported_number, &node_best_number, &node_hash);
                    info.mode = Mode::THUNDER;
                }
            }
        }

        // TODO: maybe we should consider reset the header request cooldown here
    }
}
