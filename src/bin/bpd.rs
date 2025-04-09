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
extern crate amplify;
#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod opts;

use amplify::IoError;
pub use bpnode;
use bpnode::{Config, RpcController};
use bpwallet::cli::LogLevel;
use clap::Parser;
use netservices::{NetAccept, service};

use crate::opts::Opts;

#[derive(Debug, Display, Error)]
#[display(inner)]
pub enum Error {
    Rpc(IoError),

    /// unable to create thread for {0}
    Thread(&'static str),
}

fn main() -> Result<(), Error> {
    let mut opts = Opts::parse();
    opts.process();
    LogLevel::from_verbosity_flag_count(opts.verbose).apply();
    trace!("Command-line arguments: {:#?}", &opts);

    eprintln!("BP Node (daemon): sovereign bitcoin wallet backend");
    eprintln!("    by LNP/BP Labs, Switzerland\n");

    // TODO: Update arguments basing on the configuration
    let conf = Config::from(opts);

    let controller = RpcController::new();
    let listen = conf.listening.iter().map(|addr| {
        NetAccept::bind(addr).unwrap_or_else(|err| panic!("unable to bind to {addr}: {err}"))
    });
    service::Runtime::new(conf.listening[0].clone(), controller, listen)
        .map_err(|err| Error::Rpc(err.into()))?
        .join()
        .map_err(|_| Error::Thread("RPC controller"))?;

    Ok(())
}
