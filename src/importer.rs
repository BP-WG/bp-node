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

use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::error::Error;
use std::net::{SocketAddr, TcpListener, TcpStream};

use bprpc::{ClientInfo, ExporterPub, ImporterReply, RemoteAddr, Session};
use netservices::Direction;
use netservices::remotes::DisconnectReason;
use netservices::service::{ServiceCommand, ServiceController};
use reactor::Timestamp;
use strict_encoding::DecodeError;

use crate::BlockProcessor;

// TODO: Make this configuration parameter
const MAX_PROVIDERS: usize = 0x10;
const NAME: &str = "importer";

pub struct BlockImporter {
    processor: BlockProcessor,
    commands: VecDeque<ServiceCommand<SocketAddr, ImporterReply>>,
    providers: HashMap<SocketAddr, ClientInfo>,
}

impl BlockImporter {
    pub fn new(processor: BlockProcessor) -> Self {
        Self { processor, commands: none!(), providers: none!() }
    }
}

impl ServiceController<RemoteAddr, Session, TcpListener, ()> for BlockImporter {
    type InFrame = ExporterPub;
    type OutFrame = ImporterReply;

    fn should_accept(&mut self, _remote: &RemoteAddr, _time: Timestamp) -> bool {
        self.providers.len() < MAX_PROVIDERS
    }

    fn establish_session(
        &mut self,
        remote: RemoteAddr,
        connection: TcpStream,
        _time: Timestamp,
    ) -> Result<Session, impl Error> {
        log::info!(target: NAME, "New block provider connected from {remote}");
        Result::<_, Infallible>::Ok(connection)
    }

    fn on_listening(&mut self, socket: SocketAddr) {
        log::info!(target: NAME, "Listening on {socket}");
    }

    fn on_established(
        &mut self,
        addr: SocketAddr,
        _remote: RemoteAddr,
        direction: Direction,
        time: Timestamp,
    ) {
        debug_assert_eq!(direction, Direction::Inbound);
        if self
            .providers
            .insert(addr, ClientInfo {
                agent: None,
                connected: time.as_millis(),
                last_seen: time.as_millis(),
            })
            .is_some()
        {
            panic!("Provider {addr} already connected!");
        };
    }

    fn on_disconnected(&mut self, addr: SocketAddr, _: Direction, reason: &DisconnectReason) {
        let client = self.providers.remove(&addr).unwrap_or_else(|| {
            panic!("Block provider at {addr} got disconnected but not found in providers list");
        });
        log::warn!(target: NAME, "Block provider at {addr} got disconnected due to {reason} ({})", client.agent.map(|a| a.to_string()).unwrap_or(none!()));
    }

    fn on_command(&mut self, _: ()) { unreachable!("there are no commands for this service") }

    fn on_frame(&mut self, remote: SocketAddr, msg: ExporterPub) {
        let client = self.providers.get_mut(&remote).expect("must be known");
        client.last_seen = Timestamp::now().as_millis();
        match msg {
            ExporterPub::Hello(_) => todo!(),
            ExporterPub::GetFilters => todo!(),
            ExporterPub::Block(block) => {
                let block_id = block.header.block_hash();
                log::debug!("Received block {block_id} from {remote}");
                match self.processor.process_block(block_id, block) {
                    Err(err) => {
                        log::error!(target: NAME, "{err}");
                        log::warn!(target: NAME, "Block {block_id} got dropped due to database connectivity issue");
                    }
                    Ok(count) => {
                        log::debug!(
                            "Successfully processed block {block_id}; {count} transactions added"
                        );
                    }
                }
            }
        }
    }

    fn on_frame_unparsable(&mut self, remote: SocketAddr, err: &DecodeError) {
        log::error!(target: NAME, "Disconnecting block provider {remote} due to unparsable frame: {err}");
        self.commands.push_back(ServiceCommand::Disconnect(remote))
    }
}

impl Iterator for BlockImporter {
    type Item = ServiceCommand<SocketAddr, ImporterReply>;

    fn next(&mut self) -> Option<Self::Item> { self.commands.pop_front() }
}
