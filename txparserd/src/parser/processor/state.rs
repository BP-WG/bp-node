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


use std::{
    fmt,
    ops::AddAssign
};
use chrono::{Utc, NaiveDateTime};
use diesel::{
    prelude::*,
    PgConnection,
    result::Error as DieselError,
    pg::types::date_and_time::PgInterval
};
use txlib::lnpbp::bitcoin::{
    Block, BlockHash, Txid, hashes::Hash, consensus::deserialize
};
use super::*;
use txlib::lnpbp::miniscript::bitcoin::OutPoint;
use txlib::lnpbp::miniscript::bitcoin::hashes::core::fmt::Formatter;

#[derive(Clone, Debug, Default)]
pub(super) struct State {
    pub utxo: UtxoMap,
    pub spent: Vec<(Txid, u16)>,
    pub block_cache: BlockMap,
    pub block_cache_removal: Vec<BlockHash>,

    pub last_block_hash: Option<BlockHash>,
    pub last_block_time: Option<u32>,
    pub known_height: u32,
    pub processed_height: u32,
    pub processed_txs: u64,
    pub processed_txins: u64,
    pub processed_txouts: u64,
    pub processed_blocks: u64,
    pub processed_volume: u64,
    pub processed_bytes: u64,
    pub processed_time: u64,
    pub utxo_size: u32,
    pub utxo_volume: u64,
    pub utxo_bytes: u32,
    pub block_cache_size: u32,
    pub block_cache_bytes: u32,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let utxo_count = self.utxo.map_size();
        let spent_count = self.spent.len();
        let bc_count = self.block_cache.len();
        let bcr_count = self.block_cache_removal.len();

        writeln!(f, "")?;
        writeln!(f, "Known height: {} | Processed height: {} | Last block hash: {:x}",
                 self.known_height,
                 self.processed_height,
                 self.last_block_hash.unwrap_or(BlockHash::default()))?;
        writeln!(f, "")?;
        writeln!(f, "{:<10}  {:^10} | {:^10} | {:^10} | {:^10}", "", "UTXO", "sUTXO", "BLCK_CACHE", "-BLCK_CACHE")?;
        writeln!(f, "{:<10}: {:>10} | {:>10} | {:>10} | {:>10}", "Actuals", utxo_count, spent_count, bc_count, bcr_count)?;
        writeln!(f, "{:<10}: {:>10} | {:>10} | {:>10} | {:>10}", "Statistics", self.utxo_size, "-", self.block_cache_size, "-")?;
        writeln!(f, "")?;
        writeln!(f, "{:<10}  {:^10} | {:^10} | {:^10} | {:^10} | {:^10}", "", "Block", "Tx", "TxIn", "TxOut", "Value")?;
        writeln!(f, "{:<10}: {:>10} | {:>10} | {:>10} | {:>10}", "Statistics",
                self.processed_blocks, self.processed_txs, self.processed_txins, self.processed_txouts)
    }
}

impl From<State> for model::State {
    fn from(state: State) -> Self {
        // TODO: Fix id and started_at
        Self {
            id: 0,
            started_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            last_block_hash: state.last_block_hash.unwrap_or(BlockHash::default()).to_vec(),
            last_block_time: NaiveDateTime::from_timestamp(state.last_block_time.unwrap_or(0) as i64, 0),
            known_height: state.known_height as i32,
            processed_height: state.processed_height as i32,
            processed_txs: state.processed_txs as i64,
            processed_txins: state.processed_txins as i64,
            processed_txouts: state.processed_txouts as i64,
            processed_blocks: state.processed_blocks as i64,
            processed_volume: state.processed_volume as i64,
            processed_bytes: state.processed_bytes as i64,
            processed_time: PgInterval::from_microseconds(state.processed_time as i64),
            utxo_size: state.utxo_size as i32,
            utxo_volume: state.utxo_volume as i64,
            utxo_bytes: state.utxo_bytes as i32,
            block_cache_size: state.block_cache_size as i32,
            block_cache_bytes: state.block_cache_size as i32,
        }
    }
}

impl State {
    pub(super) fn inherit_state(state: &State) -> Self {
        let mut me = Self::default();
        me.known_height = state.known_height;
        me.processed_height = state.processed_height;
        me.last_block_hash = state.last_block_hash;
        me.last_block_time = state.last_block_time;
        me
    }

