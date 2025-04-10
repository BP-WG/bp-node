// BP Node: sovereign bitcoin wallet backend.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2025 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2025 LNP/BP Labs, InDCS, Switzerland. All rights reserved.
// Copyright (C) 2020-2025 Dr Maxim Orlovsky. All rights reserved.
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

#[macro_use]
extern crate clap;

mod opts;

use std::fs;
use std::process::{ExitCode, Termination, exit};

pub use bpnode;
use bpnode::{Broker, BrokerError, Config, PATH_INDEXDB};
use clap::Parser;
use loglevel::LogLevel;
use redb::Database;

use crate::opts::{Command, Opts};

struct Status(Result<(), BrokerError>);

impl Termination for Status {
    fn report(self) -> ExitCode {
        match self.0 {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("Error: {err}");
                ExitCode::FAILURE
            }
        }
    }
}

fn main() -> Status {
    let mut opts = Opts::parse();
    opts.process();
    LogLevel::from_verbosity_flag_count(opts.verbose).apply();
    log::debug!("Command-line arguments: {:#?}", &opts);

    match opts.command {
        Some(Command::Init) => {
            eprint!("Initializing ... ");
            let index_path = opts.general.data_dir.join(PATH_INDEXDB);
            match fs::exists(&index_path) {
                Err(err) => {
                    eprintln!("unable to access path '{}': {err}", index_path.display());
                    exit(1);
                }
                Ok(true) => {
                    eprintln!("index database directory already exists, cancelling");
                    exit(2);
                }
                Ok(false) => {}
            }
            if let Err(err) = fs::create_dir_all(&opts.general.data_dir) {
                eprintln!(
                    "unable to create data directory at '{}'\n{err}",
                    opts.general.data_dir.display()
                );
                exit(3);
            }
            if let Err(err) = Database::create(&index_path) {
                eprintln!("unable to create index database.\n{err}");
                exit(4);
            }
            eprintln!("index database initialized, exiting");
            Status(Ok(()))
        }
        None => {
            let conf = Config::from(opts);
            Status(Broker::start(conf).and_then(|runtime| runtime.run()))
        }
    }
}
