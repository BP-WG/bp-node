// BP Node: sovereign bitcoin wallet backend.
//
// SPDX-License-Identifier: Apache-2.0
//
// Designed & written in 2020-2025 by
//     @will-bitlight <https://bitlightlabs.com>
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2025 LNP/BP Labs, InDCS, Switzerland. All rights reserved.
// Copyright (C) 2020-2025 Dr Maxim Orlovsky. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//        http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
// or implied. See the License for the specific language governing permissions and limitations under
// the License.

use std::cmp::Ordering;
use std::ops::ControlFlow;
use std::path::Path;
use std::process::exit;

use amplify::num::u40;
use amplify::{ByteArray, FromSliceError};
use bpwallet::{Block, BlockHeader, ConsensusDecode, ConsensusEncode, Network, Tx};
use crossbeam_channel::{SendError, Sender};
use microservices::UService;
use redb::{
    Database, DatabaseError, Key, ReadTransaction, TableDefinition, TransactionError, TypeName,
    Value, WriteTransaction,
};

// see also constants in `bin/bpd.rs`
const EXIT_DB_INIT_MAIN_TABLE: i32 = 6;
const EXIT_DB_INIT_TABLE: i32 = 7;
const EXIT_DB_INIT_ERROR: i32 = 8;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Display)]
#[display("#{0:010X}")]
pub struct TxNo(u40);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Display)]
#[display("#{0:08X}")]
pub struct Id(u32);

pub type BlockId = Id;
pub type ForkId = Id;

impl TxNo {
    pub fn start() -> Self { TxNo(u40::ONE) }

    pub fn inc_assign(&mut self) { self.0 += u40::ONE }

    pub fn into_inner(self) -> u40 { self.0 }
}

impl Id {
    pub fn start() -> Self { Id(0) }

    pub fn inc_assign(&mut self) { self.0 += 1 }

    // Method to access the u32 value
    pub fn as_u32(&self) -> u32 { self.0 }

    // Method to get bytes representation
    pub fn to_bytes(&self) -> [u8; 4] { self.0.to_be_bytes() }

    // Method to create Id from bytes
    pub fn from_bytes(bytes: &[u8]) -> Self {
        debug_assert_eq!(bytes.len(), 4);
        let mut array = [0u8; 4];
        array.copy_from_slice(bytes);
        Id(u32::from_be_bytes(array))
    }

    pub fn into_inner(self) -> u32 { self.0 }
}

impl ByteArray<5> for TxNo {
    fn from_byte_array(val: impl Into<[u8; 5]>) -> Self { Self(u40::from_be_bytes(val.into())) }

    fn from_slice(slice: impl AsRef<[u8]>) -> Result<Self, FromSliceError> {
        let len = slice.as_ref().len();
        if len != 5 {
            return Err(FromSliceError { expected: 5, actual: len });
        }
        Ok(Self::from_slice_unsafe(slice))
    }

    fn from_slice_unsafe(slice: impl AsRef<[u8]>) -> Self {
        let mut buf = [0u8; 5];
        buf.copy_from_slice(slice.as_ref());
        Self::from_byte_array(buf)
    }

    fn to_byte_array(&self) -> [u8; 5] { self.0.to_be_bytes() }
}