    pub(super) fn restore(state_conn: &PgConnection, index_conn: &PgConnection) -> Result<Self, Error> {
        let state_model = state_table.find(0)
            .first(state_conn)
            .or::<DieselError>(Ok(model::State::default()))?;

        let utxo = utxo_table.load::<Utxo>(state_conn)
            .or::<DieselError>(Ok(Vec::new()))?
            .into_iter()
            .try_fold(UtxoMap::new(), |mut map, utxo| -> Result<UtxoMap, Error> {
                map.entry(Txid::from_slice(&utxo.txid[..]).map_err(|_| Error::IndexDBIntegrityError)?)
                    .or_insert_with(VoutMap::new)
                    .insert(utxo.output_index as u16, utxo.into());
                Ok(map)
            })?;

        let block_cache = cache_table.load::<CachedBlock>(state_conn)
            .or::<DieselError>(Ok(Vec::new()))?
            .into_iter()
            .try_fold(BlockMap::new(), |mut map, block| -> Result<BlockMap, Error> {
                map.insert(
                    BlockHash::from_slice(&block.hash[..]).map_err(|_| Error::IndexDBIntegrityError)?,
                    deserialize(&block.block[..]).map_err(|_| Error::IndexDBIntegrityError)?
                );
                Ok(map)
            })?;

        Ok(Self {
            utxo,
            spent: vec![],
            block_cache,
            block_cache_removal: vec![],
            last_block_hash: Some(BlockHash::from_slice(&state_model.last_block_hash[..])?),
            last_block_time: Some(state_model.last_block_time.timestamp() as u32),
            known_height:  state_model.known_height as u32,
            processed_height:  state_model.processed_height as u32,
            processed_txs:  state_model.processed_txs as u64,
            processed_txins:  state_model.processed_txins as u64,
            processed_txouts:  state_model.processed_txouts as u64,
            processed_blocks:  state_model.processed_blocks as u64,
            processed_volume:  state_model.processed_volume as u64,
            processed_bytes:  state_model.processed_bytes as u64,
            processed_time:  state_model.processed_time.microseconds as u64,
            utxo_size:  state_model.utxo_size as u32,
            utxo_volume:  state_model.utxo_volume as u64,
            utxo_bytes:  state_model.utxo_bytes as u32,
            block_cache_size:  state_model.block_cache_size as u32,
            block_cache_bytes:  state_model.block_cache_bytes as u32,
        })
    }

    pub(super) fn store(&mut self, state_conn: &PgConnection, index_conn: &PgConnection) -> Result<(), Error> {
        // Not doing as a transaction: atomicity is the job of `BulkParser`
        // also, saving multiple times does not damage state data

        // Updating state data
        diesel::update(state_table
            .find(0))
            .set(model::State::from(self.clone()))
            .execute(state_conn)
            .map_err(|err| Error::StateDBError(err))?;

        // Removing consumed blocks
        /*
        let removal_list = self.block_cache_removal_db.clone();
        self.block_cache_removal_db = vec![];
        diesel::delete(cache_table)
            .find(block_cache_removal_db)
            .execute(&self.state_conn)
            .map_err(|err| Error::StateDBError(err))?;
            */
        diesel::delete(cache_table)
            .execute(state_conn)
            .map_err(|err| Error::StateDBError(err))?;

        // Removing spent UTXOs
        diesel::delete(utxo_table)
            .execute(state_conn)
            .map_err(|err| Error::StateDBError(err))?;

        // Saving block cache
        let values: Vec<model::CachedBlock> = self.block_cache
            .iter()
            .map(|(_, block)| model::CachedBlock::from(block.clone()))
            .collect();
        diesel::insert_into(cache_table)
            .values(values)
            .execute(state_conn)
            .map_err(|err| Error::StateDBError(err))?;

        // Saving UTXOs
        let values: Vec<model::Utxo> = self.utxo
            .iter()
            .flat_map(|(txid, vout_map)| -> Vec<model::Utxo> {
                vout_map.iter().map(|(vout, descriptor)| -> model::Utxo {
                    model::Utxo {
                        txid: txid.to_vec(),
                        block_height: descriptor.get_block_height()
                            .expect("Runtime error 5: parser-generated descriptor fails conversion") as i32,
                        block_checksum: descriptor.get_block_checksum()
                            .expect("Runtime error 6: parser-generated descriptor fails conversion") as i16,
                        tx_index: descriptor.get_tx_index()
                            .expect("Runtime error 6: parser-generated descriptor fails conversion") as i16,
                        output_index: *vout as i16
                    }
                })
                .collect()
            })
            .collect();
        diesel::insert_into(utxo_table)
            .values(values)
            .execute(state_conn)
            .map_err(|err| Error::StateDBError(err))?;

        Ok(())
    }
}

