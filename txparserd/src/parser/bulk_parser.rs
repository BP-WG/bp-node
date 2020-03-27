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

use std::collections::{HashMap, hash_map::Entry};
use diesel::{
    prelude::*,
    pg::PgConnection
};
use txlib::{
    schema,
    models,
    lnpbp::{
        bitcoin::{
            Txid, BlockHash, Block, Transaction, TxIn, TxOut,
            hashes::Hash,
            consensus::encode::deserialize
        },
        bp::short_id::{
            Descriptor, Dimension, BlockChecksum, TxChecksum
        },
        common::macros::*
    }
};
use crate::schema as state_schema;
use super::*;


pub struct BulkService {
    state_conn: PgConnection,
    index_conn: PgConnection,
    state: Stats,
    utxo: UtxoMap,
    block_cache: BlockMap,
}

impl BulkService {
    pub fn restore_or_create(state_conn: PgConnection, index_conn: PgConnection) -> Result<Self, Error> {
        let state = state_schema::state::dsl::state.find(0).first(&state_conn)?;
        let utxo = state_schema::utxo::dsl::utxo.load::<Utxo>(&state_conn)?
            .into_iter().try_fold::<_, _, Result<UtxoMap, Error>>(UtxoMap::new(), |mut map, utxo| {
                map.entry(Txid::from_slice(&utxo.txid[..]).map_err(|_| Error::IndexDbIntegrityError)?)
                    .or_insert_with(VoutMap::new)
                    .insert(utxo.output_index as u16, utxo.into());
                Ok(map)
            })?;
        let block_cache = state_schema::cached_block::dsl::cached_block.load::<CachedBlock>(&state_conn)?
            .into_iter().try_fold::<_, _, Result<BlockMap, Error>>(BlockMap::new(), |mut map, block| {
                map.insert(
                    BlockHash::from_slice(&block.hash[..]).map_err(|_| Error::IndexDbIntegrityError)?,
                    deserialize(&block.block[..]).map_err(|_| Error::IndexDbIntegrityError)?
                );
                Ok(map)
            })?;
        Ok(Self {
            state_conn,
            index_conn,
            state,
            utxo,
            block_cache
        })
    }

    pub fn init_from_scratch(state_conn: PgConnection, index_conn: PgConnection) -> Self {
        Self {
            state_conn,
            index_conn,
            state: Stats::default(),
            utxo: HashMap::new(),
            block_cache: HashMap::new()
        }
    }

    pub fn feed(&mut self, blocks: Vec<Block>) -> Result<(), Error> {
        // TODO: Ensure thread safety

        // TODO: Run though blocks and sort them into cached and processable

        // TODO: For processable blocks collect all state and data updates
        let block_chain = Vec::<Block>::with_capacity(blocks.len());
        let data = block_chain
            .into_iter()
            .try_fold::<_, _, Result<ParseData, Error>>(
                ParseData::init(self.state.clone(), &self.utxo),
                |mut data, block| {
                BlockParser::parse(&mut data, block)?;
                Ok(data)
            })?;

        self.state_conn.transaction(|| {
            self.index_conn.transaction(|| {
                let data = data.clone();
                diesel::insert_into(schema::block::table)
                    .values(data.blocks)
                    .execute(&self.index_conn)?;
                diesel::insert_into(schema::tx::table)
                    .values(data.txs)
                    .execute(&self.index_conn)?;
                diesel::insert_into(schema::txout::table)
                    .values(data.txouts)
                    .execute(&self.index_conn)?;
                diesel::insert_into(schema::txin::table)
                    .values(data.txins)
                    .execute(&self.index_conn)?;

                // TODO: Store state with UTXO and blocks cache
                diesel::update(state_schema::state::dsl::state.find(0))
                    .set(data.state)
                    .execute(&self.state_conn)
                    .map_err(|err| Error::StateDbError(err))
            })
        })?;

        self.state = data.state;
        // TODO: Update UTXO and blocks cache

        Ok(())
    }

    pub fn get_state(&self) -> Stats {
        // TODO: Ensure thread safety
        self.state.clone()
    }
}


