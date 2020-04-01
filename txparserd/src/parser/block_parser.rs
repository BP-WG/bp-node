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
use super::{*, error::Error};

#[derive(Debug, Display)]
#[display_from(Debug)]
pub(super) struct BlockParser<'a> {
    coinbase_amount: Option<u64>,
    descriptor: Descriptor,
    result: &'a mut ParseData,
}

impl<'a> BlockParser<'a> {
    pub(super) fn parse(data: &'a mut ParseData, block: Block) -> Result<(), Error> {
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
        debug!("Processing block {}", block.block_hash());

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