impl AddAssign for State {
    fn add_assign(&mut self, rhs: Self) {
        rhs.spent.into_iter().for_each(|(txid, vout)| {
            self.utxo.extract_descriptor(&OutPoint { txid, vout: vout as u32 });
        });

        rhs.utxo.into_iter().for_each(|(txid, vout_map)| {
            let entry = self.utxo.entry(txid).or_insert(VoutMap::new());
            // Can't do `extend` here; need to analyse UTXO consistency
            vout_map.into_iter().for_each(|(vout, descriptor)| {
                if entry.contains_key(&vout) {
                    error!("Duplicate UTXO entry found while merging UTXO set with new block data; \
                           this should not happen. UTXO: {}:{}, Descriptor: {}",
                           txid, vout, descriptor);
                }
                entry.insert(vout, descriptor);
            });
        });

        self.block_cache.extend(rhs.block_cache);
        rhs.block_cache_removal.into_iter().for_each(|txid| {
            self.block_cache.remove(&txid);
        });
        self.block_cache_removal = Vec::new();

        self.last_block_hash = rhs.last_block_hash;
        self.last_block_time = rhs.last_block_time;
        self.known_height = rhs.known_height;
        self.processed_height = rhs.processed_height;

        self.processed_txs += rhs.processed_txs;
        self.processed_txins += rhs.processed_txins;
        self.processed_txouts += rhs.processed_txouts;
        self.processed_blocks += rhs.processed_blocks;
        self.processed_volume += rhs.processed_volume;
        self.processed_bytes += rhs.processed_bytes;
        self.processed_time += rhs.processed_time;
        self.utxo_size += rhs.utxo_size;
        self.utxo_volume += rhs.utxo_volume;
        self.utxo_bytes += rhs.utxo_bytes;
        self.block_cache_size += rhs.block_cache_size;
        self.block_cache_bytes += rhs.block_cache_bytes;
    }
}

impl State {
    pub(super) fn order_blocks(&mut self, blocks: Vec<Block>, base_state: &Self) -> Vec<Block> {
        // TODO: Update state for block cache parameters

        trace!("Ordering blocks into chain; adding the rest to cache");
        let mut prev_block_hash = base_state.last_block_hash.unwrap_or(BlockHash::default());
        let mut prev_block_time = base_state.last_block_time.unwrap_or(0);
        let mut block_height = base_state.known_height;
        let mut blockchain = Vec::<Block>::with_capacity(blocks.len());
        blocks.into_iter().for_each(|block | {
            if block_height != 0 && block.header.prev_blockhash != prev_block_hash {
                trace!("Block out of order {}, must follow {}. Cache size {}",
                       block.block_hash(), block.header.prev_blockhash,
                       base_state.block_cache.len() + self.block_cache.len() + 1);
                self.block_cache.insert(block.header.prev_blockhash, block.clone());
                match base_state.block_cache.get(&prev_block_hash) {
                    Some(b) => {
                        blockchain.push(b.clone());
                        prev_block_hash = block.block_hash();
                        prev_block_time = block.header.time;
                        self.block_cache_removal.push(prev_block_hash);
                    }
                    None => match self.block_cache.remove(&prev_block_hash) {
                        Some(b) => {
                            blockchain.push(b);
                            prev_block_hash = block.block_hash();
                            prev_block_time = block.header.time;
                        },
                        None => block_height += 1,
                    }
                }
            } else {
                prev_block_hash = block.block_hash();
                prev_block_time = block.header.time;
                blockchain.push(block);
            }
        });
        self.last_block_hash = Some(prev_block_hash);
        self.last_block_time = Some(prev_block_time);
        self.known_height = block_height;
        blockchain
    }
}