// BP node providing distributed storage & messaging for lightning network.
//
// Written in 2022 by
//     Dr. Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2022 by LNP/BP Standards Association, Switzerland.
//
// You should have received a copy of the MIT License along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

#![recursion_limit = "256"]

//! Command-line interface to bp node

#[macro_use]
extern crate amplify;
#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod command;
mod opts;

use bp_rpc::client::Client;
use clap::Parser;
use internet2::addr::ServiceAddr;
use microservices::cli::LogStyle;
use microservices::shell::{Exec, LogLevel};

pub use crate::opts::{Command, Opts};

fn main() {
    println!("bp-cli: command-line tool for working with BP node");

    let opts = Opts::parse();
    LogLevel::from_verbosity_flag_count(opts.verbose).apply();
    trace!("Command-line arguments: {:#?}", &opts);

    let mut connect = opts.connect.clone();
    if let ServiceAddr::Ipc(ref mut path) = connect {
        *path = shellexpand::tilde(path).to_string();
    }
    debug!("RPC socket {}", connect);

    let mut client = Client::with(&connect).expect("Error initializing client");

    trace!("Executing command: {}", opts.command);
    opts.exec(&mut client)
        .unwrap_or_else(|err| eprintln!("{} {}\n", "Error:".err(), err.err_details()));
}
