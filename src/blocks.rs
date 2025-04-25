// BP Node: sovereign bitcoin wallet backend.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2025 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2025 LNP/BP Labs, InDCS, Switzerland. All rights reserved.
// Copyright (C) 2020-2025 Dr Maxim Orlovsky. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Block importer interface organized into a reactor thread.

use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use amplify::{ByteArray, FromSliceError};
use bprpc::BloomFilter32;
use bpwallet::{Block, BlockHash};
use crossbeam_channel::{RecvError, SendError, Sender};
use microservices::USender;
use redb::{
    CommitError, ReadableTable, ReadableTableMetadata, StorageError, TableError, WriteTransaction,
};

use crate::ImporterMsg;
use crate::db::{
    BlockId, DbBlock, DbBlockHeader, DbMsg, DbTx, ForkId, REC_BLOCKID, REC_FORK_ID, REC_TXNO,
    TABLE_BLKS, TABLE_BLOCK_HEIGHTS, TABLE_BLOCK_SPENDS, TABLE_BLOCK_TXS, TABLE_BLOCKIDS,
    TABLE_FORK_BLOCKS, TABLE_FORK_TIPS, TABLE_FORKS, TABLE_HEIGHTS, TABLE_INPUTS, TABLE_MAIN,
    TABLE_ORPHAN_PARENTS, TABLE_ORPHANS, TABLE_OUTS, TABLE_SPKS, TABLE_TX_BLOCKS, TABLE_TXES,
    TABLE_TXIDS, TABLE_UTXOS, TxNo,
};

const NAME: &str = "blockproc";

// TODO: Make this configuration options
// Constants for orphan block management
const MAX_ORPHAN_BLOCKS: usize = 100;
// Orphan blocks expire after 24 hours
const ORPHAN_EXPIRY_HOURS: u64 = 24;

pub struct BlockProcessor {
    db: USender<DbMsg>,
    broker: Sender<ImporterMsg>,
    tracking: HashSet<BloomFilter32>,
}

impl BlockProcessor {
    pub fn new(db: USender<DbMsg>, broker: Sender<ImporterMsg>) -> Self {
        Self { db, tracking: none!(), broker }
    }

    pub fn track(&mut self, filters: Vec<BloomFilter32>) { self.tracking.extend(filters); }

    pub fn untrack(&mut self, filters: Vec<BloomFilter32>) {
        self.tracking.retain(|filter| !filters.contains(filter));
    }

    // Helper function to calculate block height based on previous block hash
    fn calculate_block_height(
        &self,
        block: &Block,
        db: &WriteTransaction,
    ) -> Result<u32, BlockProcError> {
        // For genesis block, height is always 0
        // Check for all zeros hash which is the genesis block's prev_hash
        let zero_hash = [0u8; 32];
        if block.header.prev_block_hash.to_byte_array() == zero_hash {
            return Ok(0);
        }

        // Find block height of the previous block and add 1
        // Lookup the block ID for the previous block hash
        let blockids_table = db
            .open_table(TABLE_BLOCKIDS)
            .map_err(|e| BlockProcError::Custom(format!("Block IDs table error: {}", e)))?;

        let prev_blockid = blockids_table
            .get(block.header.prev_block_hash.to_byte_array())
            .map_err(|e| BlockProcError::Custom(format!("Block ID lookup error: {}", e)))?;

        // If previous block not found, it's an orphan block
        if prev_blockid.is_none() {
            log::debug!(
                target: NAME,
                "Orphan block detected: parent block {} not found",
                block.header.prev_block_hash
            );
            return Err(BlockProcError::OrphanBlock(block.header.prev_block_hash));
        }

        // Get the previous block's ID
        let prev_blockid = prev_blockid.unwrap().value();

        // First check the BlockId to height mapping table which is more efficient
        let block_heights_table = db
            .open_table(TABLE_BLOCK_HEIGHTS)
            .map_err(|e| BlockProcError::Custom(format!("Block heights table error: {}", e)))?;

        let height = block_heights_table
            .get(prev_blockid)
            .map_err(|e| BlockProcError::Custom(format!("Block height lookup error: {}", e)))?
            .map(|v| {
                let prev_height = v.value();
                prev_height + 1
            })
            .ok_or_else(|| {
                // Parent block has blockid but no height record - this indicates a potential fork
                // This typically happens when the parent block is part of a fork chain
                let block_hash = block.block_hash();
                let parent_hash = block.header.prev_block_hash;
                // Check if parent block is part of a known fork
                if let Some(fork_id) = match self.find_fork_by_block_id(db, prev_blockid){
                    Ok(Some(id)) => Some(id),
                    Ok(None) => None,
                    Err(e) => return e,
                } {
                    // Found the fork - this block extends a fork chain
                    log::info!(
                        target: NAME,
                        "Block {} has parent {} which is part of fork {}",
                        block_hash,
                        parent_hash,
                        fork_id
                    );
                    // Return specialized error for fork chain extension
                    return BlockProcError::ForkChainExtension(block_hash, parent_hash);
                }
                // If not part of a fork, it's likely a database inconsistency
                log::warn!(
                    target: NAME,
                    "Database inconsistency: Block {} has parent {} with ID {} but no height record",
                    block_hash,
                    parent_hash,
                    prev_blockid
                );
                BlockProcError::DatabaseInconsistency(block_hash, parent_hash, prev_blockid)
            })?;
        let heights_table = db
            .open_table(TABLE_HEIGHTS)
            .map_err(|e| BlockProcError::Custom(format!("Heights table error: {}", e)))?;

        // Check if we already have a block at this height
        if let Some(existing_blockid) = heights_table
            .get(height)
            .map_err(|e| BlockProcError::Custom(format!("Heights lookup error: {}", e)))?
            .map(|v| v.value())
        {
            log::warn!(
                target: NAME,
                "Detected potential chain fork at height {}: existing block ID {}",
                height,
                existing_blockid,
            );

            return Err(BlockProcError::PotentialFork(
                block.block_hash(),
                height,
                existing_blockid,
            ));
        }
        Ok(height)
    }

    /// Check if the block hash already exists in the database
    fn is_block_exists(
        &self,
        db: &WriteTransaction,
        block_hash: &BlockHash,
    ) -> Result<bool, BlockProcError> {
        let blockids_table = db
            .open_table(TABLE_BLOCKIDS)
            .map_err(|e| BlockProcError::Custom(format!("Block IDs table error: {}", e)))?;

        let exists = blockids_table
            .get(block_hash.to_byte_array())
            .map_err(|e| BlockProcError::Custom(format!("Block hash lookup error: {}", e)))?
            .is_some();

        Ok(exists)
    }

