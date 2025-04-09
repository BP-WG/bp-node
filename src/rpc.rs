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

//! RPC connections from clients organized into a reactor thread.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::error::Error;
use std::net::{SocketAddr, TcpListener, TcpStream};

use bprpc::{RemoteAddr, Request, Response, Session, Status};
use netservices::remotes::DisconnectReason;
use netservices::service::ServiceController;
use netservices::{Direction, Frame, NetAccept, NetTransport};
use reactor::{Action, ResourceId, Timestamp};
use strict_encoding::DecodeError;

const NAME: &str = "rpc";

pub struct RpcController {
    actions: VecDeque<Action<NetAccept<Session, TcpListener>, NetTransport<Session>>>,
    clients: u16,
}

impl RpcController {
    pub fn new() -> Self { Self { actions: none!(), clients: 0 } }
}

impl ServiceController<RemoteAddr, Session, TcpListener, ()> for RpcController {
    type InFrame = Request;

    fn should_accept(&mut self, _remote: &RemoteAddr, _time: Timestamp) -> bool {
        // For now, we just do not allow more than 64k connections.
        // In a future, we may also filter out known clients doing spam and DDoS attacks
        self.clients < 0xFFFF
    }

    fn establish_session(
        &mut self,
        _remote: RemoteAddr,
        connection: TcpStream,
        _time: Timestamp,
    ) -> Result<Session, impl Error> {
        self.clients += 1;
        Result::<_, Infallible>::Ok(connection)
    }

    fn on_listening(&mut self, socket: SocketAddr) {
        log::info!(target: NAME, "Listening on {socket}");
    }

    fn on_disconnected(&mut self, _: SocketAddr, _: Direction, _: &DisconnectReason) {
        self.clients -= 1;
    }

    fn on_command(&mut self, _: ()) { unreachable!("there are no commands for this service") }

    fn on_frame(&mut self, res_id: ResourceId, req: Request) {
        log::debug!(target: NAME, "Processing `{req}`");
        let response = match req {
            Request::Ping(noise) => Response::Pong(noise),
            Request::Noop => {
                // Do nothing
                return;
            }
            Request::Status => Response::Status(Status { clients: self.clients }),
        };
        log::debug!(target: NAME, "Sending `{response}`");
        let mut data = Vec::new();
        let _ = response.marshall(&mut data);
        self.actions.push_back(Action::Send(res_id, data));
    }

    fn on_frame_unparsable(&mut self, res_id: ResourceId, err: &DecodeError) {
        log::error!(target: NAME, "Disconnecting {res_id} due to unparsable frame: {err}");
        self.actions.push_back(Action::UnregisterTransport(res_id))
    }
}

impl Iterator for RpcController {
    type Item = Action<NetAccept<Session, TcpListener>, NetTransport<Session>>;

    fn next(&mut self) -> Option<Self::Item> { self.actions.pop_front() }
}
