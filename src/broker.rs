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

use std::collections::{HashMap, HashSet};
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use amplify::IoError;
use amplify::confinement::TinyOrdSet;
use bprpc::{BloomFilter32, Response};
use crossbeam_channel::{Receiver, select};
use microservices::UThread;
use netservices::{NetAccept, service};
use redb::DatabaseError;

use crate::db::IndexDb;
use crate::importer::BlockImporter;
use crate::rpc::RpcCmd;
use crate::{Config, ImporterCmd, ImporterMsg, RpcController};

pub const PATH_INDEXDB: &str = "bp-index";

#[derive(Debug, Display)]
pub enum BrokerRpcMsg {
    #[display("TRACK")]
    Track(SocketAddr, TrackReq),

    #[display("UNTRACK_ALL")]
    UntrackAll(SocketAddr),
}

#[derive(Debug)]
pub enum TrackReq {
    TrackTxids(TinyOrdSet<BloomFilter32>),
}

pub struct Broker {
    rpc: service::Runtime<RpcCmd>,
    importer: service::Runtime<ImporterCmd>,
    db: UThread<IndexDb>,
    rpc_rx: Receiver<BrokerRpcMsg>,
    blocks_rx: Receiver<ImporterMsg>,

    tracking: HashMap<SocketAddr, HashSet<BloomFilter32>>,
}

impl Broker {
    pub fn start(conf: Config) -> Result<Self, BrokerError> {
        // TODO: Add query thread pool

        const TIMEOUT: Option<Duration> = Some(Duration::from_secs(60 * 10));

        log::info!("Starting database managing thread...");
        let indexdb = IndexDb::new(conf.data_dir.join(PATH_INDEXDB))?;
        let db = UThread::new(indexdb, TIMEOUT);

        log::info!("Starting block importer thread...");
        let (block_tx, blocks_rx) = crossbeam_channel::unbounded::<ImporterMsg>();
        let controller = BlockImporter::new(conf.network, db.sender(), block_tx);
        let listen = conf.import.iter().map(|addr| {
            NetAccept::bind(addr).unwrap_or_else(|err| panic!("unable to bind to {addr}: {err}"))
        });
        let importer = service::Runtime::new(conf.import[0], controller, listen)
            .map_err(|err| BrokerError::Import(err.into()))?;

        log::info!("Starting RPC server thread...");
        let (rpc_tx, rpc_rx) = crossbeam_channel::unbounded::<BrokerRpcMsg>();
        let controller = RpcController::new(conf.network, rpc_tx.clone());
        let listen = conf.rpc.iter().map(|addr| {
            NetAccept::bind(addr).unwrap_or_else(|err| panic!("unable to bind to {addr}: {err}"))
        });
        let rpc = service::Runtime::new(conf.rpc[0], controller, listen)
            .map_err(|err| BrokerError::Rpc(err.into()))?;

        log::info!("Launch completed successfully");
        Ok(Self { rpc, importer, db, rpc_rx, blocks_rx, tracking: none!() })
    }

    pub fn run(mut self) -> Result<(), BrokerError> {
        select! {
            recv(self.rpc_rx) -> msg => {
                match msg {
                    Ok(msg) => { self.proc_rpc_msg(msg).expect("unable to send message"); },
                    Err(err) => {
                        log::error!("Error receiving RPC message: {err}");
                    }
                }
            }

            recv(self.blocks_rx) -> msg => {
                match msg {
                    Ok(msg) => self.proc_block_msg(msg).expect("unable to send message"),
                    Err(err) => {
                        log::error!("Error receiving importer message: {err}");
                    }
                }
            }
        }

        self.importer
            .join()
            .map_err(|_| BrokerError::Thread("importer service"))?;
        self.rpc
            .join()
            .map_err(|_| BrokerError::Thread("RPC server"))?;
        Ok(())
    }

    pub fn proc_rpc_msg(&mut self, msg: BrokerRpcMsg) -> io::Result<()> {
        log::debug!("Received RPC message: {msg}");
        match msg {
            BrokerRpcMsg::Track(remote, TrackReq::TrackTxids(filter)) => {
                self.tracking
                    .entry(remote)
                    .or_default()
                    .extend(filter.clone());
                self.importer
                    .cmd(ImporterCmd::TrackTxid(filter.into_iter().collect()))?;
            }
            BrokerRpcMsg::UntrackAll(remote) => {
                let Some(filters) = self.tracking.remove(&remote) else {
                    return Ok(());
                };
                let all = self
                    .tracking
                    .values()
                    .flatten()
                    .copied()
                    .collect::<HashSet<_>>();
                let filters = filters.difference(&all);
                self.importer
                    .cmd(ImporterCmd::Untrack(filters.copied().collect()))?;
            }
        }
        Ok(())
    }

    pub fn proc_block_msg(&mut self, msg: ImporterMsg) -> io::Result<()> {
        log::debug!("Received importer message: {msg}");
        match msg {
            ImporterMsg::Mined(txid) => {
                for (remote, filters) in &self.tracking {
                    for filter in filters {
                        if filter.contains(txid) {
                            self.rpc.cmd(RpcCmd::Send(*remote, Response::Mined(txid)))?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum BrokerError {
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
