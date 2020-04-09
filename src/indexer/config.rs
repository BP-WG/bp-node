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


use clap::Clap;


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
}


// We need config structure since not all of the parameters can be specified
// via environment and command-line arguments. Thus we need a config file and
// default set of configuration
#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub struct Config {
    pub verbose: u8,
}

impl From<Opts> for Config {
    fn from(opts: Opts) -> Self {
        Self {
            verbose: opts.verbose,
            ..Config::default()
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            verbose: 0,
        }
    }
}