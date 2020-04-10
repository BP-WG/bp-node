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


use lnpbp::bitcoin::{self, Block, Transaction};
use lnpbp::bitcoin::hashes::hex::FromHex;

// TODO: Move `parse_block_str` implementation into `bitcoin::Block::FromStr`
pub fn parse_block_str(data: &str) -> Result<Block, bitcoin::consensus::encode::Error> {
    // TODO: Fix `itcoin::consensus::encode::Error::ParseFailed` `&str` type to String
    let vec = Vec::from_hex(data)
        .map_err(|err| bitcoin::consensus::encode::Error::ParseFailed("Not a hexadecimal string"))?;
    bitcoin::consensus::deserialize(&vec)
}


// TODO: Move `parse_tx_str` implementation into `bitcoin::Transaction::FromStr`
pub fn parse_tx_str(data: &str) -> Result<Transaction, bitcoin::consensus::encode::Error> {
    let vec = Vec::from_hex(data)
        .map_err(|err| bitcoin::consensus::encode::Error::ParseFailed("Not a hexadecimal string"))?;
    bitcoin::consensus::deserialize(&vec)
}
