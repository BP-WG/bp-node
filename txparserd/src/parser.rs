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
    pg::PgConnection,
    result::Error as DbError
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
use crate::{
    state::*,
    schema as state_schema,
};

#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum Error {
    IndexDbIntegrityError,
    BlockchainIndexesOutOfShortIdRanges,
    BlockValidationIncosistency,
    IndexDbError(DbError),
    StateDbError(DbError)
}

impl From<DbError> for Error {
    fn from(err: DbError) -> Self {
        Error::IndexDbError(err)
    }
}


type VoutMap = HashMap<u16, Descriptor>;
type UtxoMap = HashMap<Txid, VoutMap>;
type BlockMap = HashMap<BlockHash, Block>;

#[derive(Clone, Debug, Display)]
#[display_from(Debug)]
pub(self) struct ParseData {
    pub state: State,
    pub utxo: UtxoMap,
    pub blocks: Vec<models::Block>,
    pub txs: Vec<models::Tx>,
    pub txins: Vec<models::Txin>,
    pub txouts: Vec<models::Txout>,
}

impl ParseData {
    pub(self) fn init(state: State, utxo: &UtxoMap) -> Self {
        Self {
            state,
            utxo: utxo.clone(),
            blocks: vec![],
            txs: vec![],
            txins: vec![],
            txouts: vec![]
        }
    }
}


pub struct Parser {
    state_conn: PgConnection,
    index_conn: PgConnection,
    state: State,
    utxo: UtxoMap,
    block_cache: BlockMap,
}

impl Parser {
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

    pub fn get_state(&self) -> State {
        // TODO: Ensure thread safety
        self.state.clone()
    }
}


#[derive(Debug, Display)]
#[display_from(Debug)]
pub(self) struct BlockParser<'a> {
    coinbase_amount: Option<u64>,
    descriptor: Descriptor,
    result: &'a mut ParseData,
}

impl<'a> BlockParser<'a> {
    pub(self) fn parse(data: &'a mut ParseData, block: Block) -> Result<(), Error> {
        let block_checksum = BlockChecksum::from(block.block_hash());
        let mut parser = Self {
            coinbase_amount: None,
            descriptor: Descriptor::OnchainBlock {
                block_height: data.state.known_height as u32,
                block_checksum
            },
            result: data,
        };
        parser.parse_block(&block)
    }
}

impl BlockParser<'_> {
    fn parse_block(&mut self, block: &Block) -> Result<(), Error> {
        self.descriptor = Descriptor::OnchainBlock {
            block_height: self.result.state.known_height as u32,
            block_checksum: BlockChecksum::from(block.block_hash())
        };

        block.txdata.iter().enumerate().try_for_each(|(index, tx)| self.parse_tx(index, tx))?;

        self.result.blocks
            .push(txlib::models::Block::compose(block, self.descriptor)
                .map_err(|_| Error::BlockchainIndexesOutOfShortIdRanges)?);

        self.result.state.known_height += 1;
        // TODO: Update the rest of state

        Ok(())
    }

    fn parse_tx(&mut self, index: usize, tx: &Transaction) -> Result<(), Error> {
        let descriptor = self.descriptor.clone();

        self.descriptor
            .upgraded(index as u16, None)
            .expect("Descriptor upgrade for an onchain block does not fail");

        self.coinbase_amount = if tx.is_coin_base() {
            Some(tx.output[0].value)
        } else {
            None
        };

        let txid = tx.txid();
        tx.output.iter().enumerate().try_for_each(|(index, txout)| self.parse_txout(index, txid, txout))?;
        tx.input.iter().enumerate().try_for_each(|(index, txin)| self.parse_txin(index, txin))?;

        self.result.txs.push(txlib::models::Tx::compose(tx, self.descriptor)
            .map_err(|_| Error::BlockchainIndexesOutOfShortIdRanges)?);

        self.descriptor = descriptor;

        // TODO: Update state

        Ok(())
    }

    fn parse_txin(&mut self, index: usize, txin: &TxIn) -> Result<(), Error> {
        let descriptor = self.descriptor.clone();
        let block_descriptor = descriptor.downgraded()
            .expect("Transaction to block descriptor downgrade can't fail");

        let txo_descriptor = if let Some(coinbase_amount) = self.coinbase_amount {
            self.result.txouts.push(models::Txout {
                id: block_descriptor.try_into_u64()
                    .expect("Block descriptor is generated from other already used descriptor, so can't fail")
                    as i64,
                amount: coinbase_amount as i64,
                script: vec![]
            });
            block_descriptor
        } else {
            let mut txoset = self.result.utxo.get_mut(&txin.previous_output.txid)
                .ok_or(Error::BlockValidationIncosistency)?;
            let prev_vout: u16 = txin.previous_output.vout as u16;
            let txo_descriptor = txoset.remove(&prev_vout)
                .ok_or(Error::BlockValidationIncosistency)?;
            if txoset.is_empty() {
                self.result.utxo.remove(&txin.previous_output.txid);
            }
            // TODO: Update state
            txo_descriptor
        };

        self.descriptor
            .upgraded(index as u16, Some(Dimension::Input))
            .expect("Descriptor upgrade for an onchain transaction does not fail");

        self.result.txins.push(txlib::models::Txin::compose(txin, self.descriptor, txo_descriptor)
            .map_err(|_| Error::BlockchainIndexesOutOfShortIdRanges)?);

        self.descriptor = descriptor;

        // TODO: Update state

        Ok(())
    }

    fn parse_txout(&mut self, index: usize, txid: Txid, txout: &TxOut) -> Result<(), Error> {
        let descriptor = self.descriptor.clone();

        self.descriptor
            .upgraded(index as u16, Some(Dimension::Output))
            .expect("Descriptor upgrade for an onchain transaction does not fail");

        let mut txoset = match self.result.utxo.entry(txid) {
            Entry::Vacant(entry) => entry.insert(HashMap::new()),
            Entry::Occupied(entry) => entry.into_mut(),
        };
        txoset.insert(index as u16, self.descriptor);

        self.result.txouts.push(txlib::models::Txout::compose(txout, self.descriptor)
            .map_err(|_| Error::BlockchainIndexesOutOfShortIdRanges)?);

        self.descriptor = descriptor;

        // TODO: Update state

        Ok(())
    }
}
