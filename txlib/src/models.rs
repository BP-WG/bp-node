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

use chrono::NaiveDateTime;

use super::schema::*;

#[derive(Identifiable, Queryable, Insertable)]
#[table_name="block"]
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

#[derive(Identifiable, Queryable, Insertable)]
#[table_name="tx"]
pub struct Tx {
    pub id: i64,
    pub ver: i32,
    pub locktime: i32,
    pub out_count: i16,
    pub in_count: i16,
    pub fee: Option<i64>
}

#[derive(Identifiable, Queryable, Insertable)]
#[table_name="txin"]
pub struct Txin {
    pub id: i64,
    pub seq: i32,
    pub txout_id: i64,
}

#[derive(Identifiable, Queryable, Insertable)]
#[table_name="txout"]
pub struct Txout {
    pub id: i64,
    pub amount: i64,
    pub script: Vec<u8>
}
