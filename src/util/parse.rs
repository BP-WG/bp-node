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

use bitcoin::hashes::hex::FromHex;
use bitcoin::{consensus, Block, Transaction};

// TODO: Move `parse_block_str` implementation into `bitcoin::Block::FromStr`
pub fn parse_block_str(data: &str) -> Result<Block, consensus::encode::Error> {
    // TODO: Fix `consensus::encode::Error::ParseFailed` `&str` type to String
    let vec = Vec::from_hex(data)
        .map_err(|err| consensus::encode::Error::ParseFailed("Not a hexadecimal string"))?;
    consensus::deserialize(&vec)
}

// TODO: Move `parse_tx_str` implementation into `bitcoin::Transaction::FromStr`
pub fn parse_tx_str(data: &str) -> Result<Transaction, consensus::encode::Error> {
    let vec = Vec::from_hex(data)
        .map_err(|err| consensus::encode::Error::ParseFailed("Not a hexadecimal string"))?;
    consensus::deserialize(&vec)
}
