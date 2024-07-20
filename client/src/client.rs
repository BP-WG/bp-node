// BP Node: sovereign bitcoin wallet backend.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2024 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2020-2024 Dr Maxim Orlovsky. All rights reserved.
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

use std::io;
use std::io::Error;

use amplify::confinement::TinyBlob;
use bprpc::{PubMessage, Request, Response};
use cyphernet::addr::{InetHost, NetAddr, PeerAddr};
use cyphernet::ed25519;
use netservices::client::rpc_pub::RpcPubClient;
use netservices::client::{ConnectionDelegate, OnDisconnect, RpcDelegate, RpcPubDelegate};
use netservices::session::CypherSession;
use netservices::{ImpossibleResource, NetSession, NetTransport};
use sha2::Sha256;

pub type Addr = PeerAddr<ed25519::PublicKey, NetAddr<InetHost>>;
pub type Session = CypherSession<ed25519::PrivateKey, Sha256>;

pub struct Delegate;

pub struct Client {
    inner: RpcPubClient<Request, Response>,
}

impl Client {
    pub fn new(remote: Addr) -> io::Result<Self> {
        let delegate = Delegate;
        let inner = RpcPubClient::new(delegate, remote)?;
        Ok(Self { inner })
    }

    pub fn ping(&mut self) -> io::Result<()> {
        let noise = TinyBlob::default(); // TODO: produce random noise
        self.inner.send(Request::Ping(noise), |_| {
            #[cfg(feature = "log")]
            log::trace!("so far server connection is still alive")
        })
    }
}

impl ConnectionDelegate<Addr, Session> for Delegate {
    fn connect(&self, remote: &Addr) -> Session { todo!() }

    fn on_established(&self, artifact: <Session as NetSession>::Artifact, attempt: usize) {
        todo!()
    }

    fn on_disconnect(&self, err: Error, attempt: usize) -> OnDisconnect { todo!() }

    fn on_io_error(&self, err: reactor::Error<ImpossibleResource, NetTransport<Session>>) {
        todo!()
    }
}

impl RpcDelegate<Addr, Session> for Delegate {
    type Reply = Response;

    fn on_msg_error(&self, err: impl std::error::Error) { todo!() }

    fn on_reply(&mut self, reply: Self::Reply) { todo!() }
}

impl RpcPubDelegate<Addr, Session> for Delegate {
    type PubMsg = PubMessage;

    fn on_msg_pub(&self, id: u16, msg: PubMessage) { todo!() }
}
