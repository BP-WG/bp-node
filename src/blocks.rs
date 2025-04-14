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
use bpwallet::{Block, BlockHash, ConsensusDecode, ConsensusEncode};
use crossbeam_channel::{RecvError, SendError, Sender};
use microservices::USender;
use redb::{CommitError, ReadableTable, ReadableTableMetadata, StorageError, TableError};

use crate::ImporterMsg;
use crate::db::{
    BlockId, DbBlock, DbBlockHeader, DbMsg, DbTx, REC_BLOCKID, REC_TXNO, TABLE_BLKS,
    TABLE_BLOCK_HEIGHTS, TABLE_BLOCK_SPENDS, TABLE_BLOCK_TXS, TABLE_BLOCKIDS, TABLE_HEIGHTS,
    TABLE_INPUTS, TABLE_MAIN, TABLE_ORPHAN_PARENTS, TABLE_ORPHANS, TABLE_OUTS, TABLE_SPKS,
    TABLE_TX_BLOCKS, TABLE_TXES, TABLE_TXIDS, TABLE_UTXOS, TxNo,
};

const NAME: &str = "blockproc";

// Network information record in main table
pub const REC_NETWORK: &str = "network";

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
    fn calculate_block_height(&self, block: &Block) -> Result<u32, BlockProcError> {
        // For genesis block, height is always 0
        // Check for all zeros hash which is the genesis block's prev_hash
        let zero_hash = [0u8; 32];
        if block.header.prev_block_hash.to_byte_array() == zero_hash {
            return Ok(0);
        }

        // Find block height of the previous block and add 1
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.db.send(DbMsg::Write(tx))?;
        let db = rx.recv()?;

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

        let prev_blockid_record = prev_blockid.unwrap();
        // Get the previous block's ID
        let prev_blockid = prev_blockid_record.value();

        // First check the BlockId to height mapping table which is more efficient
        let block_heights_table = db
            .open_table(TABLE_BLOCK_HEIGHTS)
            .map_err(|e| BlockProcError::Custom(format!("Block heights table error: {}", e)))?;

        if let Some(prev_height_record) = block_heights_table
            .get(prev_blockid)
            .map_err(|e| BlockProcError::Custom(format!("Block height lookup error: {}", e)))?
        {
            let prev_height = prev_height_record.value();
            return Ok(prev_height + 1);
        }

        // If not found in the direct mapping table, check the height -> blockid table
        let heights_table = db
            .open_table(TABLE_HEIGHTS)
            .map_err(|e| BlockProcError::Custom(format!("Heights table error: {}", e)))?;

        // Scan the heights table to find the previous block ID
        let heights_iter = heights_table
            .iter()
            .map_err(|e| BlockProcError::Custom(format!("Heights table iterator error: {}", e)))?;

        for height_entry in heights_iter {
            let (height, block_id) = height_entry
                .map_err(|e| BlockProcError::Custom(format!("Heights entry error: {}", e)))?;

            if block_id.value() == prev_blockid {
                // Previous block's height + 1 is the current block's height
                return Ok(height.value() + 1);
            }
        }

        // If we couldn't find the previous block in either height table,
        // this is an error condition as the database is in an inconsistent state
        Err(BlockProcError::Custom(format!(
            "Database inconsistency: Previous block with ID {} found in blockids table but not in \
             any height table",
            prev_blockid
        )))
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

        // Get current transaction number
        let mut txno = {
            let main = db
                .open_table(TABLE_MAIN)
                .map_err(BlockProcError::MainTable)?;
            let rec = main
                .get(REC_TXNO)
                .map_err(BlockProcError::TxNoAbsent)?
                .unwrap();
            TxNo::from_slice(rec.value()).map_err(BlockProcError::TxNoInvalid)?
        };

        // Get or create the next block ID
        let mut blockid = {
            let main = db
                .open_table(TABLE_MAIN)
                .map_err(BlockProcError::MainTable)?;
            match main
                .get(REC_BLOCKID)
                .map_err(|e| BlockProcError::Custom(format!("Block ID lookup error: {}", e)))?
            {
                Some(rec) => {
                    // Parse bytes into BlockId using from_bytes method
                    BlockId::from_bytes(rec.value())
                }
                None => BlockId::start(),
            }
        };

        let mut count = 0;
        let process = || -> Result<(), BlockProcError> {
            // Calculate the block height based on previous block
            // This function will also detect orphan blocks
            let height = match self.calculate_block_height(&block) {
                Ok(h) => h,
                Err(BlockProcError::OrphanBlock(_)) => {
                    // If we detect an orphan block, abort this transaction and save the orphan
                    return Err(BlockProcError::OrphanBlock(parent_hash));
                }
                Err(e) => return Err(e),
            };

            blockid.inc_assign();

            // Store block header
            let mut table = db
                .open_table(TABLE_BLKS)
                .map_err(BlockProcError::BlockTable)?;
            table
                .insert(blockid, DbBlockHeader::from(block.header))
                .map_err(BlockProcError::BlockStorage)?;

            // Map block hash to block ID
            let mut blockids_table = db
                .open_table(TABLE_BLOCKIDS)
                .map_err(|e| BlockProcError::Custom(format!("Block IDs table error: {}", e)))?;
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

            // Store block height information
            let mut heights_table = db
                .open_table(TABLE_HEIGHTS)
                .map_err(|e| BlockProcError::Custom(format!("Heights table error: {}", e)))?;

            // Check if we already have a block at this height
            if let Some(existing_blockid) = heights_table
                .get(height)
                .map_err(|e| BlockProcError::Custom(format!("Heights lookup error: {}", e)))?
                .map(|v| v.value())
            {
                // If different block at this height, we have a potential reorg
                if existing_blockid != blockid {
                    log::warn!(
                        target: NAME,
                        "Detected potential chain reorganization at height {}: replacing block ID {} with {}",
                        height,
                        existing_blockid,
                        blockid
                    );

                    // TODO: Implement full reorg handling
                    // In a single-chain BP-Node instance, reorgs are detected when a different
                    // block is encountered at the same height. The proper handling would include:
                    // 1. Finding the common ancestor block
                    // 2. Rolling back transactions in the old chain branch
                    // 3. Applying transactions from the new chain branch
                    // 4. Updating UTXO set accordingly

                    // When implementing reorg, make sure to update both height tables:
                    // - TABLE_HEIGHTS: height -> blockid mapping
                    // - TABLE_BLOCK_HEIGHTS: blockid -> height mapping

                    // For now, we'll just overwrite the existing entry
                    // This simple approach doesn't handle the full reorg properly
                    // but ensures the database doesn't get into an inconsistent state
                }
            }

            heights_table
                .insert(height, blockid)
                .map_err(|e| BlockProcError::Custom(format!("Heights storage error: {}", e)))?;

            // Also update the reverse mapping (blockid -> height)
            let mut block_heights_table = db
                .open_table(TABLE_BLOCK_HEIGHTS)
                .map_err(|e| BlockProcError::Custom(format!("Block heights table error: {}", e)))?;
            block_heights_table.insert(blockid, height).map_err(|e| {
                BlockProcError::Custom(format!("Block height storage error: {}", e))
            })?;

            // Track UTXOs spent in this block
            let mut block_spends = Vec::new();

            // Track all transactions in this block
            let mut block_txs = Vec::new();

            // Process transactions in the block
            for tx in block.transactions {
                let txid = tx.txid();
                txno.inc_assign();

                // Add transaction to the list for this block
                block_txs.push(txno);

                // Store transaction ID to transaction number mapping
                let mut txids_table = db
                    .open_table(TABLE_TXIDS)
                    .map_err(BlockProcError::TxidTable)?;
                txids_table
                    .insert(txid.to_byte_array(), txno)
                    .map_err(BlockProcError::TxidStorage)?;

                // Associate transaction with block ID
                let mut tx_blocks_table = db
                    .open_table(TABLE_TX_BLOCKS)
                    .map_err(|e| BlockProcError::Custom(format!("Tx-blocks table error: {}", e)))?;
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
                            let mut utxos_table = db.open_table(TABLE_UTXOS).map_err(|e| {
                                BlockProcError::Custom(format!("UTXOs table error: {}", e))
                            })?;
                            utxos_table
                                .remove(&(prev_txno, prev_vout.into_u32()))
                                .map_err(|e| {
                                    BlockProcError::Custom(format!("UTXOs removal error: {}", e))
                                })?;

                            // Record UTXO spent in this block
                            block_spends.push((prev_txno, prev_vout.into_u32()));

                            // Record input-output mapping
                            let mut inputs_table = db.open_table(TABLE_INPUTS).map_err(|e| {
                                BlockProcError::Custom(format!("Inputs table error: {}", e))
                            })?;
                            inputs_table
                                .insert((txno, vin_idx as u32), (prev_txno, prev_vout.into_u32()))
                                .map_err(|e| {
                                    BlockProcError::Custom(format!("Inputs storage error: {}", e))
                                })?;

                            // Update spending relationships
                            let mut outs_table = db.open_table(TABLE_OUTS).map_err(|e| {
                                BlockProcError::Custom(format!("Outs table error: {}", e))
                            })?;
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
                    let mut utxos_table = db
                        .open_table(TABLE_UTXOS)
                        .map_err(|e| BlockProcError::Custom(format!("UTXOs table error: {}", e)))?;
                    utxos_table
                        .insert((txno, vout_idx as u32), ())
                        .map_err(|e| {
                            BlockProcError::Custom(format!("UTXOs storage error: {}", e))
                        })?;

                    // Index script pubkey
                    let script = &output.script_pubkey;
                    if !script.is_empty() {
                        let mut spks_table = db.open_table(TABLE_SPKS).map_err(|e| {
                            BlockProcError::Custom(format!("SPKs table error: {}", e))
                        })?;
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
                let mut txes_table = db
                    .open_table(TABLE_TXES)
                    .map_err(BlockProcError::TxesTable)?;
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
            let mut block_txs_table = db
                .open_table(TABLE_BLOCK_TXS)
                .map_err(|e| BlockProcError::Custom(format!("Block-txs table error: {}", e)))?;
            block_txs_table
                .insert(blockid, block_txs)
                .map_err(|e| BlockProcError::Custom(format!("Block-txs storage error: {}", e)))?;

            // Store UTXOs spent in this block
            let mut block_spends_table = db
                .open_table(TABLE_BLOCK_SPENDS)
                .map_err(|e| BlockProcError::Custom(format!("Block spends table error: {}", e)))?;
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
            main.insert(REC_TXNO, txno.to_byte_array().as_slice())
                .map_err(BlockProcError::TxNoUpdate)?;

            // Update block ID counter
            main.insert(REC_BLOCKID, &blockid.to_bytes().as_slice())
                .map_err(|e| BlockProcError::Custom(format!("Block ID update error: {}", e)))?;

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
            Err(BlockProcError::OrphanBlock(_)) => {
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

                return self.save_orphan_block(id, block_clone);
            }
            Err(e) => {
                // Handle other errors
                if let Err(err) = db.abort() {
                    log::warn!(target: NAME, "Unable to abort failed database transaction due to {err}");
                };
                return Err(e);
            }
            Ok(()) => {
                // Successful processing
                db.commit()?;

                // After successful processing, check if we have any orphans that depend on this
                // block
                self.process_orphans(id)?;

                // Final log message
                log::debug!(
                    target: NAME,
                    "Successfully processed block {} with {} transactions",
                    id,
                    count
                );

                Ok(count)
            }
        }
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

    // Process orphan blocks that depend on a given block
    fn process_orphans(&mut self, parent_id: BlockHash) -> Result<(), BlockProcError> {
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

        // If no orphans depend on this block, we're done
        if orphans.is_none() {
            return Ok(());
        }

        // Get list of orphan block hashes
        let orphan_hashes = orphans.unwrap().value().to_vec();

        // Process each orphan block
        let orphans_table = db
            .open_table(TABLE_ORPHANS)
            .map_err(|e| BlockProcError::Custom(format!("Orphans table error: {}", e)))?;

        let mut processed_orphans = Vec::with_capacity(orphan_hashes.len());

        for orphan_hash in &orphan_hashes {
            // Get the orphan block data
            if let Some(orphan_block_data) = orphans_table
                .get(orphan_hash)
                .map_err(|e| BlockProcError::Custom(format!("Orphan lookup error: {}", e)))?
            {
                let (_block_data, _timestamp) = orphan_block_data.value();

                // TODO: Implement
                todo!();
                // Track that we processed this orphan
                processed_orphans.push(orphan_hash.clone());
            }
        }

        // Remove processed orphans from the database
        if !processed_orphans.is_empty() {
            let (tx, rx) = crossbeam_channel::bounded(1);
            self.db.send(DbMsg::Write(tx))?;
            let write_db = rx.recv()?;

            let remove_processed = || -> Result<(), BlockProcError> {
                // Remove from orphan parents table
                let mut orphan_parents_table =
                    write_db.open_table(TABLE_ORPHAN_PARENTS).map_err(|e| {
                        BlockProcError::Custom(format!("Orphan parents table error: {}", e))
                    })?;

                // Remove the whole entry if all orphans for this parent were processed
                if orphan_hashes.len() == processed_orphans.len() {
                    orphan_parents_table.remove(parent_hash).map_err(|e| {
                        BlockProcError::Custom(format!("Orphan parents removal error: {}", e))
                    })?;
                } else {
                    // Otherwise, update the list to remove the processed orphans
                    let remaining_orphans: Vec<[u8; 32]> = orphan_hashes
                        .into_iter()
                        .filter(|h| !processed_orphans.contains(&h))
                        .collect();

                    orphan_parents_table
                        .insert(parent_hash, remaining_orphans)
                        .map_err(|e| {
                            BlockProcError::Custom(format!("Parent update error: {}", e))
                        })?;
                }

                // Remove from orphans table
                let mut orphans_table = write_db
                    .open_table(TABLE_ORPHANS)
                    .map_err(|e| BlockProcError::Custom(format!("Orphans table error: {}", e)))?;

                for orphan_hash in &processed_orphans {
                    orphans_table.remove(*orphan_hash).map_err(|e| {
                        BlockProcError::Custom(format!("Orphan removal error: {}", e))
                    })?;
                }

                Ok(())
            };

            if let Err(e) = remove_processed() {
                if let Err(err) = write_db.abort() {
                    log::warn!(
                        target: NAME,
                        "Unable to abort failed orphan cleanup transaction due to {err}"
                    );
                };
                return Err(e);
            }

            write_db.commit()?;

            log::info!(
                target: NAME,
                "Cleaned up {} processed orphan blocks",
                processed_orphans.len()
            );
        }

        Ok(())
    }

    // Count total number of orphan blocks
    fn count_orphans(&self) -> Result<usize, BlockProcError> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.db.send(DbMsg::Read(tx))?;
        let db = rx.recv()?;

        let orphans_table = db
            .open_table(TABLE_ORPHANS)
            .map_err(|e| BlockProcError::Custom(format!("Orphans table error: {}", e)))?;

        let count = orphans_table
            .len()
            .map_err(|e| BlockProcError::Custom(format!("Orphans table length error: {}", e)))?;

        Ok(count as usize)
    }

    // Remove expired orphan blocks
    fn clean_expired_orphans(&self) -> Result<(), BlockProcError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        let expiry_seconds = ORPHAN_EXPIRY_HOURS * 3600;

        // First read to find expired orphans
        let (read_tx, read_rx) = crossbeam_channel::bounded(1);
        self.db.send(DbMsg::Read(read_tx))?;
        let read_db = read_rx.recv()?;

        let orphans_table = read_db
            .open_table(TABLE_ORPHANS)
            .map_err(|e| BlockProcError::Custom(format!("Orphans table error: {}", e)))?;

        let mut expired_orphans = Vec::new();

        let orphans_iter = orphans_table.iter().map_err(|e| {
            BlockProcError::Custom(format!("Failed to iterate orphans table: {}", e))
        })?;

        for entry in orphans_iter {
            let (hash, data) = entry.map_err(|e| {
                BlockProcError::Custom(format!("Failed to read orphan entry: {}", e))
            })?;

            let (_block_data, timestamp) = data.value();

            // Check if orphan has expired
            if now - timestamp > expiry_seconds {
                expired_orphans.push(hash.value());
            }
        }

        // If we have expired orphans, remove them
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
                        parents_to_scan
                            .push((parent_hash.value().clone(), orphans.value().to_vec()));
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

    /// Custom error: {0}
    Custom(String),
}
