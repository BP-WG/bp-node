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


use clap::{Clap};

use lnpbp::bitcoin::{Block, Transaction, BlockHash, Txid, hashes::hex::FromHex};


const BITCOIN_DIR: &str = "/var/lib/bitcoin";

#[derive(Clap, Clone, Debug, Display)]
#[display_from(Debug)]
#[clap(
    name = "bp-indexer",
    version = "0.0.1",
    author = "Dr Maxim Orlovsky <orlovsky@pandoracore.com>",
    about =  "BP blockchain indexing utility; part of Bitcoin protocol node"
)]
pub struct Opts {
    /// Path and name of the configuration file
    #[clap(global = true, short = "c", long = "config", default_value = "./indexer.toml")]
    pub config: String,

    /// Sets verbosity level; can be used multiple times to increase verbosity
    #[clap(global = true, short = "v", long = "verbose", min_values = 0, max_values = 4, parse(from_occurrences))]
    pub verbose: u8,

    /// Connection string to index database
    #[clap(global = true, short = "i", long = "index-db", default_value = "postgresql://postgres:example@localhost:5432/bp")]
    pub index_db: String,

    /// Connection string to state storing database
    #[clap(global = true, short = "s", long = "state-db", default_value = "postgresql://postgres:example@localhost:5432/bp-indexer")]
    pub state_db: String,

    #[clap(subcommand)]
    pub command: Command
}

#[derive(Clap, Clone, Debug, Display)]
#[display_from(Debug)]
pub enum Command {
    /// Clear parsing status and index data
    ClearIndex,

    /// Reports on current Bitcoin blockchain parse status
    Status {
        /// Output formatting to use
        #[clap(short = "f", long = "formatting", default_value="pretty-print", arg_enum)]
        formatting: Formatting,
    },

    /// Sends command to a wired daemon to connect to the new peer
    IndexBlockchain {
        // TODO: Relace string with `PathBuf`; use #[clap(parse(from_os_str))]
        /// Bitcoin core data directory
        #[clap(short = "b", long = "bitcoin-dir", default_value = BITCOIN_DIR)]
        bitcoin_dir: String,

        /// Clears the existing index data and starts parsing from scratch.
        /// Works the same way as if `parso-blockchain` was following
        /// `clear-index` command.
        #[clap(long = "clear")]
        clear: Option<bool>,
    },

    /// Adds custom off-chain block to the index
    IndexBlock {
        /// Format of the provided data.
        #[clap(short = "f", long = "format",
               conflicts_with("block"), default_value = "auto", arg_enum)]
        format: DataFormat,

        // TODO: Move `parse_block_str` implementation into `bitcoin::Block::FromStr`
        /// Block data provided as a hex-encoded string. If absent, the data
        /// are read from STDIN (see --format option); in this case its format
        /// (binary or hex) is automatically guessed, unless --format option
        /// is explicitly provided
        #[clap(parse(try_from_str = crate::util::parse_block_str))]
        block: Option<Block>,
    },

    /// Adds custom off-chain transaction to the index
    IndexTransaction {
        /// Format of the provided data.
        #[clap(short = "f", long = "format",
               conflicts_with("block"), default_value = "auto", arg_enum)]
        format: DataFormat,

        // TODO: Move `parse_tx_str` implementation into `bitcoin::Transaction::FromStr`
        /// Transaction data provided as a hex-encoded string. If absent, the data
        /// are read from STDIN (see --format option); in this case its format
        /// (binary or hex) is automatically guessed, unless --format option
        /// is explicitly provided
        #[clap(parse(try_from_str = crate::util::parse_tx_str))]
        block: Option<Transaction>,
    },

    /// Removes off-chain block with the given Id from the index
    RemoveBlock {
        /// Block hash (block id) to remove from database. If matches on-chain
        /// block the parameter is ignored and program fails.
        #[clap(parse(try_from_str = ::lnpbp::bitcoin::BlockHash::from_hex))]
        block_hash: BlockHash,
    },

    /// Removes off-chain transaction with the given Id from the index
    RemoveTransaction {
        /// Transaction id to remove from database. If matches on-chain
        /// transaction the parameter is ignored and program fails.
        #[clap(parse(try_from_str = ::lnpbp::bitcoin::Txid::from_hex))]
        txid: Txid,
    },
}

#[derive(Clap, Clone, Debug)]
pub enum Formatting {
    PrettyPrint,
    Json,
    Yaml,
    Xml,
    AwkFriendly,
}

#[derive(Clap, Clone, Debug)]
pub enum DataFormat {
    Auto,
    Binary,
    Hex,
}

// We need config structure since not all of the parameters can be specified
// via environment and command-line arguments. Thus we need a config file and
// default set of configuration
#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub struct Config {
    pub verbose: u8,
    pub bitcoin_dir: String,
    pub index_db: String,
    pub state_db: String,
}

impl From<Opts> for Config {
    fn from(opts: Opts) -> Self {
        let mut me = Self {
            verbose: opts.verbose,
            index_db: opts.index_db,
            state_db: opts.state_db,
            ..Config::default()
        };
        if let Command::IndexBlockchain { bitcoin_dir, .. } = opts.command {
            me.bitcoin_dir = bitcoin_dir;
        }
        me
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            verbose: 1,
            bitcoin_dir: BITCOIN_DIR.to_string(),
            index_db: "".to_string(),
            state_db: "".to_string()
        }
    }
}
