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

use clap::Parser;

use crate::msgbus::constants::*;

#[derive(Parser, Clone, Debug, Display)]
#[display_from(Debug)]
#[clap(
    name = "bp-cli",
    version = "0.0.1",
    author = "Dr Maxim Orlovsky <orlovsky@pandoracore.com>",
    about = "BP node command-line interface; part of Bitcoin protocol node"
)]
pub struct Opts {
    /// Path and name of the configuration file
    #[clap(
        global = true,
        short = 'c',
        long = "config",
        default_value = "./cli.toml"
    )]
    pub config: String,

    /// Sets verbosity level; can be used multiple times to increase verbosity
    #[clap(
        global = true,
        short = 'v',
        long = "verbose",
        min_values = 0,
        max_values = 4,
        parse(from_occurrences)
    )]
    pub verbose: u8,

    /// IPC connection string for queryd daemon API
    #[clap(global = true, short = 'w', long = "queryd-api", default_value = MSGBUS_PEER_API_ADDR, env="BP_CLI_QUERYD_API_ADDR")]
    pub queryd_api_socket_str: String,

    /// IPC connection string for queryd daemon push notifications on perr status updates
    #[clap(global = true, short = 'W', long = "queryd-push", default_value = MSGBUS_PEER_PUSH_ADDR, env="BP_CLI_QUERYD_PUSH_ADDR")]
    pub queryd_push_socket_str: String,

    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Parser, Clone, Debug, Display)]
#[display_from(Debug)]
pub enum Command {
    /// Sends command to a wired daemon to connect to the new peer
    Query {
        /// Query to run against Bitcoin blockchain & transaction index
        query: String,
    },
}

// We need config structure since not all of the parameters can be specified
// via environment and command-line arguments. Thus we need a config file and
// default set of configuration
#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub struct Config {
    pub verbose: u8,
    pub msgbus_peer_api_addr: String,
    pub msgbus_peer_sub_addr: String,
}

impl From<Opts> for Config {
    fn from(opts: Opts) -> Self {
        Self {
            verbose: opts.verbose,
            msgbus_peer_api_addr: opts.queryd_api_socket_str,
            msgbus_peer_sub_addr: opts.queryd_push_socket_str,

            ..Config::default()
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            verbose: 0,
            msgbus_peer_api_addr: MSGBUS_PEER_API_ADDR.to_string(),
            msgbus_peer_sub_addr: MSGBUS_PEER_PUSH_ADDR.to_string(),
        }
    }
}