#[derive(Wrapper, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, From)]
pub struct DbBlockHeader(#[from] BlockHeader);

#[derive(Wrapper, Clone, Eq, PartialEq, Debug, From)]
pub struct DbBlock(#[from] Block);

#[derive(Wrapper, Clone, Eq, PartialEq, Debug, From)]
pub struct DbTx(#[from] Tx);

impl redb::Key for TxNo {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering { data1.cmp(data2) }
}

impl redb::Value for TxNo {
    type SelfType<'a> = Self;

    type AsBytes<'a> = [u8; 5];

    fn fixed_width() -> Option<usize> { Some(5) }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where Self: 'a {
        debug_assert_eq!(data.len(), 5);
        TxNo::from_slice_unsafe(data)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where Self: 'b {
        value.to_byte_array()
    }

    fn type_name() -> TypeName { TypeName::new("BpNodeTxNo") }
}

impl redb::Value for DbBlockHeader {
    type SelfType<'a> = Self;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> { None }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where Self: 'a {
        Self(unsafe { BlockHeader::consensus_deserialize(data).unwrap_unchecked() })
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where Self: 'b {
        value.0.consensus_serialize()
    }

    fn type_name() -> TypeName { TypeName::new("BpNodeBlockHeader") }
}

impl redb::Value for DbBlock {
    type SelfType<'a> = Self;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> { None }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where Self: 'a {
        Self(unsafe { Block::consensus_deserialize(data).unwrap_unchecked() })
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where Self: 'b {
        value.0.consensus_serialize()
    }

    fn type_name() -> TypeName { TypeName::new("BpNodeBlock") }
}

impl redb::Value for DbTx {
    type SelfType<'a> = Self;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> { None }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where Self: 'a {
        Self(unsafe { Tx::consensus_deserialize(data).unwrap_unchecked() })
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where Self: 'b {
        value.0.consensus_serialize()
    }

    fn type_name() -> TypeName { TypeName::new("BpNodeTx") }
}

impl redb::Key for Id {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering { data1.cmp(data2) }
}

impl redb::Value for Id {
    type SelfType<'a> = Self;

    type AsBytes<'a> = [u8; 4];

    fn fixed_width() -> Option<usize> { Some(4) }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where Self: 'a {
        Id::from_bytes(data)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where Self: 'b {
        value.to_bytes()
    }

    fn type_name() -> TypeName { TypeName::new("BpNodeBlockId") }
}

pub const REC_TXNO: &str = "txno";
pub const REC_BLOCKID: &str = "blockid";
pub const REC_CHAIN: &str = "chain";
pub const REC_ORPHANS: &str = "orphans";
// Network information record in main table
pub const REC_NETWORK: &str = "network";
// Constants for fork management
pub const REC_FORK_ID: &str = "forkid";

// Main metadata table storing global counters and states
pub const TABLE_MAIN: TableDefinition<&'static str, &[u8]> = TableDefinition::new("main");

// Maps block hash to block header
pub const TABLE_BLKS: TableDefinition<BlockId, DbBlockHeader> = TableDefinition::new("blocks");

// Maps transaction ID to internal transaction number
pub const TABLE_TXIDS: TableDefinition<[u8; 32], TxNo> = TableDefinition::new("txids");

// Maps block hash to internal block ID
pub const TABLE_BLOCKIDS: TableDefinition<[u8; 32], BlockId> = TableDefinition::new("blockids");

// Stores complete transaction data
pub const TABLE_TXES: TableDefinition<TxNo, DbTx> = TableDefinition::new("transactions");

// Maps transaction number to transaction numbers that spend its outputs
pub const TABLE_OUTS: TableDefinition<TxNo, Vec<TxNo>> = TableDefinition::new("spends");

// Maps script pubkey to a list of transaction numbers containing it
pub const TABLE_SPKS: TableDefinition<&[u8], Vec<TxNo>> = TableDefinition::new("scripts");

// Tracks unspent transaction outputs
pub const TABLE_UTXOS: TableDefinition<(TxNo, u32), ()> = TableDefinition::new("utxos");

// Maps block height to block ID
pub const TABLE_HEIGHTS: TableDefinition<u32, BlockId> = TableDefinition::new("block_heights");

// Maps block ID to block height (reverse of TABLE_HEIGHTS)
pub const TABLE_BLOCK_HEIGHTS: TableDefinition<BlockId, u32> =
    TableDefinition::new("blockid_height");

// Maps transaction number to the block ID it belongs to
pub const TABLE_TX_BLOCKS: TableDefinition<TxNo, BlockId> = TableDefinition::new("tx_blocks");

// Maps block ID to all transaction numbers it contains
pub const TABLE_BLOCK_TXS: TableDefinition<BlockId, Vec<TxNo>> = TableDefinition::new("block_txs");

// Maps transaction input to the output it spends
pub const TABLE_INPUTS: TableDefinition<(TxNo, u32), (TxNo, u32)> = TableDefinition::new("inputs");

// Records all UTXOs spent in each block for potential rollback
pub const TABLE_BLOCK_SPENDS: TableDefinition<BlockId, Vec<(TxNo, u32)>> =
    TableDefinition::new("block_spends");

// Stores orphan blocks (blocks received without their parent blocks)
// Maps block hash to (block data, timestamp)
// Note: Orphan blocks are not assigned BlockId values because:
// 1. They are in a temporary state and may never become part of the main chain
// 2. Many orphans may eventually be discarded when their ancestry is resolved
// 3. BlockId resources are preserved for blocks that are (or may become) part of the chain
pub const TABLE_ORPHANS: TableDefinition<[u8; 32], (DbBlock, u64)> =
    TableDefinition::new("orphans");

// Maps parent block hash to list of orphan blocks that depend on it
pub const TABLE_ORPHAN_PARENTS: TableDefinition<[u8; 32], Vec<[u8; 32]>> =
    TableDefinition::new("orphan_parents");

// Tracks blockchain forks - maps fork ID to (fork_start_height, fork_start_block_id, tip_block_id,
// current_height)
pub const TABLE_FORKS: TableDefinition<ForkId, (u32, BlockId, BlockId, u32)> =
    TableDefinition::new("forks");

// Maps fork tip block ID to fork ID for quick lookup
pub const TABLE_FORK_TIPS: TableDefinition<BlockId, ForkId> = TableDefinition::new("fork_tips");

// Stores complete block data for fork blocks
// This allows us to access the full block content when performing chain reorganization
// Fork blocks are stored with their assigned BlockId like main chain blocks
pub const TABLE_FORK_BLOCKS: TableDefinition<BlockId, DbBlock> =
    TableDefinition::new("fork_blocks");

// Each BP-Node instance is designed to work with a single blockchain network.
// If multiple networks need to be indexed, separate instances should be used
// with different data directories. The network information is stored in the
// MAIN table under the REC_NETWORK key.
pub struct IndexDb(Database);

impl IndexDb {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        Database::open(path).map(Self)
    }
}

