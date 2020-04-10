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

use std::{
    fmt,
    collections::{
        HashMap, hash_map::Entry
    }
};
use lnpbp::{
    bitcoin::{Txid, BlockHash, Block, OutPoint},
    bp::short_id::Descriptor
};

use crate::db::models;
use super::state::State;

pub type VoutMap = HashMap<u16, Descriptor>;
pub type UtxoMap = HashMap<Txid, VoutMap>;
pub type BlockMap = HashMap<BlockHash, Block>;

pub trait UtxoAccess {
    fn get_descriptor(&self, outpoint: &OutPoint) -> Option<&Descriptor>;
    fn extract_descriptor(&mut self, outpoint: &OutPoint) -> Option<Descriptor>;
    fn remove_utxo(&mut self, outpoint: &OutPoint) -> bool;
    fn map_size(&self) -> usize;
}

impl UtxoAccess for UtxoMap {
    fn get_descriptor(&self, outpoint: &OutPoint) -> Option<&Descriptor> {
        self.get(&outpoint.txid)
            .and_then(|vout_map| vout_map.get(&(outpoint.vout as u16)))
    }

    fn extract_descriptor(&mut self, outpoint: &OutPoint) -> Option<Descriptor> {
        self.get_descriptor(outpoint)
            .map(|d| *d)
            .and_then(|d| {
                self.remove_utxo(outpoint);
                Some(d)
            })
    }

    fn remove_utxo(&mut self, outpoint: &OutPoint) -> bool {
        match self.entry(outpoint.txid) {
            Entry::Vacant(_) => false,
            Entry::Occupied(mut entry) => match entry.get_mut().entry(outpoint.vout as u16) {
                Entry::Vacant(_) => false,
                Entry::Occupied(entry) => {
                    entry.remove();
                    true
                }
            },
        }
    }

    fn map_size(&self) -> usize {
        self.iter().fold(0, |acc, (_, vmap)| {
            acc + vmap.len()
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct ParseData {
    pub state: State,
    pub spent: Vec<OutPoint>,
    pub blocks: Vec<models::Block>,
    pub txs: Vec<models::Tx>,
    pub txins: Vec<models::Txin>,
    pub txouts: Vec<models::Txout>,
}

impl ParseData {
    pub(super) fn init(state: State) -> Self {
        Self {
            state,
            spent: vec![],
            blocks: vec![],
            txs: vec![],
            txins: vec![],
            txouts: vec![]
        }
    }
}

impl fmt::Display for ParseData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.state)?;
        writeln!(f, "{:<10}: {:>10} | {:>10} | {:>10} | {:>10}", "Actuals",
                 self.blocks.len(), self.txs.len(), self.txins.len(), self.txouts.len())?;
        writeln!(f, "")
    }
}