    pub fn process_block(&mut self, id: BlockHash, block: Block) -> Result<usize, BlockProcError> {
        // Store a copy of the parent hash for potential orphan block handling
        let parent_hash = block.header.prev_block_hash;
        // Clone the block for potential orphan processing
        let block_clone = block.clone();

        // Regular block processing starts here
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.db.send(DbMsg::Write(tx))?;
        let db = rx.recv()?;

        // Check if the block already exists
        if self.is_block_exists(&db, &id)? {
            log::warn!(
                target: NAME,
                "Block {} already exists in database, skipping processing",
                id
            );
            return Err(BlockProcError::Custom(format!("Block {} already exists", id)));
        }

        // Get current transaction number
        let mut txno_counter = {
            let main = db
                .open_table(TABLE_MAIN)
                .map_err(BlockProcError::MainTable)?;

            // Get current transaction number or use starting value if not found
            match main.get(REC_TXNO).map_err(BlockProcError::TxNoAbsent)? {
                Some(rec) => TxNo::from_slice(rec.value()).map_err(BlockProcError::TxNoInvalid)?,
                None => {
                    log::debug!(target: NAME, "No transaction counter found, starting from zero");
                    TxNo::start()
                }
            }
        };

        let mut count = 0;
        let process = || -> Result<(), BlockProcError> {
            // Calculate the block height based on previous block
            // This function will also detect orphan blocks and potential forks
            let height = self.calculate_block_height(&block, &db)?;

            let blockid = self.get_next_block_id(&db)?;

            // Open tables needed in the loop to avoid repeated opening/closing which affects
            // performance
            let mut blocks_table = db
                .open_table(TABLE_BLKS)
                .map_err(BlockProcError::BlockTable)?;

            let mut blockids_table = db
                .open_table(TABLE_BLOCKIDS)
                .map_err(|e| BlockProcError::Custom(format!("Block IDs table error: {}", e)))?;

            let mut heights_table = db
                .open_table(TABLE_HEIGHTS)
                .map_err(BlockProcError::HeightsTable)?;

            let mut block_heights_table = db
                .open_table(TABLE_BLOCK_HEIGHTS)
                .map_err(|e| BlockProcError::Custom(format!("Block heights table error: {}", e)))?;

            let mut txids_table = db
                .open_table(TABLE_TXIDS)
                .map_err(BlockProcError::TxidTable)?;

            let mut tx_blocks_table = db
                .open_table(TABLE_TX_BLOCKS)
                .map_err(|e| BlockProcError::Custom(format!("Tx-blocks table error: {}", e)))?;

            let mut utxos_table = db
                .open_table(TABLE_UTXOS)
                .map_err(|e| BlockProcError::Custom(format!("UTXOs table error: {}", e)))?;

            let mut inputs_table = db
                .open_table(TABLE_INPUTS)
                .map_err(|e| BlockProcError::Custom(format!("Inputs table error: {}", e)))?;

            let mut outs_table = db
                .open_table(TABLE_OUTS)
                .map_err(|e| BlockProcError::Custom(format!("Outs table error: {}", e)))?;

            let mut spks_table = db
                .open_table(TABLE_SPKS)
                .map_err(|e| BlockProcError::Custom(format!("SPKs table error: {}", e)))?;

            let mut txes_table = db
                .open_table(TABLE_TXES)
                .map_err(BlockProcError::TxesTable)?;

            let mut block_txs_table = db
                .open_table(TABLE_BLOCK_TXS)
                .map_err(|e| BlockProcError::Custom(format!("Block-txs table error: {}", e)))?;

            let mut block_spends_table = db
                .open_table(TABLE_BLOCK_SPENDS)
                .map_err(|e| BlockProcError::Custom(format!("Block spends table error: {}", e)))?;

            // Store block header
            blocks_table
                .insert(blockid, DbBlockHeader::from(block.header))
                .map_err(BlockProcError::BlockStorage)?;

            // Map block hash to block ID
            blockids_table
                .insert(id.to_byte_array(), blockid)
                .map_err(|e| BlockProcError::Custom(format!("Block ID storage error: {}", e)))?;

            log::debug!(
                target: NAME,
                "Processing block {} at height {} with internal ID {}",
                id,
                height,
                blockid
            );

            heights_table
                .insert(height, blockid)
                .map_err(|e| BlockProcError::Custom(format!("Heights storage error: {}", e)))?;

            // Also update the reverse mapping (blockid -> height)
            block_heights_table.insert(blockid, height).map_err(|e| {
                BlockProcError::Custom(format!("Block height storage error: {}", e))
            })?;

            // Track UTXOs spent in this block
            let mut block_spends = Vec::new();

            // Track all transactions in this block
            let mut block_txs = Vec::new();

            // Process transactions in the block
            for tx in block.transactions {
                // Get txno from TABLE_TXIDS using txid. If it doesn't exist, use txno-counter,
                // otherwise use the existing txno. This is mainly to avoid issues after block
                // reorganization, where the same txid in different blocks could be
                // assigned different txno values, leading to incorrect processing
                let txid = tx.txid();
                let txno = txids_table
                    .get(txid.to_byte_array())
                    .map_err(BlockProcError::TxidLookup)?
                    .map(|v| v.value())
                    .unwrap_or_else(|| {
                        txno_counter.inc_assign();
                        txno_counter
                    });

                // Add transaction to the list for this block
                block_txs.push(txno);

                txids_table
                    .insert(txid.to_byte_array(), txno)
                    .map_err(BlockProcError::TxidStorage)?;

                // Associate transaction with block ID
                tx_blocks_table.insert(txno, blockid).map_err(|e| {
                    BlockProcError::Custom(format!("Tx-blocks storage error: {}", e))
                })?;

                // Process transaction inputs
                for (vin_idx, input) in tx.inputs.iter().enumerate() {
                    if !input.prev_output.is_coinbase() {
                        let prev_txid = input.prev_output.txid;
                        let prev_vout = input.prev_output.vout;

                        // Look up previous transaction number
                        if let Some(prev_txno) = txids_table
                            .get(prev_txid.to_byte_array())
                            .map_err(BlockProcError::TxidLookup)?
                            .map(|v| v.value())
                        {
                            // Mark UTXO as spent
                            utxos_table
                                .remove(&(prev_txno, prev_vout.into_u32()))
                                .map_err(|e| {
                                    BlockProcError::Custom(format!("UTXOs removal error: {}", e))
                                })?;

                            // Record UTXO spent in this block
                            block_spends.push((prev_txno, prev_vout.into_u32()));

                            // Record input-output mapping
                            inputs_table
                                .insert((txno, vin_idx as u32), (prev_txno, prev_vout.into_u32()))
                                .map_err(|e| {
                                    BlockProcError::Custom(format!("Inputs storage error: {}", e))
                                })?;

                            // Update spending relationships
                            let mut spending_txs = outs_table
                                .get(prev_txno)
                                .map_err(|e| {
                                    BlockProcError::Custom(format!("Outs lookup error: {}", e))
                                })?
                                .map(|v| v.value().to_vec())
                                .unwrap_or_default();
                            spending_txs.push(txno);
                            outs_table.insert(prev_txno, spending_txs).map_err(|e| {
                                BlockProcError::Custom(format!("Outs update error: {}", e))
                            })?;
                        }
                    }
                }

                // Process transaction outputs
                for (vout_idx, output) in tx.outputs.iter().enumerate() {
                    // Add new UTXO
                    utxos_table
                        .insert((txno, vout_idx as u32), ())
                        .map_err(|e| {
                            BlockProcError::Custom(format!("UTXOs storage error: {}", e))
                        })?;

                    // Index script pubkey
                    let script = &output.script_pubkey;
                    if !script.is_empty() {
                        let mut txnos = spks_table
                            .get(script.as_slice())
                            .map_err(|e| {
                                BlockProcError::Custom(format!("SPKs lookup error: {}", e))
                            })?
                            .map(|v| v.value().to_vec())
                            .unwrap_or_default();
                        txnos.push(txno);
                        spks_table.insert(script.as_slice(), txnos).map_err(|e| {
                            BlockProcError::Custom(format!("SPKs update error: {}", e))
                        })?;
                    }
                }

                // Store complete transaction
                txes_table
                    .insert(txno, DbTx::from(tx))
                    .map_err(BlockProcError::TxesStorage)?;

                // Check if transaction ID is in tracking list and notify if needed
                let txid_bytes = txid.to_byte_array();
                let mut should_notify = false;
                for filter in &self.tracking {
                    if filter.contains(txid_bytes) {
                        should_notify = true;
                        break;
                    }
                }
                if should_notify {
                    self.broker.send(ImporterMsg::Mined(txid))?;
                }

                count += 1;
            }

            // Store all transaction numbers in this block
            block_txs_table
                .insert(blockid, block_txs)
                .map_err(|e| BlockProcError::Custom(format!("Block-txs storage error: {}", e)))?;

            // Store UTXOs spent in this block
            block_spends_table
                .insert(blockid, block_spends)
                .map_err(|e| {
                    BlockProcError::Custom(format!("Block spends storage error: {}", e))
                })?;

            // Update global counters
            let mut main = db
                .open_table(TABLE_MAIN)
                .map_err(BlockProcError::MainTable)?;

            // Update transaction counter
            main.insert(REC_TXNO, txno_counter.to_byte_array().as_slice())
                .map_err(BlockProcError::TxNoUpdate)?;

            // Log successful block processing
            log::debug!(
                target: NAME,
                "Successfully processed block {} at height {} with {} transactions",
                id,
                height,
                count
            );

            Ok(())
        };

        match process() {
            Ok(()) => {
                db.commit()?;

                log::debug!(
                    target: NAME,
                    "Successfully processed block {} with {} transactions",
                    id,
                    count
                );

                Ok(count)
            }
            Err(BlockProcError::OrphanBlock(e)) => {
                // Handle orphan block case
                if let Err(err) = db.abort() {
                    log::warn!(target: NAME, "Unable to abort failed database transaction due to {err}");
                };

                // Save the orphan block for later processing
                log::info!(
                    target: NAME,
                    "Orphan block detected: Parent block {} not found for block {}",
                    parent_hash,
                    id
                );

                self.save_orphan_block(id, block_clone)?;
                Err(BlockProcError::OrphanBlock(e))
            }
            Err(BlockProcError::PotentialFork(new_block_hash, height, existing_blockid)) => {
                // Handle potential fork case - conflict with existing block at same height
                if let Err(err) = db.abort() {
                    log::warn!(target: NAME, "Unable to abort failed database transaction due to {err}");
                };

                // Record this as a potential fork for later verification
                // Store the new block but don't update the height tables yet
                // We'll only perform a reorganization if this fork becomes the longest chain
                let result = self.process_potential_fork(
                    id,
                    &block_clone,
                    Some(height),
                    Some(existing_blockid),
                )?;

                debug_assert!(result.is_none());

                Err(BlockProcError::PotentialFork(new_block_hash, height, existing_blockid))
            }
            Err(BlockProcError::ForkChainExtension(block_hash, parent_hash)) => {
                // Handle fork chain extension case - parent block is part of a fork
                if let Err(err) = db.abort() {
                    log::warn!(target: NAME, "Unable to abort failed database transaction due to {err}");
                };

                log::info!(
                    target: NAME,
                    "Processing block {} as fork chain extension with parent {}",
                    block_hash,
                    parent_hash
                );

                // If a chain reorganization occurs, return the number of transactions added
                if let Some(txs_added) =
                    self.process_potential_fork(id, &block_clone, None, None)?
                {
                    return Ok(txs_added);
                }

                Err(BlockProcError::ForkChainExtension(block_hash, parent_hash))
            }
            Err(e) => {
                // Handle other errors
                if let Err(err) = db.abort() {
                    log::warn!(target: NAME, "Unable to abort failed database transaction due to {err}");
                };
                Err(e)
            }
        }
    }

