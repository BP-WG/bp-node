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

            // Create the database
            let db = match Database::create(&index_path) {
                Ok(db) => db,
                Err(err) => {
                    eprintln!("unable to create index database.\n{err}");
                    exit(4);
                }
            };

            // Initialize database with network information
            let network = opts.general.network;
            match db.begin_write() {
                Ok(tx) => {
                    match tx.open_table(bpnode::db::TABLE_MAIN) {
                        Ok(mut main_table) => {
                            if let Err(err) = main_table
                                .insert(bpnode::REC_NETWORK, network.to_string().as_bytes())
                            {
                                eprintln!("Failed to write network information to database: {err}");
                                exit(5);
                            }
                        }
                        Err(err) => {
                            eprintln!("Failed to open main table in database: {err}");
                            exit(6);
                        }
                    }

                    if let Err(err) = tx.commit() {
                        eprintln!("Failed to commit initial database transaction: {err}");
                        exit(7);
                    }
                }
                Err(err) => {
                    eprintln!("Failed to begin database transaction: {err}");
                    exit(8);
                }
            }

            eprintln!("index database initialized for {} network, exiting", network);
            Status(Ok(()))
        }
        None => {
            let conf = Config::from(opts);
            let index_path = conf.data_dir.join(PATH_INDEXDB);

            // Check if the database exists
            if let Ok(true) = fs::exists(&index_path) {
                // Open the database to check network configuration
                match Database::open(&index_path) {
                    Ok(db) => {
                        // Check stored network matches configured network
                        if let Ok(tx) = db.begin_read() {
                            if let Ok(main_table) = tx.open_table(bpnode::db::TABLE_MAIN) {
                                if let Ok(Some(network_rec)) = main_table.get(bpnode::REC_NETWORK) {
                                    let stored_network =
                                        String::from_utf8_lossy(network_rec.value());
                                    if stored_network != conf.network.to_string() {
                                        eprintln!("ERROR: Database network mismatch!");
                                        eprintln!("Configured network: {}", conf.network);
                                        eprintln!("Database network: {}", stored_network);
                                        eprintln!(
                                            "Each BP-Node instance works with a single chain."
                                        );
                                        eprintln!(
                                            "To use a different network, create a separate \
                                             instance with a different data directory."
                                        );
                                        exit(9);
                                    }
                                    log::info!(
                                        "Database network matches configured network: {}",
                                        stored_network
                                    );
                                } else {
                                    // Network information not found in the database
                                    eprintln!(
                                        "ERROR: Database exists but doesn't contain network \
                                         information."
                                    );
                                    eprintln!(
                                        "Please reinitialize the database with 'bpd init' command."
                                    );
                                    exit(10);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!(
                            "Warning: Could not open database to check network configuration: {}",
                            err
                        );
                    }
                }
            } else {
                eprintln!(
                    "ERROR: Database not found! Please initialize with 'bpd init' command first."
                );
                exit(11);
            }

            Status(Broker::start(conf).and_then(|runtime| runtime.run()))
        }
    }
}
