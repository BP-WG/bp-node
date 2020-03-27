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
use txlib::{
    models,
    lnpbp::{
        bitcoin::{Txid, BlockHash, Block},
        bp::short_id::Descriptor
    },
};
use super::*;

pub(super) type VoutMap = HashMap<u16, Descriptor>;
pub(super) type UtxoMap = HashMap<Txid, VoutMap>;
pub(super) type BlockMap = HashMap<BlockHash, Block>;

#[derive(Clone, Debug, Display)]
#[display_from(Debug)]
pub(super) struct ParseData {
    pub state: Stats,
    pub utxo: UtxoMap,
    pub blocks: Vec<models::Block>,
    pub txs: Vec<models::Tx>,
    pub txins: Vec<models::Txin>,
    pub txouts: Vec<models::Txout>,
}

impl ParseData {
    pub(super) fn init(state: Stats, utxo: &UtxoMap) -> Self {
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
