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

use std::net::{SocketAddr, TcpStream};
use std::process::exit;

use bprpc::{BlockMsg, RemoteAddr, Session};
use netservices::client::{ClientDelegate, ConnectionDelegate, OnDisconnect};
use netservices::{Frame, ImpossibleResource, NetTransport};

const NAME: &str = "exporter";

pub struct BlockExporter {
    pub provider: RemoteAddr,
}

impl ConnectionDelegate<RemoteAddr, Session> for BlockExporter {
    fn connect(&self, remote: &RemoteAddr) -> Session {
        debug_assert_eq!(remote, &self.provider);
        TcpStream::connect(remote).unwrap_or_else(|err| {
            log::error!(target: NAME, "Unable to connect blockchain provider {remote} due to {err}");
            log::warn!(target: NAME, "Stopping RPC import thread");
            exit(1);
        })
    }

    fn on_established(&self, remote: SocketAddr, _attempt: usize) {
        log::info!(target: NAME, "Connected to blockchain provider {} ({remote})", self.provider);
    }

    fn on_disconnect(&self, err: std::io::Error, _attempt: usize) -> OnDisconnect {
        log::error!(target: NAME, "Blockchain provider {} got disconnected due to {err}", self.provider);
        log::warn!(target: NAME, "Stopping RPC import thread");
        exit(1)
    }

    fn on_io_error(&self, err: reactor::Error<ImpossibleResource, NetTransport<Session>>) {
        log::error!(target: NAME, "I/O error in communicating with blockchain provider {}: {err}", self.provider);
    }
}

impl ClientDelegate<RemoteAddr, Session> for BlockExporter {
    type Reply = BlockMsg;

    fn on_reply(&mut self, block: BlockMsg) { todo!() }

    fn on_reply_unparsable(&mut self, err: <Self::Reply as Frame>::Error) {
        log::error!("Invalid message from blockchain provider {}: {err}", self.provider);
    }
}
