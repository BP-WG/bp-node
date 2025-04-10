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

use std::time::Duration;

use amplify::IoError;
use bpwallet::Network;
use microservices::UThread;
use netservices::{NetAccept, service};
use redb::DatabaseError;

use crate::db::IndexDb;
use crate::importer::BlockImporter;
use crate::{BlockProcessor, Config, RpcController};

pub const PATH_INDEXDB: &str = "bp-index";

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum InitError {
    /// unable to initialize RPC service.
    ///
    /// {0}
    Rpc(IoError),

    /// unable to initialize importing service.
    ///
    /// {0}
    Import(IoError),

    /// unable to initialize block importing service.
    ///
    /// {0}
    Importer(IoError),

    /// unable to open database.
    ///
    /// {0}
    ///
    /// Tip: make sure you have initialized database with `bpd init` command.
    #[from]
    Db(DatabaseError),

    /// unable to create thread for {0}.
    Thread(&'static str),
}

pub struct Runtime {
    network: Network,
    rpc: service::Runtime<()>,
    importer: service::Runtime<()>,
    db: UThread<IndexDb>,
}

impl Runtime {
    pub fn start(conf: Config) -> Result<Self, InitError> {
        // TODO: Add query thread

        const TIMEOUT: Option<Duration> = Some(Duration::from_secs(60 * 10));

        log::info!("Starting database managing thread...");
        let indexdb = IndexDb::new(&conf.data_dir.join(PATH_INDEXDB))?;
        let db = UThread::new(indexdb, TIMEOUT);

        log::info!("Starting block importer thread...");
        let processor = BlockProcessor::new(db.sender());
        let controller = BlockImporter::new(processor);
        let listen = conf.import.iter().map(|addr| {
            NetAccept::bind(addr).unwrap_or_else(|err| panic!("unable to bind to {addr}: {err}"))
        });
        let importer = service::Runtime::new(conf.import[0].clone(), controller, listen)
            .map_err(|err| InitError::Import(err.into()))?;

        log::info!("Starting RPC server thread...");
        let controller = RpcController::new();
        let listen = conf.rpc.iter().map(|addr| {
            NetAccept::bind(addr).unwrap_or_else(|err| panic!("unable to bind to {addr}: {err}"))
        });
        let rpc = service::Runtime::new(conf.rpc[0].clone(), controller, listen)
            .map_err(|err| InitError::Rpc(err.into()))?;

        log::info!("Launch completed successfully");
        Ok(Self { network: conf.network, rpc, importer, db })
    }

    pub fn run(self) -> Result<(), InitError> {
        self.importer
            .join()
            .map_err(|_| InitError::Thread("importer service"))?;
        self.rpc
            .join()
            .map_err(|_| InitError::Thread("RPC server"))?;
        Ok(())
    }
}
