// Bitcoin transaction processing & database indexing rust library
// Written in 2020 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use super::schema::*;
use bitcoin::{Transaction, TxIn, TxOut};
use chrono::NaiveDateTime;
use bp::short_id::{BlockChecksum, Descriptor, Error};

#[derive(Identifiable, Queryable, Insertable, Clone, Debug, Display)]
#[display_from(Debug)]
#[table_name = "block"]
pub struct Block {
    pub id: i64,
    pub block_id: Vec<u8>,
    pub merkle_root: Vec<u8>,
    pub ts: NaiveDateTime,
    pub difficulty: i64,
    pub nonce: i32,
    pub ver: i32,
    pub tx_count: i32,
}

impl Block {
    pub fn compose(
        block: &bitcoin::Block,
        descriptor: Descriptor,
    ) -> Result<Self, Error> {
        Ok(Self {
            id: descriptor.try_into_u64()? as i64,
            block_id: block.block_hash().to_vec(),
            merkle_root: block.merkle_root().to_vec(),
            ts: NaiveDateTime::from_timestamp(block.header.time as i64, 0),
            difficulty: block.header.bits as i64,
            nonce: block.header.nonce as i32,
            ver: block.header.version as i32,
            tx_count: block.txdata.len() as i32,
        })
    }

    #[allow(dead_code)]
    fn from_blockchain(block: &bitcoin::Block, block_height: u32) -> Self {
        let descriptor = Descriptor::OnchainBlock {
            block_height,
            block_checksum: BlockChecksum::from(block.block_hash()),
        };
        Self::compose(block, descriptor).expect("Just generated descriptor can't fail")
    }
}

#[derive(Identifiable, Queryable, Insertable, Clone, Debug, Display)]
#[display_from(Debug)]
#[table_name = "tx"]
pub struct Tx {
    pub id: i64,
    pub ver: i32,
    pub locktime: i32,
    pub out_count: i16,
    pub in_count: i16,
    pub fee: Option<i64>,
}

impl Tx {
    pub fn compose(
        tx: &Transaction,
        descriptor: Descriptor,
    ) -> Result<Self, Error> {
        Ok(Self {
            id: descriptor.try_into_u64()? as i64,
            ver: tx.version as i32,
            locktime: tx.lock_time as i32,
            out_count: tx.output.len() as i16,
            in_count: tx.input.len() as i16,
            fee: None,
        })
    }
}

#[derive(Identifiable, Queryable, Insertable, Clone, Debug, Display)]
#[display_from(Debug)]
#[table_name = "txin"]
pub struct Txin {
    pub id: i64,
    pub seq: i32,
    pub txout_id: i64,
}

impl Txin {
    pub fn compose(
        txin: &TxIn,
        descriptor: Descriptor,
        txo_descriptor: Descriptor,
    ) -> Result<Self, Error> {
        Ok(Self {
            id: descriptor.try_into_u64()? as i64,
            seq: txin.sequence as i32,
            txout_id: txo_descriptor.try_into_u64()? as i64,
        })
    }
}

#[derive(Identifiable, Queryable, Insertable, Clone, Debug, Display)]
#[display_from(Debug)]
#[table_name = "txout"]
pub struct Txout {
    pub id: i64,
    pub amount: i64,
    pub script: Vec<u8>,
}

impl Txout {
    pub fn compose(
        txout: &TxOut,
        descriptor: Descriptor,
    ) -> Result<Self, Error> {
        Ok(Self {
            id: descriptor.try_into_u64()? as i64,
            amount: txout.value as i64,
            script: txout.script_pubkey.to_bytes(),
        })
    }
}