pub enum DbMsg {
    Read(Sender<ReadTransaction>),
    Write(Sender<WriteTransaction>),
}

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum IndexDbError {
    #[from]
    Transaction(TransactionError),

    #[from]
    Read(SendError<ReadTransaction>),

    #[from]
    Write(SendError<WriteTransaction>),
}

impl UService for IndexDb {
    type Msg = DbMsg;
    type Error = IndexDbError;
    const NAME: &'static str = "indexdb";

    fn process(&mut self, msg: Self::Msg) -> Result<ControlFlow<u8>, Self::Error> {
        match msg {
            DbMsg::Read(sender) => {
                let tx = self.0.begin_read()?;
                sender.send(tx)?;
            }
            DbMsg::Write(sender) => {
                let tx = self.0.begin_write()?;
                sender.send(tx)?;
            }
        }
        Ok(ControlFlow::Continue(()))
    }

    fn terminate(&mut self) {
        log::info!("Compacting database on shutdown...");
        if let Err(e) = self.0.compact() {
            log::error!("Failed to compact database: {e}");
        }
    }
}

/// Initialize database tables
pub fn initialize_db_tables(db: &Database, network: Network) {
    // It's necessary to open all tables with WriteTransaction to ensure they are created
    // In ReDB, tables are only created when first opened with a WriteTransaction
    // If later accessed with ReadTransaction without being created first, errors will occur
    match db.begin_write() {
        Ok(tx) => {
            // Initialize main table with network information
            initialize_main_table(&tx, network);

            // Initialize all other tables by group
            create_core_tables(&tx);
            create_utxo_tables(&tx);
            create_block_height_tables(&tx);
            create_transaction_block_tables(&tx);
            create_orphan_tables(&tx);
            create_fork_tables(&tx);

            // Commit the transaction
            if let Err(err) = tx.commit() {
                eprintln!("Failed to commit initial database transaction: {err}");
                exit(EXIT_DB_INIT_ERROR);
            }
        }
        Err(err) => {
            eprintln!("Failed to begin database transaction: {err}");
            exit(EXIT_DB_INIT_ERROR);
        }
    }
}