    /// Process a block and all its dependent orphans in an iterative manner to avoid stack
    /// overflow.
    ///
    /// This method should be used instead of directly calling `process_block` when you want to
    /// ensure that orphan blocks dependent on the processed block are also handled.
    pub fn process_block_and_orphans(
        &mut self,
        id: BlockHash,
        block: Block,
    ) -> Result<usize, BlockProcError> {
        // Create a queue to store blocks that need to be processed
        // Store (block_hash, block, parent_hash) tuples
        let mut pending_blocks = std::collections::VecDeque::new();
        pending_blocks.push_back((id, block, None));

        let mut total_processed = 0;

        // Process blocks in a loop rather than recursive calls
        while let Some((current_id, current_block, parent_hash)) = pending_blocks.pop_front() {
            // Process the current block
            match self.process_block(current_id, current_block) {
                Ok(count) => {
                    total_processed += count;

                    // If this was an orphan block (has a parent_hash), remove it from the orphan
                    // pool
                    if let Some(parent) = parent_hash {
                        // Only remove this specific orphan after successful processing
                        if let Err(e) = self.remove_processed_orphans(parent, &[current_id]) {
                            log::warn!(
                                target: NAME,
                                "Failed to remove processed orphan {}: {}",
                                current_id,
                                e
                            );
                        } else {
                            log::info!(
                                target: NAME,
                                "Successfully removed processed orphan {} from pool",
                                current_id
                            );
                        }
                    }

                    // Find orphans that depend on this block
                    if let Ok(orphans) = self.find_dependent_orphans(current_id) {
                        // Skip if no orphans found
                        if !orphans.is_empty() {
                            // Add them to the queue for processing
                            for (orphan_id, orphan_block) in orphans {
                                log::info!(
                                    target: NAME,
                                    "Adding orphan block {} to processing queue",
                                    orphan_id
                                );

                                // Add to the queue for processing
                                // Include the parent hash so we can remove it from orphan pool
                                // after processing
                                pending_blocks.push_back((
                                    orphan_id,
                                    orphan_block,
                                    Some(current_id),
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    // For orphan blocks, we just continue with the next block
                    if let BlockProcError::OrphanBlock(_) = e {
                        log::debug!(
                            target: NAME,
                            "Orphan block {} will be processed later when its parent is available",
                            current_id
                        );
                        continue;
                    }

                    // For other errors, log and return the error
                    log::error!(
                        target: NAME,
                        "Error processing block {}: {}",
                        current_id,
                        e
                    );
                    return Err(e);
                }
            }
        }

        Ok(total_processed)
    }

    // Helper method to find orphans that depend on a specific block
    fn find_dependent_orphans(
        &self,
        parent_id: BlockHash,
    ) -> Result<Vec<(BlockHash, Block)>, BlockProcError> {
        // First check if we have any orphans that depend on this block
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.db.send(DbMsg::Read(tx))?;
        let db = rx.recv()?;

        // Check orphan parents table
        let orphan_parents_table = db
            .open_table(TABLE_ORPHAN_PARENTS)
            .map_err(|e| BlockProcError::Custom(format!("Orphan parents table error: {}", e)))?;

        let parent_hash = parent_id.to_byte_array();
        let orphans = orphan_parents_table
            .get(parent_hash)
            .map_err(|e| BlockProcError::Custom(format!("Orphan parents lookup error: {}", e)))?;

        // If no orphans depend on this block, return empty list
        if orphans.is_none() {
            return Ok(Vec::new());
        }

        // Get list of orphan block hashes
        let orphan_hashes = orphans.unwrap().value();

        // Get orphans data
        let orphans_table = db
            .open_table(TABLE_ORPHANS)
            .map_err(|e| BlockProcError::Custom(format!("Orphans table error: {}", e)))?;

        let mut dependent_orphans = Vec::with_capacity(orphan_hashes.len());

        for orphan_hash in &orphan_hashes {
            // Get the orphan block data
            if let Some(orphan_block_data) = orphans_table
                .get(orphan_hash)
                .map_err(|e| BlockProcError::Custom(format!("Orphan lookup error: {}", e)))?
            {
                let (block_data, _timestamp) = orphan_block_data.value();

                // Extract the Block object and create a BlockHash
                let block = Block::from(block_data);
                let block_hash = BlockHash::from_byte_array(*orphan_hash);
                debug_assert_eq!(block.block_hash(), block_hash);

                dependent_orphans.push((block_hash, block));

                log::info!(
                    target: NAME,
                    "Found orphan block {} with parent {}",
                    block_hash,
                    parent_id
                );
            }
        }

        // We don't remove orphans here - they'll be removed after successful processing
        if !dependent_orphans.is_empty() {
            log::info!(
                target: NAME,
                "Found {} orphan blocks dependent on block {}",
                dependent_orphans.len(),
                parent_id
            );
        }

        Ok(dependent_orphans)
    }

    // Modified to remove orphans after they've been processed
    fn remove_processed_orphans(
        &mut self,
        parent_id: BlockHash,
        processed: &[BlockHash],
    ) -> Result<(), BlockProcError> {
        if processed.is_empty() {
            return Ok(());
        }

        let (tx, rx) = crossbeam_channel::bounded(1);
        self.db.send(DbMsg::Write(tx))?;
        let write_db = rx.recv()?;

        let remove_orphans = || -> Result<(), BlockProcError> {
            // Remove from orphan parents table
            let mut orphan_parents_table =
                write_db.open_table(TABLE_ORPHAN_PARENTS).map_err(|e| {
                    BlockProcError::Custom(format!("Orphan parents table error: {}", e))
                })?;

            // Get the current list of orphans for this parent
            let parent_hash = parent_id.to_byte_array();

            // Get orphan list and immediately convert to Vec to drop the borrow
            let orphan_hashes = {
                let orphans = orphan_parents_table.get(parent_hash).map_err(|e| {
                    BlockProcError::Custom(format!("Orphan parents lookup error: {}", e))
                })?;

                if let Some(orphans_record) = orphans {
                    orphans_record.value().to_vec()
                } else {
                    // No orphans found for this parent, nothing to do
                    return Ok(());
                }
            };

            // Filter out processed orphans
            let remaining_orphans: Vec<[u8; 32]> = orphan_hashes
                .into_iter()
                .filter(|h| !processed.iter().any(|p| p.to_byte_array() == *h))
                .collect();

            // Update or remove the entry
            if remaining_orphans.is_empty() {
                orphan_parents_table.remove(parent_hash).map_err(|e| {
                    BlockProcError::Custom(format!("Orphan parents removal error: {}", e))
                })?;

                log::debug!(
                    target: NAME,
                    "Removed all orphans for parent block {}",
                    parent_id
                );
            } else {
                orphan_parents_table
                    .insert(parent_hash, remaining_orphans)
                    .map_err(|e| BlockProcError::Custom(format!("Parent update error: {}", e)))?;

                log::debug!(
                    target: NAME,
                    "Updated orphan list for parent block {}",
                    parent_id
                );
            }

            // Remove from orphans table
            let mut orphans_table = write_db
                .open_table(TABLE_ORPHANS)
                .map_err(|e| BlockProcError::Custom(format!("Orphans table error: {}", e)))?;

            for orphan_hash in processed {
                orphans_table
                    .remove(orphan_hash.to_byte_array())
                    .map_err(|e| BlockProcError::Custom(format!("Orphan removal error: {}", e)))?;

                log::debug!(
                    target: NAME,
                    "Removed orphan block {} from orphans table",
                    orphan_hash
                );
            }

            Ok(())
        };

        if let Err(e) = remove_orphans() {
            if let Err(err) = write_db.abort() {
                log::warn!(
                    target: NAME,
                    "Unable to abort failed orphan cleanup transaction due to {err}"
                );
            }
            return Err(e);
        }

        write_db.commit()?;

        log::info!(
            target: NAME,
            "Successfully removed {} processed orphan blocks",
            processed.len()
        );

        Ok(())
    }

    // Save an orphan block for later processing
    fn save_orphan_block(&self, id: BlockHash, block: Block) -> Result<usize, BlockProcError> {
        log::info!(
            target: NAME,
            "Saving orphan block {} with parent {} for later processing",
            id,
            block.header.prev_block_hash
        );

        // First, check if we should clean up old orphans
        self.clean_expired_orphans()?;

        // Then check if we have too many orphans
        if self.count_orphans()? >= MAX_ORPHAN_BLOCKS {
            log::warn!(
                target: NAME,
                "Orphan block limit reached ({}). Rejecting new orphan block {}",
                MAX_ORPHAN_BLOCKS,
                id
            );
            // Simply ignore this orphan block
            return Ok(0);
        }

        let (tx, rx) = crossbeam_channel::bounded(1);
        self.db.send(DbMsg::Write(tx))?;
        let db = rx.recv()?;

        let process = || -> Result<(), BlockProcError> {
            // Get the current timestamp for expiry tracking
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs();

            let parent_hash = block.header.prev_block_hash.to_byte_array();

            // Store the orphan block
            let mut orphans_table = db
                .open_table(TABLE_ORPHANS)
                .map_err(|e| BlockProcError::Custom(format!("Orphans table error: {}", e)))?;

            orphans_table
                .insert(id.to_byte_array(), (DbBlock::from(block), now))
                .map_err(|e| BlockProcError::Custom(format!("Orphan storage error: {}", e)))?;

            // Index by parent hash to allow quick lookup when parent is processed
            let mut orphan_parents_table = db.open_table(TABLE_ORPHAN_PARENTS).map_err(|e| {
                BlockProcError::Custom(format!("Orphan parents table error: {}", e))
            })?;

            // Get existing orphans with the same parent, if any
            let mut orphan_list = orphan_parents_table
                .get(parent_hash)
                .map_err(|e| BlockProcError::Custom(format!("Orphan parents lookup error: {}", e)))?
                .map(|v| v.value().to_vec())
                .unwrap_or_default();

            // Add this orphan to the list
            orphan_list.push(id.to_byte_array());

            // Update the orphan parents table
            orphan_parents_table
                .insert(parent_hash, orphan_list)
                .map_err(|e| {
                    BlockProcError::Custom(format!("Orphan parents update error: {}", e))
                })?;

            Ok(())
        };

        if let Err(e) = process() {
            if let Err(err) = db.abort() {
                log::warn!(
                    target: NAME,
                    "Unable to abort failed orphan block storage transaction due to {err}"
                );
            };
            return Err(e);
        }

        db.commit()?;

        log::info!(
            target: NAME,
            "Successfully saved orphan block {} for later processing",
            id
        );

        // Return 0 since we didn't process any transactions yet
        Ok(0)
    }

    // Count total number of orphan blocks
    fn count_orphans(&self) -> Result<usize, BlockProcError> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.db.send(DbMsg::Read(tx))?;
        let db = rx.recv()?;

        let orphans_table = db
            .open_table(TABLE_ORPHANS)
            .map_err(|e| BlockProcError::Custom(format!("Orphans table error: {}", e)))?;

        let count: usize = orphans_table
            .len()
            .map_err(|e| BlockProcError::Custom(format!("Failed to count orphans: {}", e)))?
            as usize;

        Ok(count)
    }

    // Remove orphan blocks that have been in the pool for too long
    fn clean_expired_orphans(&self) -> Result<(), BlockProcError> {
        log::debug!(target: NAME, "Checking for expired orphan blocks...");

        let (tx, rx) = crossbeam_channel::bounded(1);
        self.db.send(DbMsg::Read(tx))?;
        let db = rx.recv()?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        // Calculate expiry threshold
        let expiry_secs = ORPHAN_EXPIRY_HOURS * 3600;
        let expiry_threshold = now.saturating_sub(expiry_secs);

        // Find expired orphans
        let orphans_table = db
            .open_table(TABLE_ORPHANS)
            .map_err(|e| BlockProcError::Custom(format!("Orphans table error: {}", e)))?;

        let mut expired_orphans = Vec::new();

        // Scan all orphans
        let orphans_iter = orphans_table.iter().map_err(|e| {
            BlockProcError::Custom(format!("Failed to iterate orphans table: {}", e))
        })?;

        for orphan_entry in orphans_iter {
            let (orphan_hash, data) = orphan_entry.map_err(|e| {
                BlockProcError::Custom(format!("Failed to read orphan entry: {}", e))
            })?;

            let (_block_data, timestamp) = data.value();

            // Check if orphan has expired
            if timestamp < expiry_threshold {
                expired_orphans.push(orphan_hash.value());
            }
        }

        if !expired_orphans.is_empty() {
            log::info!(
                target: NAME,
                "Found {} expired orphan blocks to clean up",
                expired_orphans.len()
            );

            let (write_tx, write_rx) = crossbeam_channel::bounded(1);
            self.db.send(DbMsg::Write(write_tx))?;
            let write_db = write_rx.recv()?;

            let remove_expired = || -> Result<(), BlockProcError> {
                let mut orphans_table = write_db
                    .open_table(TABLE_ORPHANS)
                    .map_err(|e| BlockProcError::Custom(format!("Orphans table error: {}", e)))?;

                let mut orphan_parents_table =
                    write_db.open_table(TABLE_ORPHAN_PARENTS).map_err(|e| {
                        BlockProcError::Custom(format!("Orphan parents table error: {}", e))
                    })?;

                for orphan_hash in &expired_orphans {
                    // Remove from orphans table
                    orphans_table.remove(orphan_hash).map_err(|e| {
                        BlockProcError::Custom(format!("Orphan removal error: {}", e))
                    })?;

                    // Also need to remove from parent mappings
                    // This is more complex as we need to scan all parent entries
                    let parents_iter = orphan_parents_table.iter().map_err(|e| {
                        BlockProcError::Custom(format!("Failed to iterate orphan parents: {}", e))
                    })?;

                    // First collect all parents to scan
                    let mut parents_to_scan = Vec::new();

                    for parent_entry in parents_iter {
                        let (parent_hash, orphans) = parent_entry.map_err(|e| {
                            BlockProcError::Custom(format!("Failed to read parent entry: {}", e))
                        })?;

                        // Store parent data for later processing
                        parents_to_scan.push((parent_hash.value(), orphans.value().to_vec()));
                    }

                    // Now process parents without borrowing the table
                    for (parent_hash, orphans_list) in parents_to_scan {
                        // We need to iterate each orphan hash in the list and check if it
                        // matches our target
                        let mut found = false;
                        for list_hash in &orphans_list {
                            // Convert both to slices for comparison
                            if list_hash == orphan_hash {
                                found = true;
                                break;
                            }
                        }

                        if found {
                            // Remove this orphan from the list
                            let updated_list: Vec<[u8; 32]> = orphans_list
                                .into_iter()
                                .filter(|h| h != orphan_hash)
                                .collect();

                            if updated_list.is_empty() {
                                // If no orphans left for this parent, remove the entry
                                orphan_parents_table.remove(parent_hash).map_err(|e| {
                                    BlockProcError::Custom(format!("Parent removal error: {}", e))
                                })?;
                            } else {
                                // Otherwise update with the filtered list
                                orphan_parents_table
                                    .insert(parent_hash, updated_list)
                                    .map_err(|e| {
                                        BlockProcError::Custom(format!(
                                            "Parent update error: {}",
                                            e
                                        ))
                                    })?;
                            }
                        }
                    }
                }

                Ok(())
            };

            if let Err(e) = remove_expired() {
                if let Err(err) = write_db.abort() {
                    log::warn!(
                        target: NAME,
                        "Unable to abort failed orphan cleanup transaction due to {err}"
                    );
                }
                return Err(e);
            }

            write_db.commit()?;

            log::info!(
                target: NAME,
                "Successfully removed {} expired orphan blocks",
                expired_orphans.len()
            );
        }

        Ok(())
    }

    /// Process a block that might create a fork in the blockchain.
    /// This method records fork information and checks if we need to perform a chain
    /// reorganization.
    fn process_potential_fork(
        &mut self,
        block_hash: BlockHash,
        block: &Block,
        height: Option<u32>,
        existing_blockid: Option<BlockId>,
    ) -> Result<Option<usize>, BlockProcError> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.db.send(DbMsg::Write(tx))?;
        let db = rx.recv()?;

        // Get a new block ID for this fork block
        let new_blockid = self.get_next_block_id(&db)?;

        {
            // Store the block header
            let mut blocks_table = db
                .open_table(TABLE_BLKS)
                .map_err(BlockProcError::BlockTable)?;
            blocks_table
                .insert(new_blockid, DbBlockHeader::from(block.header))
                .map_err(BlockProcError::BlockStorage)?;

            // Store the complete block data in the fork blocks table
            let mut fork_blocks_table = db
                .open_table(TABLE_FORK_BLOCKS)
                .map_err(|e| BlockProcError::Custom(format!("Fork blocks table error: {}", e)))?;
            fork_blocks_table
                .insert(new_blockid, DbBlock::from(block.clone()))
                .map_err(|e| BlockProcError::Custom(format!("Fork block storage error: {}", e)))?;

            // Map block hash to the assigned block ID
            let mut blockids_table = db
                .open_table(TABLE_BLOCKIDS)
                .map_err(|e| BlockProcError::Custom(format!("Block IDs table error: {}", e)))?;
            blockids_table
                .insert(block_hash.to_byte_array(), new_blockid)
                .map_err(|e| BlockProcError::Custom(format!("Block ID storage error: {}", e)))?;
        }

        // First step: Check if this block extends an existing fork
        // Find the parent block ID
        let parent_block_id = self
            .find_block_id_by_hash(&db, block.header.prev_block_hash)?
            .ok_or(BlockProcError::Custom(format!(
                "Parent block not found: {}",
                block.header.prev_block_hash
            )))?;
        let fork_id = if let Some(parent_fork_id) =
            self.find_fork_by_block_id(&db, parent_block_id)?
        {
            // This block extends an existing fork
            log::info!(
                target: NAME,
                "Block {} extends existing fork {}",
                block_hash,
                parent_fork_id
            );

            // Update the fork with this new block
            self.update_fork(&db, parent_fork_id, new_blockid)?;

            parent_fork_id
        } else {
            // This block might start a new fork
            // First check if its parent is in the main chain
            if !self.is_block_in_main_chain(&db, block.header.prev_block_hash)? {
                // Parent block is not in main chain and not in a known fork
                log::warn!(
                    target: NAME,
                    "Block {} is disconnected: parent {} not found in main chain or forks",
                    block_hash,
                    block.header.prev_block_hash
                );
                return Ok(None);
            }

            self.record_fork(
                &db,
                height
                    .ok_or(BlockProcError::Custom("Height is required for new fork".to_string()))?,
                existing_blockid.ok_or(BlockProcError::Custom(
                    "Existing block ID is required for new fork".to_string(),
                ))?,
                new_blockid,
                block_hash,
            )?
        };

        // Check if this fork is now longer than the main chain
        let txs_added = self.check_fork_length(&db, fork_id)?;

        db.commit()?;

        Ok(txs_added)
    }

    /// Check if a fork is longer than the main chain and perform reorganization if needed
    fn check_fork_length(
        &mut self,
        db: &WriteTransaction,
        fork_id: ForkId,
    ) -> Result<Option<usize>, BlockProcError> {
        // Get fork information
        let (_fork_start_height, _fork_start_block_id, _fork_tip_id, fork_height) =
            self.get_fork_info(db, fork_id)?;

        // Get main chain height
        let main_chain_height = self.get_main_chain_height(db)?;

        // If fork is longer than main chain, perform reorganization
        if fork_height > main_chain_height {
            log::info!(
                target: NAME,
                "Fork {} is longer than main chain ({} > {}), initiating chain reorganization",
                fork_id,
                fork_height,
                main_chain_height
            );

            // Perform chain reorganization
            let txs_added = self.perform_chain_reorganization(db, fork_id)?;
            return Ok(Some(txs_added));
        } else {
            log::debug!(
                target: NAME,
                "Fork {} is not longer than main chain ({} <= {}), no reorganization needed",
                fork_id,
                fork_height,
                main_chain_height
            );
        }

        Ok(None)
    }

    /// Perform a chain reorganization to adopt a fork as the new main chain
    fn perform_chain_reorganization(
        &mut self,
        db: &WriteTransaction,
        fork_id: ForkId,
    ) -> Result<usize, BlockProcError> {
        // Get fork information
        let (fork_start_height, _fork_start_block_id, fork_tip_id, fork_height) =
            self.get_fork_info(db, fork_id)?;

        log::info!(
            target: NAME,
            "Starting chain reorganization: Fork {} from height {} to {} with tip block {}",
            fork_id,
            fork_start_height,
            fork_height,
            fork_tip_id
        );

        // 1. Find the common ancestor
        let common_ancestor_height = fork_start_height;

        // 2. Get blocks to rollback from main chain
        let main_chain_height = self.get_main_chain_height(db)?;
        let blocks_to_rollback =
            self.get_blocks_to_rollback(db, common_ancestor_height, main_chain_height)?;

        // 3. Get blocks to apply from fork chain
        let blocks_to_apply =
            self.get_blocks_to_apply(db, fork_id, common_ancestor_height, fork_height)?;

        log::info!(
            target: NAME,
            "Chain reorganization: rolling back {} blocks and applying {} blocks",
            blocks_to_rollback.len(),
            blocks_to_apply.len()
        );

        // 4. Roll back blocks from main chain
        self.rollback_blocks(db, &blocks_to_rollback)?;

        // 5. Apply blocks from fork chain
        let txs_added = self.apply_blocks(db, &blocks_to_apply)?;

        // 6. Update fork status
        self.cleanup_after_reorg(db, fork_id)?;

        log::info!(
            target: NAME,
            "Chain reorganization complete: new chain height is {}",
            fork_height
        );

        Ok(txs_added)
    }

    /// Records a potential fork in the blockchain.
    /// This happens when we discover two different blocks at the same height.
    fn record_fork(
        &self,
        db: &WriteTransaction,
        height: u32,
        existing_blockid: BlockId,
        new_blockid: BlockId,
        new_block_hash: BlockHash,
    ) -> Result<ForkId, BlockProcError> {
        // Check if this block is already part of a known fork
        if let Some(fork_id) = self.find_fork_by_block_id(db, new_blockid)? {
            log::debug!(
                target: NAME,
                "Block {} at height {} is already part of fork {}",
                new_block_hash,
                height,
                fork_id
            );
            return Ok(fork_id);
        }

        // Generate a new fork ID
        let fork_id = self.get_next_fork_id(db)?;

        // Record the fork in the forks table
        let mut forks_table = db
            .open_table(TABLE_FORKS)
            .map_err(|e| BlockProcError::Custom(format!("Forks table error: {}", e)))?;

        // A fork starts at the current height with the current block
        // Parameters: (fork_start_height, fork_start_block_id, tip_block_id, current_height)
        forks_table
            .insert(fork_id, (height, existing_blockid, new_blockid, height))
            .map_err(|e| BlockProcError::Custom(format!("Fork insertion error: {}", e)))?;

        // Map the fork tip block ID to the fork ID
        let mut fork_tips_table = db
            .open_table(TABLE_FORK_TIPS)
            .map_err(|e| BlockProcError::Custom(format!("Fork tips table error: {}", e)))?;

        fork_tips_table
            .insert(new_blockid, fork_id)
            .map_err(|e| BlockProcError::Custom(format!("Fork tip mapping error: {}", e)))?;

        log::info!(
            target: NAME,
            "Created new fork {} at height {}: Main chain block {} vs Fork block {}",
            fork_id,
            height,
            existing_blockid,
            new_blockid
        );

        Ok(fork_id)
    }

    /// Gets the next available block ID and increments the counter
    fn get_next_block_id(&self, db: &WriteTransaction) -> Result<BlockId, BlockProcError> {
        let mut main = db
            .open_table(TABLE_MAIN)
            .map_err(BlockProcError::MainTable)?;
        let mut block_id = match main
            .get(REC_BLOCKID)
            .map_err(|e| BlockProcError::Custom(format!("Block ID lookup error: {}", e)))?
        {
            Some(rec) => BlockId::from_bytes(rec.value()),
            None => BlockId::start(),
        };

        block_id.inc_assign();
        main.insert(REC_BLOCKID, block_id.to_bytes().as_slice())
            .map_err(|e| BlockProcError::Custom(format!("Block ID update error: {}", e)))?;

        Ok(block_id)
    }

    /// Gets the next available fork ID and increments the counter
    fn get_next_fork_id(&self, db: &WriteTransaction) -> Result<ForkId, BlockProcError> {
        let mut main = db
            .open_table(TABLE_MAIN)
            .map_err(BlockProcError::MainTable)?;

        let mut fork_id = {
            match main
                .get(REC_FORK_ID)
                .map_err(|e| BlockProcError::Custom(format!("Fork ID lookup error: {}", e)))?
            {
                Some(rec) => ForkId::from_bytes(rec.value()),
                None => ForkId::start(),
            }
        };
        fork_id.inc_assign();
        main.insert(REC_FORK_ID, fork_id.to_bytes().as_slice())
            .map_err(|e| BlockProcError::Custom(format!("Fork ID update error: {}", e)))?;

        Ok(fork_id)
    }

    /// Find block ID by block hash
    fn find_block_id_by_hash(
        &self,
        db: &WriteTransaction,
        block_hash: BlockHash,
    ) -> Result<Option<BlockId>, BlockProcError> {
        let blockids_table = db
            .open_table(TABLE_BLOCKIDS)
            .map_err(BlockProcError::BlockTable)?;
        let block_id = blockids_table
            .get(block_hash.to_byte_array())
            .map_err(|e| BlockProcError::Custom(format!("Block ID lookup error: {}", e)))?;
        if let Some(record) = block_id { Ok(Some(record.value())) } else { Ok(None) }
    }

    /// Find fork ID by block hash
    fn find_fork_by_block_id(
        &self,
        db: &WriteTransaction,
        block_id: BlockId,
    ) -> Result<Option<ForkId>, BlockProcError> {
        let fork_tips_table = db
            .open_table(TABLE_FORK_TIPS)
            .map_err(|e| BlockProcError::Custom(format!("Fork tips table error: {}", e)))?;

        if let Some(fork_id_record) = fork_tips_table
            .get(block_id)
            .map_err(|e| BlockProcError::Custom(format!("Fork tip lookup error: {}", e)))?
        {
            return Ok(Some(fork_id_record.value()));
        }

        Ok(None)
    }

    /// Update fork information with a new block
    fn update_fork(
        &self,
        db: &WriteTransaction,
        fork_id: ForkId,
        new_block_id: BlockId,
    ) -> Result<(), BlockProcError> {
        // Get current fork info
        let (start_height, start_block_id, old_tip_id, current_height) =
            self.get_fork_info(db, fork_id)?;
        let new_height = current_height + 1;

        // Update the fork record
        let mut forks_table = db
            .open_table(TABLE_FORKS)
            .map_err(|e| BlockProcError::Custom(format!("Forks table error: {}", e)))?;

        // Update fork with new tip and height
        forks_table
            .insert(fork_id, (start_height, start_block_id, new_block_id, new_height))
            .map_err(|e| BlockProcError::Custom(format!("Fork update error: {}", e)))?;

        // Update the fork tip mapping
        let mut fork_tips_table = db
            .open_table(TABLE_FORK_TIPS)
            .map_err(|e| BlockProcError::Custom(format!("Fork tips table error: {}", e)))?;

        // Remove old tip mapping if it exists
        fork_tips_table
            .remove(old_tip_id)
            .map_err(|e| BlockProcError::Custom(format!("Fork tip removal error: {}", e)))?;

        // Add new tip mapping
        fork_tips_table
            .insert(new_block_id, fork_id)
            .map_err(|e| BlockProcError::Custom(format!("Fork tip mapping error: {}", e)))?;

        log::debug!(
            target: NAME,
            "Updated fork {}: new height {}, new tip {}",
            fork_id,
            new_height,
            new_block_id
        );

        Ok(())
    }

    /// Get the current height of the main chain
    fn get_main_chain_height(&self, db: &WriteTransaction) -> Result<u32, BlockProcError> {
        // Find the maximum height in the heights table
        let heights_table = db
            .open_table(TABLE_HEIGHTS)
            .map_err(|e| BlockProcError::Custom(format!("Heights table error: {}", e)))?;

        let mut max_height = 0;
        let iter = heights_table
            .iter()
            .map_err(|e| BlockProcError::Custom(format!("Heights iterator error: {}", e)))?;

        for entry in iter {
            let (height, _) =
                entry.map_err(|e| BlockProcError::Custom(format!("Heights entry error: {}", e)))?;

            let h = height.value();
            if h > max_height {
                max_height = h;
            }
        }

        Ok(max_height)
    }

    /// Check if a block with the given hash is in the main chain
    fn is_block_in_main_chain(
        &self,
        db: &WriteTransaction,
        block_hash: BlockHash,
    ) -> Result<bool, BlockProcError> {
        // Look up the block ID
        let blockids_table = db
            .open_table(TABLE_BLOCKIDS)
            .map_err(|e| BlockProcError::Custom(format!("Block IDs table error: {}", e)))?;

        let block_id = match blockids_table
            .get(block_hash.to_byte_array())
            .map_err(|e| BlockProcError::Custom(format!("Block ID lookup error: {}", e)))?
        {
            Some(id_record) => id_record.value(),
            None => return Ok(false), // Block not found
        };

        // Check if this block ID has a height entry
        let block_heights_table = db
            .open_table(TABLE_BLOCK_HEIGHTS)
            .map_err(|e| BlockProcError::Custom(format!("Block heights table error: {}", e)))?;

        if block_heights_table
            .get(block_id)
            .map_err(|e| BlockProcError::Custom(format!("Block height lookup error: {}", e)))?
            .is_some()
        {
            return Ok(true); // Block has a height, so it's in the main chain
        }

        Ok(false)
    }

    /// Get blocks that need to be rolled back from the main chain
    /// Returns a list of (height, block_id) pairs, from highest to lowest height
    fn get_blocks_to_rollback(
        &self,
        db: &WriteTransaction,
        start_height: u32,
        end_height: u32,
    ) -> Result<Vec<(u32, BlockId)>, BlockProcError> {
        let mut blocks_to_rollback = Vec::new();

        let heights_table = db
            .open_table(TABLE_HEIGHTS)
            .map_err(|e| BlockProcError::Custom(format!("Heights table error: {}", e)))?;

        // We need to roll back from highest to lowest height
        for height in (start_height..=end_height).rev() {
            if let Some(block_id_record) = heights_table
                .get(height)
                .map_err(|e| BlockProcError::Custom(format!("Heights lookup error: {}", e)))?
            {
                blocks_to_rollback.push((height, block_id_record.value()));
            }
        }

        log::debug!(
            target: NAME,
            "Found {} blocks to roll back from heights {} to {}",
            blocks_to_rollback.len(),
            start_height,
            end_height
        );

        Ok(blocks_to_rollback)
    }

    /// Get blocks that need to be applied from the fork chain
    /// Returns a list of (height, block_id) pairs, from lowest to highest height
    fn get_blocks_to_apply(
        &self,
        db: &WriteTransaction,
        fork_id: ForkId,
        start_height: u32,
        end_height: u32,
    ) -> Result<Vec<(u32, BlockId)>, BlockProcError> {
        // Find the blocks in the fork that need to be applied
        // This is more complex as fork blocks aren't in the heights table yet

        // Get the tip block ID of the fork
        let (_fork_start_height, _fork_start_block_id, fork_tip_id, fork_height) =
            self.get_fork_info(db, fork_id)?;

        // We need to find all blocks from the tip down to the start height
        // Since they're not yet in the heights table, we need to traverse backwards

        // Start with the tip block
        let mut current_height = fork_height;
        let mut current_block_id = fork_tip_id;

        // Collect blocks (from high to low)
        let mut temp_blocks = Vec::new();

        let blks_table = db
            .open_table(TABLE_BLKS)
            .map_err(|e| BlockProcError::Custom(format!("Blocks table error: {}", e)))?;

        let blockids_table = db
            .open_table(TABLE_BLOCKIDS)
            .map_err(|e| BlockProcError::Custom(format!("Block IDs table error: {}", e)))?;

        while current_height >= start_height {
            temp_blocks.push((current_height, current_block_id));

            if current_height == start_height {
                break;
            }

            // Find the parent of this block
            let block_header = match blks_table
                .get(current_block_id)
                .map_err(|e| BlockProcError::Custom(format!("Block lookup error: {}", e)))?
            {
                Some(record) => record.value(),
                None => {
                    return Err(BlockProcError::Custom(format!(
                        "Block with ID {} not found in database",
                        current_block_id
                    )));
                }
            };

            let prev_hash = block_header.as_ref().prev_block_hash;

            // Find the block ID for this hash
            let prev_block_id = match blockids_table
                .get(prev_hash.to_byte_array())
                .map_err(|e| BlockProcError::Custom(format!("Block ID lookup error: {}", e)))?
            {
                Some(record) => record.value(),
                None => {
                    return Err(BlockProcError::Custom(format!(
                        "Previous block with hash {} not found in database",
                        prev_hash
                    )));
                }
            };

            current_block_id = prev_block_id;
            current_height -= 1;
        }

        // Reverse to get blocks from low to high
        let blocks_to_apply: Vec<(u32, crate::db::Id)> = temp_blocks.into_iter().rev().collect();

        log::debug!(
            target: NAME,
            "Found {} blocks to apply from heights {} to {}",
            blocks_to_apply.len(),
            start_height,
            end_height
        );

        Ok(blocks_to_apply)
    }

    /// Roll back blocks from the main chain
    fn rollback_blocks(
        &self,
        db: &WriteTransaction,
        blocks: &[(u32, BlockId)],
    ) -> Result<(), BlockProcError> {
        if blocks.is_empty() {
            return Ok(());
        }

        let mut total_txs_removed = 0;
        let mut total_utxos_restored = 0;
        let mut total_utxos_removed = 0;

        let block_spends_table = db
            .open_table(TABLE_BLOCK_SPENDS)
            .map_err(|e| BlockProcError::Custom(format!("Block spends table error: {}", e)))?;

        let mut utxos_table = db
            .open_table(TABLE_UTXOS)
            .map_err(|e| BlockProcError::Custom(format!("UTXOs table error: {}", e)))?;

        let block_txs_table = db
            .open_table(TABLE_BLOCK_TXS)
            .map_err(|e| BlockProcError::Custom(format!("Block-txs table error: {}", e)))?;

        let txes_table = db
            .open_table(TABLE_TXES)
            .map_err(|e| BlockProcError::Custom(format!("Txes table error: {}", e)))?;

        let mut heights_table = db
            .open_table(TABLE_HEIGHTS)
            .map_err(|e| BlockProcError::Custom(format!("Heights table error: {}", e)))?;

        let mut block_heights_table = db
            .open_table(TABLE_BLOCK_HEIGHTS)
            .map_err(|e| BlockProcError::Custom(format!("Block heights table error: {}", e)))?;

        // Iterate through blocks to roll back (should be in descending height order)
        for &(height, block_id) in blocks {
            log::info!(
                target: NAME,
                "Rolling back block at height {}: block ID {}",
                height,
                block_id
            );

            let mut block_utxos_restored = 0;
            let mut block_utxos_removed = 0;
            let mut block_txs_removed = 0;

            // 1. Restore UTXOs spent in this block
            if let Some(spends_record) = block_spends_table
                .get(block_id)
                .map_err(|e| BlockProcError::Custom(format!("Block spends lookup error: {}", e)))?
            {
                let spends = spends_record.value();
                block_utxos_restored = spends.len();
                total_utxos_restored += block_utxos_restored;

                // Restore each spent UTXO
                for (txno, vout) in spends {
                    utxos_table.insert((txno, vout), ()).map_err(|e| {
                        BlockProcError::Custom(format!("UTXO restoration error: {}", e))
                    })?;

                    log::debug!(
                        target: NAME,
                        "Restored UTXO: txno={}, vout={}",
                        txno,
                        vout
                    );
                }
            }

            // 2. Find all transactions in this block
            if let Some(txs_record) = block_txs_table
                .get(block_id)
                .map_err(|e| BlockProcError::Custom(format!("Block-txs lookup error: {}", e)))?
            {
                let txs = txs_record.value();
                block_txs_removed = txs.len();
                total_txs_removed += block_txs_removed;

                // For each transaction
                for txno in txs {
                    // 3. Remove UTXOs created by this transaction
                    if let Some(tx_record) = txes_table
                        .get(txno)
                        .map_err(|e| BlockProcError::Custom(format!("Tx lookup error: {}", e)))?
                    {
                        let tx = tx_record.value();
                        let num_outputs = tx.as_ref().outputs.len();
                        block_utxos_removed += num_outputs;
                        total_utxos_removed += num_outputs;

                        for vout in 0..num_outputs {
                            utxos_table.remove(&(txno, vout as u32)).map_err(|e| {
                                BlockProcError::Custom(format!("UTXO removal error: {}", e))
                            })?;

                            log::debug!(
                                target: NAME,
                                "Removed UTXO: txno={}, vout={}",
                                txno,
                                vout
                            );
                        }
                    }
                }
            }

            // 4. Remove this block from the heights tables
            heights_table
                .remove(height)
                .map_err(|e| BlockProcError::Custom(format!("Heights removal error: {}", e)))?;

            block_heights_table.remove(block_id).map_err(|e| {
                BlockProcError::Custom(format!("Block height removal error: {}", e))
            })?;

            log::debug!(
                target: NAME,
                "Removed block height mapping for height {} and block ID {}",
                height,
                block_id
            );

            log::info!(
                target: NAME,
                "Block rollback stats for height {}: removed {} transactions, restored {} UTXOs, removed {} UTXOs",
                height,
                block_txs_removed,
                block_utxos_restored,
                block_utxos_removed
            );
        }

        log::info!(
            target: NAME,
            "Successfully rolled back {} blocks: removed {} transactions, restored {} UTXOs, removed {} UTXOs",
            blocks.len(),
            total_txs_removed,
            total_utxos_restored,
            total_utxos_removed
        );

        Ok(())
    }

    /// Apply blocks from the fork chain to make it the new main chain
    /// This method processes all transactions in the fork blocks to ensure
    /// the UTXO set and other indexes are properly updated
    fn apply_blocks(
        &self,
        db: &WriteTransaction,
        blocks: &[(u32, BlockId)],
    ) -> Result<usize, BlockProcError> {
        if blocks.is_empty() {
            return Ok(0);
        }

        // Get current transaction number - we'll need this for processing new transactions
        let mut txno = {
            let main = db
                .open_table(TABLE_MAIN)
                .map_err(BlockProcError::MainTable)?;
            // Get current transaction number or use starting value if not found
            match main.get(REC_TXNO).map_err(BlockProcError::TxNoAbsent)? {
                Some(rec) => TxNo::from_slice(rec.value()).map_err(BlockProcError::TxNoInvalid)?,
                None => {
                    log::debug!(target: NAME, "No transaction counter found, starting from zero");
                    TxNo::start()
                }
            }
        };

        let mut total_txs_added = 0;
        let mut total_utxos_added = 0;
        let mut total_utxos_spent = 0;

        let fork_blocks_table = db
            .open_table(TABLE_FORK_BLOCKS)
            .map_err(|e| BlockProcError::Custom(format!("Fork blocks table error: {}", e)))?;

        let mut txids_table = db
            .open_table(TABLE_TXIDS)
            .map_err(BlockProcError::TxidTable)?;

        let mut txes_table = db
            .open_table(TABLE_TXES)
            .map_err(BlockProcError::TxesTable)?;

        let mut tx_blocks_table = db
            .open_table(TABLE_TX_BLOCKS)
            .map_err(|e| BlockProcError::Custom(format!("Tx-blocks table error: {}", e)))?;

        let mut utxos_table = db
            .open_table(TABLE_UTXOS)
            .map_err(|e| BlockProcError::Custom(format!("UTXOs table error: {}", e)))?;

        let mut inputs_table = db
            .open_table(TABLE_INPUTS)
            .map_err(|e| BlockProcError::Custom(format!("Inputs table error: {}", e)))?;

        let mut outs_table = db
            .open_table(TABLE_OUTS)
            .map_err(|e| BlockProcError::Custom(format!("Outs table error: {}", e)))?;

        let mut spks_table = db
            .open_table(TABLE_SPKS)
            .map_err(|e| BlockProcError::Custom(format!("SPKs table error: {}", e)))?;

        let mut block_txs_table = db
            .open_table(TABLE_BLOCK_TXS)
            .map_err(|e| BlockProcError::Custom(format!("Block-txs table error: {}", e)))?;

        let mut block_spends_table = db
            .open_table(TABLE_BLOCK_SPENDS)
            .map_err(|e| BlockProcError::Custom(format!("Block spends table error: {}", e)))?;

        let mut heights_table = db
            .open_table(TABLE_HEIGHTS)
            .map_err(|e| BlockProcError::Custom(format!("Heights table error: {}", e)))?;

        let mut block_heights_table = db
            .open_table(TABLE_BLOCK_HEIGHTS)
            .map_err(|e| BlockProcError::Custom(format!("Block heights table error: {}", e)))?;

        // Iterate through blocks to apply (should be in ascending height order)
        for &(height, block_id) in blocks {
            log::info!(
                target: NAME,
                "Applying block at height {}: block ID {}",
                height,
                block_id
            );

            // Get the complete block data from fork blocks table
            let block_data = fork_blocks_table
                .get(block_id)
                .map_err(|e| BlockProcError::Custom(format!("Fork block lookup error: {}", e)))?
                .ok_or_else(|| {
                    BlockProcError::Custom(format!("Fork block {} not found in database", block_id))
                })?
                .value();

            let block = block_data.as_ref();
            log::debug!(
                target: NAME,
                "Processing {} transactions from fork block {}",
                block.transactions.len(),
                block_id
            );

            let mut block_txs_added: usize = 0;
            let mut block_utxos_added: usize = 0;
            let mut block_utxos_spent: usize = 0;

            // Track UTXOs spent in this block
            let mut block_spends = Vec::new();

            // Track all transactions in this block
            let mut block_txs = Vec::new();

            // Process all transactions in the block
            for tx in &block.transactions {
                let txid = tx.txid();

                // For fork blocks, txids may already be in the database with assigned txno
                // Check if this txid already exists
                let existing_txno = txids_table
                    .get(txid.to_byte_array())
                    .map_err(BlockProcError::TxidLookup)?
                    .map(|v| v.value());

                let tx_txno = if let Some(existing) = existing_txno {
                    // Use the existing transaction number
                    existing
                } else {
                    // Assign a new transaction number
                    txno.inc_assign();
                    txno
                };

                // Add transaction to the list for this block
                block_txs.push(tx_txno);

                // If this is a new transaction, store its mapping and data
                if existing_txno.is_none() {
                    txids_table
                        .insert(txid.to_byte_array(), tx_txno)
                        .map_err(BlockProcError::TxidStorage)?;

                    // Store the transaction data
                    txes_table
                        .insert(tx_txno, DbTx::from(tx.clone()))
                        .map_err(BlockProcError::TxesStorage)?;

                    block_txs_added += 1;
                }

                // Associate transaction with block ID (update even if transaction existed)
                tx_blocks_table.insert(tx_txno, block_id).map_err(|e| {
                    BlockProcError::Custom(format!("Tx-blocks storage error: {}", e))
                })?;

                // Process transaction inputs
                for (vin_idx, input) in tx.inputs.iter().enumerate() {
                    if !input.prev_output.is_coinbase() {
                        let prev_txid = input.prev_output.txid;
                        let prev_vout = input.prev_output.vout;

                        // Look up previous transaction number
                        if let Some(prev_txno) = txids_table
                            .get(prev_txid.to_byte_array())
                            .map_err(BlockProcError::TxidLookup)?
                            .map(|v| v.value())
                        {
                            // Mark UTXO as spent
                            utxos_table
                                .remove(&(prev_txno, prev_vout.into_u32()))
                                .map_err(|e| {
                                    BlockProcError::Custom(format!("UTXOs removal error: {}", e))
                                })?;

                            block_utxos_spent += 1;

                            // Record UTXO spent in this block
                            block_spends.push((prev_txno, prev_vout.into_u32()));

                            // Record input-output mapping
                            inputs_table
                                .insert(
                                    (tx_txno, vin_idx as u32),
                                    (prev_txno, prev_vout.into_u32()),
                                )
                                .map_err(|e| {
                                    BlockProcError::Custom(format!("Inputs storage error: {}", e))
                                })?;

                            // Update spending relationships
                            let mut spending_txs = outs_table
                                .get(prev_txno)
                                .map_err(|e| {
                                    BlockProcError::Custom(format!("Outs lookup error: {}", e))
                                })?
                                .map(|v| v.value().to_vec())
                                .unwrap_or_default();

                            // Avoid duplicate entries
                            if !spending_txs.contains(&tx_txno) {
                                spending_txs.push(tx_txno);
                                outs_table.insert(prev_txno, spending_txs).map_err(|e| {
                                    BlockProcError::Custom(format!("Outs update error: {}", e))
                                })?;
                            }
                        }
                    }
                }

                // Process transaction outputs
                for (vout_idx, output) in tx.outputs.iter().enumerate() {
                    // Add new UTXO
                    utxos_table
                        .insert((tx_txno, vout_idx as u32), ())
                        .map_err(|e| {
                            BlockProcError::Custom(format!("UTXOs storage error: {}", e))
                        })?;

                    block_utxos_added += 1;

                    // Index script pubkey
                    let script = &output.script_pubkey;
                    if !script.is_empty() {
                        let mut txnos = spks_table
                            .get(script.as_slice())
                            .map_err(|e| {
                                BlockProcError::Custom(format!("SPKs lookup error: {}", e))
                            })?
                            .map(|v| v.value().to_vec())
                            .unwrap_or_default();

                        // Avoid duplicate entries
                        if !txnos.contains(&tx_txno) {
                            txnos.push(tx_txno);
                            spks_table.insert(script.as_slice(), txnos).map_err(|e| {
                                BlockProcError::Custom(format!("SPKs update error: {}", e))
                            })?;
                        }
                    }
                }
            }

            // Store all transaction numbers in this block
            block_txs_table
                .insert(block_id, block_txs)
                .map_err(|e| BlockProcError::Custom(format!("Block-txs storage error: {}", e)))?;

            // Store UTXOs spent in this block
            block_spends_table
                .insert(block_id, block_spends)
                .map_err(|e| {
                    BlockProcError::Custom(format!("Block spends storage error: {}", e))
                })?;

            // Update the heights tables
            heights_table
                .insert(height, block_id)
                .map_err(|e| BlockProcError::Custom(format!("Heights storage error: {}", e)))?;

            block_heights_table.insert(block_id, height).map_err(|e| {
                BlockProcError::Custom(format!("Block height storage error: {}", e))
            })?;

            log::debug!(
                target: NAME,
                "Updated block height mapping for height {} and block ID {}",
                height,
                block_id
            );

            total_txs_added += block_txs_added;
            total_utxos_added += block_utxos_added;
            total_utxos_spent += block_utxos_spent;

            log::info!(
                target: NAME,
                "Block apply stats for height {}: added {} transactions, added {} UTXOs, spent {} UTXOs",
                height,
                block_txs_added,
                block_utxos_added,
                block_utxos_spent
            );
        }

        // Update the global transaction counter
        let mut main = db
            .open_table(TABLE_MAIN)
            .map_err(BlockProcError::MainTable)?;
        main.insert(REC_TXNO, txno.to_byte_array().as_slice())
            .map_err(BlockProcError::TxNoUpdate)?;

        log::info!(
            target: NAME,
            "Successfully applied {} blocks: added {} transactions, added {} UTXOs, spent {} UTXOs",
            blocks.len(),
            total_txs_added,
            total_utxos_added,
            total_utxos_spent
        );

        Ok(total_txs_added)
    }

    /// Clean up fork information after a successful reorganization
    fn cleanup_after_reorg(
        &self,
        db: &WriteTransaction,
        applied_fork_id: ForkId,
    ) -> Result<(), BlockProcError> {
        // Get information about the applied fork
        let (_start_height, _start_block_id, _tip_id, fork_height) =
            match self.get_fork_info(db, applied_fork_id) {
                Ok(info) => info,
                Err(BlockProcError::Custom(msg)) if msg.contains("not found") => {
                    // Fork already removed, nothing to do
                    return Ok(());
                }
                Err(e) => return Err(e),
            };

        // Remove old forks that are now definitely invalid
        // Any fork that starts at a height less than the applied fork's height
        // and has not become the main chain by now should be removed
        let mut forks_table = db
            .open_table(TABLE_FORKS)
            .map_err(|e| BlockProcError::Custom(format!("Forks table error: {}", e)))?;

        let mut forks_to_remove = Vec::new();

        let iter = forks_table
            .iter()
            .map_err(|e| BlockProcError::Custom(format!("Forks iterator error: {}", e)))?;

        for entry in iter {
            let (fork_id, info) =
                entry.map_err(|e| BlockProcError::Custom(format!("Fork entry error: {}", e)))?;

            let fork_id_value = fork_id.value();

            // Skip the fork that was just applied
            if fork_id_value == applied_fork_id {
                continue;
            }

            let (start_height, _start_block_id, tip_id, current_height) = info.value();

            // If this fork is left behind the main chain, remove it
            if start_height < fork_height && current_height <= fork_height {
                forks_to_remove.push((fork_id_value, tip_id));
            }
        }

        // Now remove the outdated forks
        let mut fork_tips_table = db
            .open_table(TABLE_FORK_TIPS)
            .map_err(|e| BlockProcError::Custom(format!("Fork tips table error: {}", e)))?;

        for (fork_id, tip_id) in &forks_to_remove {
            // Remove the tip mapping
            fork_tips_table
                .remove(*tip_id)
                .map_err(|e| BlockProcError::Custom(format!("Fork tip removal error: {}", e)))?;

            // Remove the fork entry
            forks_table
                .remove(*fork_id)
                .map_err(|e| BlockProcError::Custom(format!("Fork removal error: {}", e)))?;

            log::info!(
                target: NAME,
                "Removed obsolete fork {} after reorganization",
                fork_id
            );
        }

        let (_start_height, _start_block_id, tip_id, _current_height) = {
            // Finally, remove the applied fork as well
            // Get the tip ID for the applied fork
            let fork_info = forks_table
                .get(applied_fork_id)
                .map_err(|e| BlockProcError::Custom(format!("Fork lookup error: {}", e)))?
                .expect("Applied fork should exist");
            fork_info.value()
        };

        // Remove the tip mapping
        fork_tips_table
            .remove(tip_id)
            .map_err(|e| BlockProcError::Custom(format!("Fork tip removal error: {}", e)))?;

        // Remove the fork entry
        forks_table
            .remove(applied_fork_id)
            .map_err(|e| BlockProcError::Custom(format!("Fork removal error: {}", e)))?;

        log::info!(
            target: NAME,
            "Removed applied fork {} after successful reorganization",
            applied_fork_id
        );

        Ok(())
    }

    /// Helper method to get fork information, reducing the need to repeatedly open the forks table
    fn get_fork_info(
        &self,
        db: &WriteTransaction,
        fork_id: ForkId,
    ) -> Result<(u32, BlockId, BlockId, u32), BlockProcError> {
        let forks_table = db
            .open_table(TABLE_FORKS)
            .map_err(|e| BlockProcError::Custom(format!("Forks table error: {}", e)))?;

        let fork_info = match forks_table
            .get(fork_id)
            .map_err(|e| BlockProcError::Custom(format!("Fork lookup error: {}", e)))?
        {
            Some(record) => record.value(),
            None => {
                return Err(BlockProcError::Custom(format!(
                    "Fork {} not found in database",
                    fork_id
                )));
            }
        };

        Ok(fork_info)
    }
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum BlockProcError {
    /// Unable to connect to database: {0}
    #[from]
    DbSend(SendError<DbMsg>),

    /// Broken broker link: {0}
    #[from]
    BrokerSend(SendError<ImporterMsg>),

    /// Unable to obtain database transaction: {0}
    #[from]
    Recv(RecvError),

    /// Unable to commit database transaction: {0}
    #[from]
    Commit(CommitError),

    /// Main table misses information about the latest transaction number. Details: {0}
    TxNoAbsent(StorageError),

    /// Latest transaction number in the main table contains invalid data: {0}
    TxNoInvalid(FromSliceError),

    /// Unable to store updated transaction number. Details: {0}
    TxNoUpdate(StorageError),

    /// Unable to open main table: {0}
    MainTable(TableError),

    /// Unable to open blocks table: {0}
    BlockTable(TableError),

    /// Unable to open heights table: {0}
    HeightsTable(TableError),

    /// Unable to write to blocks table: {0}
    BlockStorage(StorageError),

    /// Unable to open txids table: {0}
    TxidTable(TableError),

    /// Unable to write to txid table: {0}
    TxidStorage(StorageError),

    /// Unable to open transactions table: {0}
    TxesTable(TableError),

    /// Unable to write to transactions table: {0}
    TxesStorage(StorageError),

    /// Error looking up transaction ID: {0}
    TxidLookup(StorageError),

    /// Unable to find block: {0}
    BlockLookup(StorageError),

    /// Orphan block detected: parent block {0} not found
    OrphanBlock(BlockHash),

    /// Potential fork detected: new block {0} at height {1} conflicts with existing block {2}
    PotentialFork(BlockHash, u32, BlockId),

    /// Fork chain extension: new block {0} extends fork chain with parent block {1}
    ForkChainExtension(BlockHash, BlockHash),

    /// Database inconsistency: block {0} has parent {1} with ID {2} but missing height
    DatabaseInconsistency(BlockHash, BlockHash, BlockId),

    /// Custom error: {0}
    Custom(String),
}
