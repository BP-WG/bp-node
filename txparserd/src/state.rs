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

use chrono::NaiveDateTime;
use diesel::sql_types::Interval;
use diesel::pg::data_types::PgInterval;
use super::schema::*;

#[derive(Identifiable, Queryable, Insertable)]
#[table_name="state"]
pub struct State {
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
