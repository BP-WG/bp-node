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
    ops::AddAssign
};
use bitcoin::{Block, BlockHash, Txid, OutPoint};

use super::*;


#[derive(Clone, Debug, Default)]
pub struct State {
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl State {
    pub(super) fn inherit_state(state: &State) -> Self {
        let mut me = Self::default();
        me.known_height = state.known_height;
        me.processed_height = state.processed_height;
        me.last_block_hash = state.last_block_hash;
        me.last_block_time = state.last_block_time;
        me
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
                           this should not happen. UTXO: {}:{}, Descriptor: {:?}",
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