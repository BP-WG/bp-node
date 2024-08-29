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

//! Command-line interface to BP Node

#[macro_use]
extern crate amplify;
#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod args;
mod client;

use std::process::ExitCode;

use bpwallet::cli::{ExecError, LogLevel};
use clap::Parser;

pub use crate::args::{Args, Command};

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run() -> Result<(), ExecError> {
    eprintln!("BP: command-line interface to BP Node");
    eprintln!("    by LNP/BP Labs\n");

    let args = Args::parse();
    // args.process();
    LogLevel::from_verbosity_flag_count(args.verbose).apply();
    trace!("Command-line arguments: {:#?}", &args);

    // TODO: Update arguments basing on the configuration
    // let conf = Config::load(&args.conf_path("bp"));
    // debug!("Executing command: {}", args.command);
    // args.exec(conf, "bp-cli")
    Ok(())
}