/// Initialize the main table with network information
fn initialize_main_table(tx: &WriteTransaction, network: Network) {
    match tx.open_table(TABLE_MAIN) {
        Ok(mut main_table) => {
            if let Err(err) = main_table.insert(REC_NETWORK, network.to_string().as_bytes()) {
                eprintln!("Failed to write network information to database: {err}");
                exit(EXIT_DB_INIT_MAIN_TABLE);
            }
        }
        Err(err) => {
            eprintln!("Failed to open main table in database: {err}");
            exit(EXIT_DB_INIT_MAIN_TABLE);
        }
    }
}

/// Create core block and transaction tables
fn create_core_tables(tx: &WriteTransaction) {
    log::info!("Creating core block and transaction tables...");
    create_table(tx, TABLE_BLKS, "blocks");
    create_table(tx, TABLE_TXIDS, "txids");
    create_table(tx, TABLE_BLOCKIDS, "blockids");
    create_table(tx, TABLE_TXES, "transactions");
}

/// Create UTXO and transaction relationship tables
fn create_utxo_tables(tx: &WriteTransaction) {
    log::info!("Creating UTXO and transaction relationship tables...");
    create_table(tx, TABLE_OUTS, "spends");
    create_table(tx, TABLE_SPKS, "scripts");
    create_table(tx, TABLE_UTXOS, "utxos");
}

/// Create block height mapping tables
fn create_block_height_tables(tx: &WriteTransaction) {
    log::info!("Creating block height mapping tables...");
    create_table(tx, TABLE_HEIGHTS, "block_heights");
    create_table(tx, TABLE_BLOCK_HEIGHTS, "blockid_height");
}

/// Create transaction-block relationship tables
fn create_transaction_block_tables(tx: &WriteTransaction) {
    log::info!("Creating transaction-block relationship tables...");
    create_table(tx, TABLE_TX_BLOCKS, "tx_blocks");
    create_table(tx, TABLE_BLOCK_TXS, "block_txs");
    create_table(tx, TABLE_INPUTS, "inputs");
    create_table(tx, TABLE_BLOCK_SPENDS, "block_spends");
}

/// Create orphan blocks tables
fn create_orphan_tables(tx: &WriteTransaction) {
    log::info!("Creating orphan blocks tables...");
    create_table(tx, TABLE_ORPHANS, "orphans");
    create_table(tx, TABLE_ORPHAN_PARENTS, "orphan_parents");
}

/// Create fork management tables
fn create_fork_tables(tx: &WriteTransaction) {
    log::info!("Creating fork management tables...");
    create_table(tx, TABLE_FORKS, "forks");
    create_table(tx, TABLE_FORK_TIPS, "fork_tips");
    create_table(tx, TABLE_FORK_BLOCKS, "fork_blocks");
}

/// Generic function to create a table with error handling
fn create_table<K: Key + 'static, V: Value + 'static>(
    tx: &WriteTransaction,
    table_def: TableDefinition<K, V>,
    table_name: &str,
) {
    if let Err(err) = tx.open_table(table_def) {
        eprintln!("Failed to create {} table: {err}", table_name);
        exit(EXIT_DB_INIT_TABLE);
    }
}
