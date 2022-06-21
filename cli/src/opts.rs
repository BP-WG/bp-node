// BP Node: bitcoin blockchain indexing and notification service
//
// Written in 2020-2022 by
//     Dr. Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2022 by LNP/BP Standards Association, Switzerland.
//
// You should have received a copy of the MIT License along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use bp_rpc::BP_NODE_RPC_ENDPOINT;
use internet2::addr::ServiceAddr;

/// Command-line tool for working with store daemon
#[derive(Parser, Clone, PartialEq, Eq, Debug)]
#[clap(name = "bp-cli", bin_name = "bp-cli", author, version)]
pub struct Opts {
    /// ZMQ socket for connecting daemon RPC interface.
    ///
    /// Socket can be either TCP address in form of `<ipv4 | ipv6>:<port>` – or a path
    /// to an IPC file.
    ///
    /// Defaults to `0.0.0.0:61961`.
    #[clap(
        short = 'R',
        long = "rpc",
        global = true,
        default_value = BP_NODE_RPC_ENDPOINT,
        env = "BP_NODE_RPC_ENDPOINT"
    )]
    pub connect: ServiceAddr,

    /// Set verbosity level.
    ///
    /// Can be used multiple times to increase verbosity.
    #[clap(short, long, global = true, parse(from_occurrences))]
    pub verbose: u8,

    /// Command to execute
    #[clap(subcommand)]
    pub command: Command,
}

/// Command-line commands:
#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display)]
pub enum Command {
    #[display("none")]
    None,
}
