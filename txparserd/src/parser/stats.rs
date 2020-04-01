// Bitcoin transaction processing & database indexing daemon
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


use chrono::{NaiveDateTime, NaiveDate, Utc};
use diesel::{
    prelude::*,
    sql_types::Interval,
    pg::data_types::PgInterval
};
use txlib::lnpbp::bp::{short_id, Descriptor, BlockChecksum};
use crate::schema::*;

#[derive(Identifiable, Queryable, Insertable, AsChangeset, Clone, Debug, Display)]
#[display_from(Debug)]
#[table_name="state"]
pub(super) struct Stats {
    pub id: i16,
    pub started_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_block_hash: Vec<u8>,
    pub last_block_time: NaiveDateTime,
    pub known_height: i32,
    pub processed_height: i32,
    pub processed_txs: i64,
    pub processed_txins: i64,
    pub processed_txouts: i64,
    pub processed_blocks: i64,
    pub processed_volume: i64,
    pub processed_bytes: i64,
    pub processed_time: PgInterval,
    pub utxo_size: i32,
    pub utxo_volume: i64,
    pub utxo_bytes: i32,
    pub block_cache_size: i32,
    pub block_cache_bytes: i32,
}

impl Default for Stats {
    fn default() -> Self {
        let now = NaiveDateTime::from_timestamp(Utc::now().timestamp(), 0);
        Self {
            id: 0,
            started_at: now,
            updated_at: now,
            last_block_hash: vec![],
            last_block_time: NaiveDateTime::from_timestamp(0, 0),
            known_height: 0,
            processed_height: 0,
            processed_txs: 0,
            processed_txins: 0,
            processed_txouts: 0,
            processed_blocks: 0,
            processed_volume: 0,
            processed_bytes: 0,
            processed_time: PgInterval {
                microseconds: 0,
                days: 0,
                months: 0
            },
            utxo_size: 0,
            utxo_volume: 0,
            utxo_bytes: 0,
            block_cache_size: 0,
            block_cache_bytes: 0
        }
    }
}

#[derive(Queryable, Insertable)]
#[table_name="cached_block"]
pub struct CachedBlock {
    pub hash: Vec<u8>,
    pub prev_hash: Vec<u8>,
    pub block: Vec<u8>,
}

#[derive(Queryable, Insertable)]
#[table_name="utxo"]
pub struct Utxo {
    pub txid: Vec<u8>,
    pub block_height: i32,
    pub block_checksum: i16,
    pub tx_index: i16,
    pub output_index: i16,
}

impl From<Utxo> for short_id::Descriptor {
    fn from(utxo: Utxo) -> Self {
        short_id::Descriptor::OnchainTxOutput {
            block_height: utxo.block_height as u32,
            block_checksum: BlockChecksum::from_inner(utxo.block_checksum as u8),
            tx_index: utxo.tx_index as u16,
            output_index: utxo.output_index as u16
        }
    }
}
