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

use std::cmp::Ordering;
use std::ops::ControlFlow;
use std::path::Path;

use amplify::num::u40;
use amplify::{ByteArray, FromSliceError};
use bpwallet::{BlockHeader, ConsensusDecode, ConsensusEncode, Tx};
use crossbeam_channel::{SendError, Sender};
use microservices::UService;
use redb::{
    Database, DatabaseError, ReadTransaction, TableDefinition, TransactionError, TypeName,
    WriteTransaction,
};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Display)]
#[display("#{0:010X}")]
pub struct TxNo(u40);

impl TxNo {
    pub fn start() -> Self { TxNo(u40::ONE) }

    pub fn inc_assign(&mut self) { self.0 += u40::ONE }
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

pub const TABLE_MAIN: TableDefinition<&'static str, &[u8]> = TableDefinition::new("main");
pub const TABLE_BLKS: TableDefinition<[u8; 32], DbBlockHeader> = TableDefinition::new("blocks");
pub const TABLE_TXIDS: TableDefinition<[u8; 32], TxNo> = TableDefinition::new("txids");
pub const TABLE_TXES: TableDefinition<TxNo, DbTx> = TableDefinition::new("transactions");
pub const TABLE_OUTS: TableDefinition<TxNo, Vec<TxNo>> = TableDefinition::new("spends");
pub const TABLE_SPKS: TableDefinition<&[u8], TxNo> = TableDefinition::new("scripts");

pub const REC_TXNO: &str = "txno";

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
