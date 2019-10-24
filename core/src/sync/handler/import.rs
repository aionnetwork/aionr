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

use client::{BlockId, BlockChainClient, BlockStatus, BlockImportError};
use types::error::{BlockError, ImportError};
use header::Seal;
use views::BlockView;
use sync::storage::SyncStorage;
use sync::node_info::{NodeInfo, Mode};
use aion_types::H256;

pub fn import_staged_blocks(hash: &H256, client: Arc<BlockChainClient>, storage: Arc<SyncStorage>) {
    let mut blocks_to_import = Vec::new();
    let mut staged_blocks = storage.staged_blocks().lock();
    if staged_blocks.contains_key(&hash) {
        if let Some(blocks_staged) = staged_blocks.remove(hash) {
            blocks_to_import.extend(blocks_staged);
        }
    }
    drop(staged_blocks);

    if blocks_to_import.len() <= 0 {
        return;
    }

    info!(target: "sync", "Importing {} staged blocks...", blocks_to_import.len());

    for block in &blocks_to_import {
        let block_number = BlockView::new(block).header_view().number();

        match client.import_block(block.clone()) {
            Ok(_)
            | Err(BlockImportError::Import(ImportError::AlreadyInChain))
            | Err(BlockImportError::Import(ImportError::AlreadyQueued)) => {
                trace!(target: "sync", "Staged block #{} imported...", block_number);
            }
            Err(e) => {
                warn!(target: "sync", "Failed to import staged block #{}, due to {:?}", block_number, e);
                // Remove records so that they can be downloaded again.
                storage.remove_downloaded_blocks_hashes(
                    &blocks_to_import
                        .iter()
                        .map(|block| BlockView::new(block).header_view().hash())
                        .collect(),
                );
                break;
            }
        }
    }
}

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
                break;
            } else {
                trace!(target: "sync", "ignore block #{}({}): {:?} ,from node {} ", block.header.number(), block.header.hash() , status, blocks_wrapper.node_hash);
            }
        }

        let mut empty_batch = false;
        // Mark if no block to import
        if blocks_to_import.is_empty() {
            empty_batch = true;
        }

        // Import blocks
        let mut first_imported_number = 0u64;
        let mut last_imported_number = 0u64;
        let mut unknown_number = 0u64;
        let mut unknown_parent_hash = H256::new();

        for block in &blocks_to_import {
            let block_view = BlockView::new(&block);
            let block_number = block_view.header_view().number();

            // Import block and check results
            let result = client.import_block(block.to_vec());
            match result {
                Ok(_)
                | Err(BlockImportError::Import(ImportError::AlreadyInChain))
                | Err(BlockImportError::Import(ImportError::AlreadyQueued)) => {
                    if first_imported_number == 0u64 {
                        first_imported_number = block_number;
                    }
                    last_imported_number = block_number;
                }
                Err(BlockImportError::Block(BlockError::UnknownParent(_))) => {
                    if first_imported_number != 0u64 {
                        error!(target: "sync", "Can't import inconsistent blocks");
                    }
                    unknown_number = block_number;
                    unknown_parent_hash = block_view.header_view().parent_hash();
                    break;
                }
                Err(_e) => {
                    // TODO add repeat threshold
                    break;
                }
            }
        }

        // Update mode
        let node_hash = blocks_wrapper.node_hash;
        let nodes_info = nodes_info.read();
        // Get network syncing modes (other than the current node)
        let normal_nodes = count_nodes_with_mode(&nodes_info, &node_hash, &Mode::Normal);
        let thunder_nodes = count_nodes_with_mode(&nodes_info, &node_hash, &Mode::Thunder);
        let lightning_nodes = count_nodes_with_mode(&nodes_info, &node_hash, &Mode::Lightning);
        if let Some(node_info) = nodes_info.get(&node_hash) {
            // Get the write lock here. The operations should be kept atomic
            let mut info = node_info.write();
            let node_best_number = info.best_block_number;
            let local_best_block = client.chain_info().best_block_number;

            // When get empty batch in backward or forward mode, switch to normal.
            if empty_batch {
                if info.mode == Mode::Backward || info.mode == Mode::Forward {
                    info.switch_mode(Mode::Normal, &local_best_block, &node_hash);
                }
                continue;
            }

            // Handle unknown-parent blocks, based on syncing mode.
            // We need to stage them in Lightning mode, or
            // in other cases, discard them and switch to backward mode
            if unknown_number != 0u64 {
                let unknown_blocks = blocks_to_import.clone();
                let mut unknown_blocks_hashes = Vec::new();
                for block in &unknown_blocks {
                    unknown_blocks_hashes.push(BlockView::new(&block).header_view().hash());
                }
                // In lightning mode, unknown parent blocks are far-away blocks that need to be staged and imported later
                if info.mode == Mode::Lightning {
                    // Try to stage blocks if not staged yet
                    if storage.stage_blocks(unknown_parent_hash, unknown_blocks) {
                        info!(target: "sync", "Node: {}, {} blocks staged for future import.", &node_hash, blocks_to_import.len());
                        // Get last block number
                        let last_block = blocks_to_import.last().expect(
                            "checked collection is not empty. Should be able to get the last",
                        );
                        let next_block_number =
                            BlockView::new(&last_block).header_view().number() + 1;
                        // Set next step
                        if next_block_number > storage.lightning_base() {
                            storage.set_lightning_base(next_block_number);
                        }
                    }
                    // If we cannot stage these blocks (staged blocks cache full), we need to remove downloaded records
                    // and try to download them again later.
                    else {
                        info.switch_mode(Mode::Thunder, &local_best_block, &node_hash);
                        storage.remove_downloaded_blocks_hashes(&unknown_blocks_hashes);
                    }
                } else {
                    // Remove hashes that are not imported due to unknown parent, so that they can be downloaded again.
                    storage.remove_downloaded_blocks_hashes(&unknown_blocks_hashes);
                    // If known parent blocks are fork blocks, we need to sync backward
                    if unknown_number <= local_best_block {
                        info.switch_mode(Mode::Backward, &local_best_block, &node_hash);
                        info.sync_base_number = unknown_number;
                    }
                }

                continue;
            }

            // Do nothing if no block is imported
            if first_imported_number == 0u64 || last_imported_number == 0u64 {
                continue;
            }

            match info.mode {
                // Fork point found, switch to forward mode
                Mode::Backward => {
                    info!(target: "sync", "Node: {}, found the fork point #{}", &node_hash, first_imported_number);
                    info.switch_mode(Mode::Forward, &local_best_block, &node_hash);
                    info.sync_base_number = last_imported_number + 1;
                }
                // Continue forward
                Mode::Forward => {
                    info.sync_base_number = last_imported_number + 1;
                }
                // Switch among NORMAL, THUNDER and LIGHTNING
                Mode::Normal | Mode::Thunder | Mode::Lightning => {
                    let mut mode = Mode::Normal;
                    // Must have at least 1 normal node
                    // If the target height is very close, sync in normal mode
                    if last_imported_number + 32 < node_best_number && normal_nodes > 0 {
                        // If the target height is far away and there are already enough normal and thunder nodes, jump to lightning
                        if last_imported_number + 500 < node_best_number
                            && thunder_nodes >= 1
                            && lightning_nodes < (normal_nodes + thunder_nodes) / 2
                        {
                            storage.set_lightning_base(last_imported_number + 1);
                            mode = Mode::Lightning;
                        } else {
                            mode = Mode::Thunder;
                        }
                    }
                    info.switch_mode(mode, &local_best_block, &node_hash);
                }
            }
            info!(target: "sync", "Node: {}, {} blocks imported", &node_hash, last_imported_number - first_imported_number + 1);
            trace!(target: "sync", "Node: {}, NORMAL: {}, THUNDER: {}, LIGHTNING: {}", &node_hash, normal_nodes, thunder_nodes, lightning_nodes);
        }
        drop(nodes_info);
        // TODO: maybe we should consider reset the header request cooldown here
    }
}

fn count_nodes_with_mode(
    nodes_info: &HashMap<u64, RwLock<NodeInfo>>,
    node_hash: &u64,
    mode: &Mode,
) -> u32
{
    nodes_info.iter().fold(0u32, |sum, node_info_lock| {
        // Skip the current node
        if node_info_lock.0 == node_hash {
            return sum;
        }
        let node_info = node_info_lock.1.read();
        if &node_info.mode == mode {
            sum + 1u32
        } else {
            sum
        }
    })
}
