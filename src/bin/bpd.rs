// BP Node: bitcoin blockchain indexing and notification service
//
// Written in 2020-2022 by
//     Dr. Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2022 by LNP/BP Standards Association, Switzerland.
//
// You should have received a copy of the MIT License along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

#![recursion_limit = "256"]

//! Main executable for BP node.

#[macro_use]
extern crate log;

use bp_node::bpd::Opts;
use bp_node::{bpd, Config, LaunchError};
use clap::Parser;
use microservices::error::BootstrapError;

fn main() -> Result<(), BootstrapError<LaunchError>> {
    println!("bpd: managing bp node daemon");

    let mut opts = Opts::parse();
    trace!("Command-line arguments: {:?}", opts);
    opts.process();
    trace!("Processed arguments: {:?}", opts);

    let config = Config::from(opts);
    trace!("Daemon configuration: {:?}", config);
    debug!("CTL socket {}", config.ctl_endpoint);
    debug!("RPC socket {}", config.rpc_endpoint);
    debug!("STORE socket {}", config.store_endpoint);

    /*
    use self::internal::ResultExt;
    let (config_from_file, _) =
        internal::Config::custom_args_and_optional_files(std::iter::empty::<
            &str,
        >())
        .unwrap_or_exit();
     */

    debug!("Starting runtime ...");
    bpd::run(config).expect("running bpd runtime");

    unreachable!()
}
