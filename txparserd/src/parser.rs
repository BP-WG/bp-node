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

use std::collections::HashMap;
use diesel::{
    prelude::*,
    pg::PgConnection,
    result::Error as DbError
};
use txlib::{
    schema,
    models,
    lnpbp::{
        bitcoin::{Txid, BlockHash, Block, Transaction, TxIn, TxOut},
        bp::short_id::{
            Descriptor, Dimension, BlockChecksum, TxChecksum
        },
        common::macros::*
    }
};
use crate::{
    state::*,
    schema as state_schema,
};
use txlib::lnpbp::miniscript::bitcoin::hashes::sha1::Hash;

#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum Error {
    IndexDbError(DbError),
    StateDbError(DbError)
}

impl From<DbError> for Error {
    fn from(err: DbError) -> Self {
        Error::IndexDbError(err)
    }
}


#[derive(Debug, Display)]
#[display_from(Debug)]
pub(self) struct ParseData {
    pub state: State,
    pub utxo: HashMap<Txid, HashMap<u16, Descriptor>>,
    pub blocks: Vec<models::Block>,
    pub txs: Vec<models::Tx>,
    pub txins: Vec<models::Txin>,
    pub txouts: Vec<models::Txout>,
}

impl ParseData {
    pub(self) fn init(state: State) -> Self {
        Self {
            state,
            utxo: HashMap::new(),
            blocks: vec![],
            txs: vec![],
            txins: vec![],
            txouts: vec![]
        }
    }
}


#[derive(Debug, Display)]
#[display_from(Debug)]
pub struct Parser {
    state_conn: PgConnection,
    index_conn: PgConnection,
    state: State,
    utxo: HashMap<Txid, HashMap<u16, Descriptor>>,
    block_cache: HashMap<BlockHash, Block>,
}

impl Parser {
    pub fn restore_or_create(state_conn: PgConnection, index_conn: PgConnection) -> Self {
        let state = state_schema::state::dsl::state.find(0).first(&state_conn);
        let utxo = state_schema::utxo::dsl::utxo.load(&state_conn);
        let block_cache = state_schema::cached_block::dsl::cached_block.load(&state_conn);
        Self {
            state_conn,
            index_conn,
            state,
            utxo,
            block_cache
        }
    }

    pub fn init_from_scratch(state_conn: PgConnection, index_conn: PgConnection) -> Self {
        Self {
            state_conn,
            index_conn,
            state: State::default(),
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
            .try_fold(ParseData::init(self.state.copy()), |mut data, block| {
                BlockParser::parse(&mut data, block)?;
            })?;

        self.state_conn.transaction(|| {
            self.index_conn.transaction(|| {
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
                    .set(*data.state)
                    .execute(&self.state_conn)
                    .map_err(|err| Error::StateDbError(err))
            })
        })?;

        self.state = data.state;
        // TODO: Update UTXO and blocks cache

        Ok(())
    }

    pub fn get_state(&self) -> State {
        // TODO: Ensure thread safety
        self.state.clone()
    }
}


#[derive(Debug, Display)]
#[display_from(Debug)]
pub(self) struct BlockParser<'a> {
    descriptor: Descriptor,
    result: &'a mut ParseData,
}

impl BlockParser<'_> {
    pub(self) fn parse(data: &mut ParseData, block: Block) -> Result<(), Error> {
        let block_checksum = BlockChecksum::from(block.block_hash());
        let mut parser = Self {
            descriptor: Descriptor::OnchainBlock {
                block_height: data.state.known_height as u32,
                block_checksum
            },
            result: data,
        };
        parser.parse_block(&block)?;
        Ok(())
    }
}

impl BlockParser<'_> {
    fn parse_block(&mut self, block: &Block) -> Result<(), Error> {
        self.descriptor = Descriptor::OnchainBlock {
            block_height: self.result.state.known_height as u32,
            block_checksum: BlockChecksum::from(block.block_hash())
        };

        block.txdata.iter().enumerate().try_for_each(self.parse_tx)?;

        self.result.blocks.push(block.into());

        self.result.state.known_height += 1;
        // TODO: Update the rest of state

        Ok(())
    }

    fn parse_tx(&mut self, data: (usize, &Transaction)) -> Result<(), Error> {
        let (index, tx) = data;
        let descriptor = self.descriptor.clone();

        self.descriptor
            .upgrade(index as u16, None)
            .expect("Descriptor upgrade for an onchain block does not fail");

        tx.output.iter().enumerate().try_for_each(self.parse_txout)?;
        tx.input.iter().enumerate().try_for_each(self.parse_txin)?;

        self.result.txs.push(tx.into());

        self.descriptor = descriptor;

        // TODO: Update state

        Ok(())
    }

    fn parse_txin(&mut self, data: (usize, &TxIn)) -> Result<(), Error> {
        let (index, txin) = data;
        let descriptor = self.descriptor.clone();

        self.descriptor
            .upgrade(index as u16, Some(Dimension::Input))
            .expect("Descriptor upgrade for an onchain transaction does not fail");

        self.result.txins.push(txin.into());

        self.descriptor = descriptor;

        // TODO: Update state

        Ok(())
    }

    fn parse_txout(&mut self, data: (usize, &TxOut)) -> Result<(), Error> {
        let (index, txout) = data;
        let descriptor = self.descriptor.clone();

        self.descriptor
            .upgrade(index as u16, Some(Dimension::Output))
            .expect("Descriptor upgrade for an onchain transaction does not fail");

        self.result.txouts.push(txout.into());

        self.descriptor = descriptor;

        // TODO: Update state

        Ok(())
    }
}
