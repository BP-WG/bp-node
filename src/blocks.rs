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

use amplify::{ByteArray, Bytes32, FromSliceError};
use bpwallet::{Block, BlockHash};
use crossbeam_channel::{RecvError, SendError, Sender};
use microservices::USender;
use redb::{CommitError, ReadableTable, StorageError, TableError};

use crate::ImporterMsg;
use crate::db::{
    DbBlockHeader, DbMsg, DbTx, REC_TXNO, TABLE_BLKS, TABLE_MAIN, TABLE_TXES, TABLE_TXIDS, TxNo,
};

const NAME: &str = "blockproc";

pub struct BlockProcessor {
    db: USender<DbMsg>,
    broker: Sender<ImporterMsg>,
    tracking: HashSet<Bytes32>,
}

impl BlockProcessor {
    pub fn new(db: USender<DbMsg>, broker: Sender<ImporterMsg>) -> Self {
        Self { db, tracking: none!(), broker }
    }

    pub fn track(&mut self, filters: Vec<Bytes32>) { self.tracking.extend(filters); }

    pub fn untrack(&mut self, filters: Vec<Bytes32>) {
        self.tracking.retain(|filter| !filters.contains(filter));
    }

    pub fn process_block(&mut self, id: BlockHash, block: Block) -> Result<usize, BlockProcError> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.db.send(DbMsg::Write(tx))?;
        let db = rx.recv()?;

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

        let mut count = 0;
        let process = || -> Result<(), BlockProcError> {
            let mut table = db
                .open_table(TABLE_BLKS)
                .map_err(BlockProcError::BlockTable)?;
            table
                .insert(id.to_byte_array(), DbBlockHeader::from(block.header))
                .map_err(BlockProcError::BlockStorage)?;

            for tx in block.transactions {
                let txid = tx.txid();
                txno.inc_assign();

                let mut table = db
                    .open_table(TABLE_TXIDS)
                    .map_err(BlockProcError::TxidTable)?;
                table
                    .insert(txid.to_byte_array(), txno)
                    .map_err(BlockProcError::TxidStorage)?;

                // TODO: Add remaining transaction information to other database tables

                let mut table = db
                    .open_table(TABLE_TXES)
                    .map_err(BlockProcError::TxesTable)?;
                table
                    .insert(txno, DbTx::from(tx))
                    .map_err(BlockProcError::TxesStorage)?;

                // TODO: If txid match `tracking` Bloom filters, send information to the broker
                if false {
                    self.broker.send(ImporterMsg::Mined(txid))?;
                }

                count += 1;
            }

            let mut main = db
                .open_table(TABLE_MAIN)
                .map_err(BlockProcError::MainTable)?;
            main.insert(REC_TXNO, txno.to_byte_array().as_slice())
                .map_err(BlockProcError::TxNoUpdate)?;

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
}
