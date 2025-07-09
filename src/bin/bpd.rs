// BP Node: sovereign bitcoin wallet backend.
//
// SPDX-License-Identifier: Apache-2.0
//
// Designed & written in 2020-2025 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2025 LNP/BP Labs, InDCS, Switzerland. All rights reserved.
// Copyright (C) 2020-2025 Dr Maxim Orlovsky. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//        http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
// or implied. See the License for the specific language governing permissions and limitations under
// the License.

#[macro_use]
extern crate clap;

mod opts;

use std::fs;
use std::path::Path;
use std::process::{ExitCode, Termination, exit};

pub use bpnode;
use bpnode::{Broker, BrokerError, Config, PATH_INDEXDB, initialize_db_tables};
use bpstd::Network;
use clap::Parser;
use loglevel::LogLevel;
use redb::Database;

use crate::opts::{Command, Opts};

// Exit status codes for different error conditions
// see also constants in `db.rs`
const EXIT_PATH_ACCESS_ERROR: i32 = 1;
const EXIT_DB_EXISTS_ERROR: i32 = 2;
const EXIT_DIR_CREATE_ERROR: i32 = 3;
const EXIT_DB_CREATE_ERROR: i32 = 4;
const EXIT_DB_OPEN_ERROR: i32 = 5;
const EXIT_NETWORK_MISMATCH: i32 = 10;
const EXIT_NO_NETWORK_INFO: i32 = 11;
const EXIT_DB_NOT_FOUND: i32 = 12;

/// Wrapper for result status to implement Termination trait
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
        Some(Command::Init) => initialize_database(&opts),
        None => run_node(opts),
    }
}

/// Initialize a new database for the BP Node
fn initialize_database(opts: &Opts) -> Status {
    eprint!("Initializing ... ");

    // Prepare the database path
    let index_path = opts.data_dir.join(PATH_INDEXDB);

    // Check if database already exists
    if let Err(err) = check_db_path(&index_path, false) {
        return err;
    }

    // Create data directory if needed
    if let Err(err) = fs::create_dir_all(&opts.data_dir) {
        eprintln!("Unable to create data directory at '{}'\n{err}", opts.data_dir.display());
        exit(EXIT_DIR_CREATE_ERROR);
    }

    // Create the database
    let db = match Database::create(&index_path) {
        Ok(db) => db,
        Err(err) => {
            eprintln!("Unable to create index database.\n{err}");
            exit(EXIT_DB_CREATE_ERROR);
        }
    };

    // Initialize database with network information and create all tables
    let network = opts.network;
    initialize_db_tables(&db, network);

    eprintln!("Index database initialized for {} network, exiting", network);
    Status(Ok(()))
}

/// Run the BP Node service
fn run_node(opts: Opts) -> Status {
    let conf = Config::from(opts);
    let index_path = conf.data_dir.join(PATH_INDEXDB);

    // Check if database exists
    if let Err(err) = check_db_path(&index_path, true) {
        return err;
    }

    // Verify network configuration
    verify_network_configuration(&index_path, &conf.network);

    // Start the broker service
    Status(Broker::start(conf).and_then(|runtime| runtime.run()))
}

/// Check if database path exists or not, depending on expected state
fn check_db_path(index_path: &Path, should_exist: bool) -> Result<(), Status> {
    match fs::exists(index_path) {
        Err(err) => {
            eprintln!("Unable to access path '{}': {err}", index_path.display());
            exit(EXIT_PATH_ACCESS_ERROR);
        }
        Ok(exists) => {
            if exists && !should_exist {
                eprintln!("Index database directory already exists, cancelling");
                exit(EXIT_DB_EXISTS_ERROR);
            } else if !exists && should_exist {
                eprintln!(
                    "ERROR: Database not found! Please initialize with 'bpd init' command first."
                );
                exit(EXIT_DB_NOT_FOUND);
            }
        }
    }
    Ok(())
}

/// Verify that database network configuration matches the configured network
fn verify_network_configuration(index_path: &Path, configured_network: &Network) {
    let Ok(db) = Database::open(index_path)
        .inspect_err(|err| eprintln!("Error: could not open the database due to {err}"))
    else {
        exit(EXIT_DB_OPEN_ERROR)
    };
    let Ok(tx) = db
        .begin_read()
        .inspect_err(|err| eprintln!("Error: could not access the database due to {err}"))
    else {
        exit(EXIT_DB_OPEN_ERROR)
    };
    let Ok(main_table) = tx
        .open_table(bpnode::db::TABLE_MAIN)
        .inspect_err(|err| eprintln!("Error: could not open the main table due to {err}"))
    else {
        exit(EXIT_DB_OPEN_ERROR)
    };
    let Ok(Some(network_rec)) = main_table.get(bpnode::REC_NETWORK) else {
        // Network information isn't found in the database
        eprintln!("ERROR: Database exists but doesn't contain network information.");
        eprintln!("Please reinitialize the database with `bpd init` command.");
        exit(EXIT_NO_NETWORK_INFO);
    };
    let stored_network = String::from_utf8_lossy(network_rec.value());
    if stored_network != configured_network.to_string() {
        eprintln!("ERROR: Database network mismatch!");
        eprintln!("Configured network: {}", configured_network);
        eprintln!("Database network: {}", stored_network);
        eprintln!("Each BP-Node instance works with a single chain.");
        eprintln!(
            "To use a different network, create a separate instance with a different data \
             directory."
        );
        exit(EXIT_NETWORK_MISMATCH);
    }
    log::info!("Database network matches configured network: {}", stored_network);
}
