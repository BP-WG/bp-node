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
use std::convert::Infallible;
use std::error::Error;
use std::net::{SocketAddr, TcpListener, TcpStream};

use amplify::Wrapper;
use bprpc::{BlockMsg, RemoteAddr, Session};
use netservices::remotes::DisconnectReason;
use netservices::service::ServiceController;
use netservices::{Direction, NetAccept, NetTransport};
use reactor::{Action, ResourceId, Timestamp};
use strict_encoding::DecodeError;

use crate::BlockProcessor;

const MAX_PROVIDERS: u16 = 0x10;
const NAME: &str = "importer";

pub struct BlockImporter {
    processor: BlockProcessor,
    actions: VecDeque<Action<NetAccept<Session, TcpListener>, NetTransport<Session>>>,
    providers: u16,
}

impl BlockImporter {
    pub fn new(processor: BlockProcessor) -> Self {
        Self { processor, actions: none!(), providers: 0 }
    }
}

impl ServiceController<RemoteAddr, Session, TcpListener, ()> for BlockImporter {
    type InFrame = BlockMsg;

    fn should_accept(&mut self, _remote: &RemoteAddr, _time: Timestamp) -> bool {
        self.providers < MAX_PROVIDERS
    }

    fn establish_session(
        &mut self,
        remote: RemoteAddr,
        connection: TcpStream,
        _time: Timestamp,
    ) -> Result<Session, impl Error> {
        log::info!(target: NAME, "New block provider connected from {remote}");
        self.providers += 1;
        Result::<_, Infallible>::Ok(connection)
    }

    fn on_listening(&mut self, socket: SocketAddr) {
        log::info!(target: NAME, "Listening on {socket}");
    }

    fn on_disconnected(&mut self, addr: SocketAddr, _: Direction, reason: &DisconnectReason) {
        log::warn!(target: NAME, "Block provider att {addr} got disconnected due to {reason}");
        self.providers -= 1;
    }

    fn on_command(&mut self, _: ()) { unreachable!("there are no commands for this service") }

    fn on_frame(&mut self, res_id: ResourceId, block: BlockMsg) {
        let block_id = block.header.block_hash();
        log::debug!("Received block {block_id} from {res_id}");
        match self.processor.process_block(block_id, block.into_inner()) {
            Err(err) => {
                log::error!(target: NAME, "{err}");
                log::warn!(target: NAME, "Block {block_id} got dropped due to database connectivity issue");
            }
            Ok(count) => {
                log::debug!("Successfully processed block {block_id}; {count} transactions added");
            }
        }
    }

    fn on_frame_unparsable(&mut self, res_id: ResourceId, err: &DecodeError) {
        log::error!(target: NAME, "Disconnecting block provider {res_id} due to unparsable frame: {err}");
        self.actions.push_back(Action::UnregisterTransport(res_id))
    }
}

impl Iterator for BlockImporter {
    type Item = Action<NetAccept<Session, TcpListener>, NetTransport<Session>>;

    fn next(&mut self) -> Option<Self::Item> { self.actions.pop_front() }
}
