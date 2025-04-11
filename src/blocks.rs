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

use amplify::{ByteArray, FromSliceError};
use bprpc::BloomFilter32;
use bpwallet::{Block, BlockHash};
use crossbeam_channel::{RecvError, SendError, Sender};
use microservices::USender;
use redb::{CommitError, ReadableTable, StorageError, TableError};

use crate::ImporterMsg;
use crate::db::{
    BlockId, DbBlockHeader, DbMsg, DbTx, REC_BLOCKID, REC_CHAIN, REC_ORPHANS, REC_TXNO, TABLE_BLKS,
    TABLE_BLOCK_SPENDS, TABLE_BLOCKIDS, TABLE_HEIGHTS, TABLE_INPUTS, TABLE_MAIN, TABLE_OUTS,
    TABLE_SPKS, TABLE_TX_BLOCKS, TABLE_TXES, TABLE_TXIDS, TABLE_UTXOS, TxNo,
};

const NAME: &str = "blockproc";

// Network information record in main table
pub const REC_NETWORK: &str = "network";

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

    // Helper function to calculate block height
    fn calculate_block_height(
        &self,
        block: &Block,
        blockid: BlockId,
    ) -> Result<u32, BlockProcError> {
        // For genesis block, height is always 0
        // Check for all zeros hash which is the genesis block's prev_hash
        let zero_hash = [0u8; 32];
        if block.header.prev_block_hash.to_byte_array() == zero_hash {
            return Ok(0);
        }

        // Since each BP-Node instance works with a single chain,
        // for simplicity we use block ID as a height fallback.
        // In a multi-chain system, we would need more sophisticated height calculation.
        // When proper reorg handling is implemented this should be revisited.

        // For now, if this is genesis block (blockid == 0), return 0
        // otherwise, simply use blockid as height which will be roughly equivalent
        // This simplifies the logic while maintaining the distinction between concepts

        Ok(blockid.as_u32())
    }

    pub fn process_block(&mut self, id: BlockHash, block: Block) -> Result<usize, BlockProcError> {
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
        let blockid = {
            let main = db
                .open_table(TABLE_MAIN)
                .map_err(BlockProcError::MainTable)?;
            match main
                .get(REC_BLOCKID)
                .map_err(|e| BlockProcError::Custom(format!("Block ID lookup error: {}", e)))?
            {
                Some(rec) => {
                    // Parse bytes into BlockId using from_bytes method
                    let mut bid = BlockId::from_bytes(rec.value());
                    bid.inc_assign();
                    bid
                }
                None => BlockId::start(),
            }
        };

        let mut count = 0;
        let process = || -> Result<(), BlockProcError> {
            // Store block header
            let mut table = db
                .open_table(TABLE_BLKS)
                .map_err(BlockProcError::BlockTable)?;
            table
                .insert(id.to_byte_array(), DbBlockHeader::from(block.header))
                .map_err(BlockProcError::BlockStorage)?;

            // Map block hash to block ID
            let mut blockids_table = db
                .open_table(TABLE_BLOCKIDS)
                .map_err(|e| BlockProcError::Custom(format!("Block IDs table error: {}", e)))?;
            blockids_table
                .insert(id.to_byte_array(), blockid)
                .map_err(|e| BlockProcError::Custom(format!("Block ID storage error: {}", e)))?;

            // Calculate the block height based on previous block instead of using blockid
            // This is crucial for maintaining correct block heights during chain reorganizations
            let height = self.calculate_block_height(&block, blockid)?;

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
                    // For now, we'll just overwrite the existing entry
                }
            }

            heights_table
                .insert(height, blockid)
                .map_err(|e| BlockProcError::Custom(format!("Heights storage error: {}", e)))?;

            // Track UTXOs spent in this block
            let mut block_spends = Vec::new();

            // Process transactions in the block
            for tx in block.transactions {
                let txid = tx.txid();
                txno.inc_assign();

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

        if let Err(e) = process() {
            if let Err(err) = db.abort() {
                log::warn!(target: NAME, "Unable to abort failed database transaction due to {err}");
            };
            return Err(e);
        }
        db.commit()?;

        Ok(count)
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

    /// Custom error: {0}
    Custom(String),
}
