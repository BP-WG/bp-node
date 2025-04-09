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

//! Block importer interface organized into a reactor thread.

use std::net::SocketAddr;

use bprpc::{BlockMsg, RemoteAddr, Session};
use netservices::client::{ClientDelegate, ConnectionDelegate, OnDisconnect};
use netservices::{Frame, ImpossibleResource, NetTransport};

const NAME: &str = "importer";

pub struct RpcImport {}

impl RpcImport {
    pub fn new() -> Self { Self {} }
}

impl ConnectionDelegate<RemoteAddr, Session> for RpcImport {
    fn connect(&self, remote: &RemoteAddr) -> Session { todo!() }

    fn on_established(&self, remote: SocketAddr, attempt: usize) { todo!() }

    fn on_disconnect(&self, err: std::io::Error, attempt: usize) -> OnDisconnect { todo!() }

    fn on_io_error(&self, err: reactor::Error<ImpossibleResource, NetTransport<Session>>) {
        todo!()
    }
}

impl ClientDelegate<RemoteAddr, Session> for RpcImport {
    type Reply = BlockMsg;

    fn on_reply(&mut self, block: Self::Reply) { todo!() }

    fn on_reply_unparsable(&mut self, err: <Self::Reply as Frame>::Error) { todo!() }
}
