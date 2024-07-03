// BP Node: sovereign bitcoin wallet backend.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2024 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2020-2024 Dr Maxim Orlovsky. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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

use clap::Parser;

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
