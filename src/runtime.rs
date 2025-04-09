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
use bprpc::BlockMsg;
use bpwallet::Network;
use microservices::UThread;
use netservices::client::Client;
use netservices::{NetAccept, service};
use redb::DatabaseError;

use crate::db::IndexDb;
use crate::{BlockImporter, Config, RpcController};

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum InitError {
    Rpc(IoError),
    Importer(IoError),

    #[from]
    Db(DatabaseError),

    /// unable to create thread for {0}
    Thread(&'static str),
}

pub struct Runtime {
    network: Network,
    rpc: service::Runtime<()>,
    importers: Vec<Client<BlockMsg>>,
    db: UThread<IndexDb>,
}

impl Runtime {
    pub fn start(conf: Config) -> Result<Self, InitError> {
        // TODO: Add inter-thread connectivity

        const TIMEOUT: Option<Duration> = Some(Duration::from_secs(60 * 10));

        log::info!("Starting database managing thread...");
        let indexdb = IndexDb::new(&conf.data_dir.join("bp-index"))?;
        let db = UThread::new(indexdb, TIMEOUT);

        let mut importers = Vec::new();
        for provider in &conf.providers {
            log::info!("Connecting to block provider {provider}...");
            let controller = BlockImporter::new(db.sender(), provider.clone());
            let importer = Client::new(controller, provider.clone())
                .map_err(|err| InitError::Importer(err.into()))?;
            importers.push(importer);
        }

        log::info!("Starting RPC server thread...");
        let controller = RpcController::new();
        let listen = conf.listening.iter().map(|addr| {
            NetAccept::bind(addr).unwrap_or_else(|err| panic!("unable to bind to {addr}: {err}"))
        });
        let rpc = service::Runtime::new(conf.listening[0].clone(), controller, listen)
            .map_err(|err| InitError::Rpc(err.into()))?;

        log::info!("Launch completed successfully");
        Ok(Self { network: conf.network, rpc, importers, db })
    }

    pub fn run(self) -> Result<(), InitError> {
        self.rpc
            .join()
            .map_err(|_| InitError::Thread("RPC server"))?;
        for importer in self.importers {
            importer.join().map_err(|_| InitError::Thread("importer"))?;
        }
        Ok(())
    }
}
