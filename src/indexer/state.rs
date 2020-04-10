// Bitcoin protocol (BP) daemon node
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


use diesel::{
    prelude::*,
    PgConnection,
    result::Error as DieselError,
};
use lnpbp::bitcoin::{
    BlockHash, Txid, hashes::Hash, consensus::deserialize
};

use super::Error;
use super::db::model::{self, *};
use crate::parser::{State as ParserState, data::*, BulkParser};


impl BulkParser {
    pub fn restore(state_conn: PgConnection, index_conn: PgConnection) -> Result<Self, Error> {
        Ok(Self {
            state: ParserState::restore(&state_conn, &index_conn)?,
            state_conn,
            index_conn,
        })
    }
}

impl ParserState {
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