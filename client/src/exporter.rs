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

use std::collections::VecDeque;
use std::net::{SocketAddr, TcpStream};
use std::process::exit;

use bprpc::{ExporterPub, FiltersMsg, ImporterReply, RemoteAddr, Session};
use netservices::client::{ClientCommand, ClientDelegate, ConnectionDelegate, OnDisconnect};
use netservices::{Frame, ImpossibleResource, NetTransport};

const NAME: &str = "exporter";

pub struct BlockExporter {
    commands: VecDeque<ClientCommand<ExporterPub>>,
    filters: FiltersMsg,
    filters_received: bool,
}

impl BlockExporter {
    pub fn new() -> Self {
        Self {
            commands: none!(),
            filters: strict_dumb!(),
            filters_received: false,
        }
    }

    pub fn disconnect(&mut self) {
        self.commands.clear();
        self.commands.push_back(ClientCommand::Terminate);
    }
}

impl ConnectionDelegate<RemoteAddr, Session> for BlockExporter {
    type Request = ExporterPub;

    fn connect(&mut self, remote: &RemoteAddr) -> Session {
        TcpStream::connect(remote).unwrap_or_else(|err| {
            log::error!(target: NAME, "Unable to connect BP Node {remote} due to {err}");
            log::warn!(target: NAME, "Stopping RPC import thread");
            exit(1);
        })
    }

    fn on_established(&mut self, remote: SocketAddr, _attempt: usize) {
        log::info!(target: NAME, "Connected to BP Node {remote}, sending `hello(...)`");
    }

    fn on_disconnect(&mut self, err: std::io::Error, _attempt: usize) -> OnDisconnect {
        log::error!(target: NAME, "BP Node got disconnected due to {err}");
        exit(1)
    }

    fn on_io_error(&mut self, err: reactor::Error<ImpossibleResource, NetTransport<Session>>) {
        log::error!(target: NAME, "I/O error in communicating with BP Node: {err}");
        self.disconnect();
    }
}

impl ClientDelegate<RemoteAddr, Session> for BlockExporter {
    type Reply = ImporterReply;

    fn on_reply(&mut self, msg: ImporterReply) {
        match msg {
            ImporterReply::Filters(filters) => {
                if self.filters_received {
                    log::warn!(target: NAME, "Received duplicate filters");
                } else {
                    log::info!(target: NAME, "Received filters");
                }
                self.filters = filters;
                self.filters_received = true;
            }
            ImporterReply::Error(failure) => {
                log::error!(target: NAME, "Received error from BP Node: {failure}");
                self.disconnect();
            }
        }
    }

    fn on_reply_unparsable(&mut self, err: <Self::Reply as Frame>::Error) {
        log::error!("Invalid message from BP Node: {err}");
    }
}

impl Iterator for BlockExporter {
    type Item = ClientCommand<ExporterPub>;

    fn next(&mut self) -> Option<Self::Item> { self.commands.pop_front() }
}
