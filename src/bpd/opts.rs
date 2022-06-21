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
use clap::{Parser, ValueHint};
use internet2::addr::ServiceAddr;
use microservices::shell::shell_setup;

use crate::opts::Opts as SharedOpts;

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
#[clap(author, version, name = "bpd", about = "RGB node managing service")]
pub struct Opts {
    /// These params can be read also from the configuration file, not just
    /// command-line args or environment variables
    #[clap(flatten)]
    pub shared: SharedOpts,

    /// ZMQ socket name/address for RGB node RPC interface.
    ///
    /// Internal interface for control PRC protocol communications.
    #[clap(
        short = 'R',
        long = "rpc",
        env = "BP_NODE_RPC_ENDPOINT",
        default_value = BP_NODE_RPC_ENDPOINT,
        value_hint = ValueHint::FilePath
    )]
    pub rpc_endpoint: ServiceAddr,

    /// Spawn daemons as threads and not processes
    #[clap(short = 't', long = "threaded")]
    pub threaded_daemons: bool,
}

impl Opts {
    pub fn process(&mut self) {
        self.shared.process();
        shell_setup(self.shared.verbose, [&mut self.rpc_endpoint], &mut self.shared.data_dir, &[(
            "{chain}",
            self.shared.chain.to_string(),
        )]);
    }
}
