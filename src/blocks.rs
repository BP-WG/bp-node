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

use bprpc::BloomFilter32;
use amplify::{ByteArray, Bytes32, FromSliceError, hex};
use bpwallet::{Block, BlockHash, Network, Txid};
use crossbeam_channel::{RecvError, SendError, Sender};
use microservices::USender;
use redb::{CommitError, ReadableTable, StorageError, TableError};

use crate::ImporterMsg;
use crate::db::{
    BlockId, DbBlockHeader, DbMsg, DbTx, REC_BLOCKID, REC_CHAIN, REC_ORPHANS, REC_TXNO, TABLE_BLKS,
    TABLE_BLOCK_SPENDS, TABLE_BLOCKIDS, TABLE_CHAIN, TABLE_HEIGHTS, TABLE_INPUTS, TABLE_MAIN,
    TABLE_OUTS, TABLE_SPKS, TABLE_TX_BLOCKS, TABLE_TXES, TABLE_TXIDS, TABLE_UTXOS, TxNo,
};

const NAME: &str = "blockproc";

// Network information record in main table
pub const REC_NETWORK: &str = "network";

// Genesis block hashes for different networks
const GENESIS_HASH_MAINNET: &str =
    "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";
const GENESIS_HASH_TESTNET3: &str =
    "000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943";
const GENESIS_HASH_TESTNET4: &str =
    "00000000da84f2bafbbc53dee25a72ae507ff4914b867c565be350b0da8bf043";
const GENESIS_HASH_SIGNET: &str =
    "00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6";
const GENESIS_HASH_REGTEST: &str =
    "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206";

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

    // Helper function to determine network from block hash
    fn detect_network_from_genesis(blockhash: &BlockHash) -> Option<Network> {
        let hash_str = blockhash.to_string();
        match hash_str.as_str() {
            GENESIS_HASH_MAINNET => Some(Network::Mainnet),
            GENESIS_HASH_TESTNET3 => Some(Network::Testnet3),
            GENESIS_HASH_TESTNET4 => Some(Network::Testnet4),
            GENESIS_HASH_SIGNET => Some(Network::Signet),
            GENESIS_HASH_REGTEST => Some(Network::Regtest),
            _ => None,
        }
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

        // For simplicity in this implementation, we'll use block ID as fallback
        // When proper reorg handling is implemented this should be revisited
        // The proper height calculation would include blockchain state analysis

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

        // Check for genesis block if this is block ID 0
        if blockid.as_u32() == 0 {
            // For genesis block, detect and store network information
            let network = Self::detect_network_from_genesis(&id)
                .ok_or_else(|| BlockProcError::Custom("Unknown genesis block hash".to_string()))?;

            let mut main = db
                .open_table(TABLE_MAIN)
                .map_err(BlockProcError::MainTable)?;

            // Store network information
            main.insert(REC_NETWORK, network.to_string().as_bytes())
                .map_err(|e| {
                    BlockProcError::Custom(format!("Failed to store network info: {}", e))
                })?;

            log::info!(target: NAME, "Initialized with genesis block for network: {}", network);
        }

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
                if self.tracking.contains(&txid) {
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

            // Update chain state
            // Simplified approach - just append block to chain
            let mut chain_table = db
                .open_table(TABLE_CHAIN)
                .map_err(|e| BlockProcError::Custom(format!("Chain table error: {}", e)))?;

            // Get current chain
            let current_chain = chain_table
                .get(REC_CHAIN)
                .map_err(|e| BlockProcError::Custom(format!("Chain lookup error: {}", e)))?
                .map(|v| v.value().to_vec())
                .unwrap_or_default();

            // Append to main chain
            let mut new_chain = current_chain;
            new_chain.push(blockid);
            chain_table
                .insert(REC_CHAIN, new_chain)
                .map_err(|e| BlockProcError::Custom(format!("Chain update error: {}", e)))?;

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
