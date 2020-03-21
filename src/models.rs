use chrono::NaiveDateTime;
//use bitcoin::hash_types::{BlockHash, TxMerkleNode};

use super::schema::block;

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

