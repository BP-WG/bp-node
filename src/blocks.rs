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

use std::net::{SocketAddr, TcpStream};
use std::process::exit;

use amplify::{ByteArray, FromSliceError};
use bprpc::{BlockMsg, RemoteAddr, Session};
use bpwallet::BlockHash;
use crossbeam_channel::{RecvError, SendError};
use microservices::USender;
use netservices::client::{ClientDelegate, ConnectionDelegate, OnDisconnect};
use netservices::{Frame, ImpossibleResource, NetTransport};
use redb::{CommitError, ReadableTable, StorageError, TableError};

use crate::db::{
    DbBlockHeader, DbMsg, DbTx, REC_TXNO, TABLE_BLKS, TABLE_MAIN, TABLE_TXES, TABLE_TXIDS, TxNo,
};

const NAME: &str = "importer";

pub struct BlockImporter {
    db: USender<DbMsg>,
    provider: RemoteAddr,
}

impl BlockImporter {
    pub fn new(db: USender<DbMsg>, remote: RemoteAddr) -> Self { Self { db, provider: remote } }

    fn process_block(&mut self, id: BlockHash, block: BlockMsg) -> Result<usize, BlockProcError> {
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

impl ConnectionDelegate<RemoteAddr, Session> for BlockImporter {
    fn connect(&self, remote: &RemoteAddr) -> Session {
        debug_assert_eq!(remote, &self.provider);
        TcpStream::connect(remote).unwrap_or_else(|err| {
            log::error!(target: NAME, "Unable to connect blockchain provider {remote} due to {err}");
            log::warn!(target: NAME, "Stopping RPC import thread");
            exit(1);
        })
    }

    fn on_established(&self, remote: SocketAddr, _attempt: usize) {
        log::info!(target: NAME, "Connected to blockchain provider {} ({remote})", self.provider);
    }

    fn on_disconnect(&self, err: std::io::Error, _attempt: usize) -> OnDisconnect {
        log::error!(target: NAME, "Blockchain provider {} got disconnected due to {err}", self.provider);
        log::warn!(target: NAME, "Stopping RPC import thread");
        exit(1)
    }

    fn on_io_error(&self, err: reactor::Error<ImpossibleResource, NetTransport<Session>>) {
        log::error!(target: NAME, "I/O error in communicating with blockchain provider {}: {err}", self.provider);
    }
}

impl ClientDelegate<RemoteAddr, Session> for BlockImporter {
    type Reply = BlockMsg;

    fn on_reply(&mut self, block: BlockMsg) {
        let block_id = block.header.block_hash();
        log::debug!("Received block {block_id} from {}", self.provider);
        match self.process_block(block_id, block) {
            Err(err) => {
                log::error!(target: NAME, "{err}");
                log::warn!(target: NAME, "Block {block_id} got dropped due to database connectivity issue");
            }
            Ok(count) => {
                log::debug!("Successfully processed block {block_id}; {count} transactions added");
            }
        }
    }

    fn on_reply_unparsable(&mut self, err: <Self::Reply as Frame>::Error) {
        log::error!("Invalid message from blockchain provider {}: {err}", self.provider);
    }
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
enum BlockProcError {
    /// Unable to connect to database: {0}
    #[from]
    Send(SendError<DbMsg>),

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
