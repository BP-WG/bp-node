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

use std::any::Any;
use std::io;
use std::io::Error;
use std::net::TcpStream;

use amplify::confinement::TinyBlob;
use bprpc::{RemoteAddr, Request, Response, Session};
use netservices::client::{Client, ClientDelegate, ConnectionDelegate, OnDisconnect};
use netservices::{Frame, ImpossibleResource, NetSession, NetTransport};

pub struct Delegate;

pub struct BpClient {
    inner: Client<Request>,
}

impl BpClient {
    pub fn new(remote: RemoteAddr) -> io::Result<Self> {
        let delegate = Delegate;
        let inner = Client::new(delegate, remote)?;
        Ok(Self { inner })
    }

    pub fn ping(&mut self) -> io::Result<()> {
        let noise = TinyBlob::default(); // TODO: produce random noise
        self.inner.send(Request::Ping(noise))
    }

    pub fn join(self) -> Result<(), Box<dyn Any + Send>> { self.inner.join() }
}

impl ConnectionDelegate<RemoteAddr, Session> for Delegate {
    fn connect(&self, remote: &RemoteAddr) -> Session {
        TcpStream::connect(remote).expect("unable to connect to the server")
    }

    fn on_established(&self, _node_id: <Session as NetSession>::Artifact, _attempt: usize) {
        #[cfg(feature = "log")]
        log::info!("connection to the server is established");
    }

    fn on_disconnect(&self, err: Error, _attempt: usize) -> OnDisconnect {
        #[cfg(feature = "log")]
        log::error!("disconnected due to {err}");
        OnDisconnect::Terminate
    }

    fn on_io_error(&self, err: reactor::Error<ImpossibleResource, NetTransport<Session>>) {
        panic!("I/O error: {err}")
    }
}

impl ClientDelegate<RemoteAddr, Session> for Delegate {
    type Reply = Response;

    fn on_reply(&mut self, reply: Self::Reply) {
        #[cfg(feature = "log")]
        log::debug!("Received reply: {reply}");
        match reply {
            Response::Failure(failure) => {
                println!("Failure: {failure}");
            }
            Response::Pong(_noise) => {}
            Response::Status(status) => {
                println!("Status: {status}");
            }
        }
    }

    fn on_reply_unparsable(&mut self, err: <Self::Reply as Frame>::Error) {
        #[cfg(feature = "log")]
        log::error!("Received error message: {err}");
        panic!("received error message: {err}")
    }
}

/*
impl RpcDelegate<RemoteAddr, Session> for Delegate {
    type Reply = Response;

    fn on_msg_error(&self, err: impl std::error::Error) {
        #[cfg(feature = "log")]
        log::error!("received error message: {err}");
        panic!("received error message: {err}")
    }

    fn on_reply(&mut self, reply: Self::Reply) {
        #[cfg(feature = "log")]
        log::debug!("received reply: {reply}");
        match reply {
            Response::Failure(failure) => {
                println!("Failure: {failure}");
            }
            Response::Pong(_noise) => {}
            Response::Status(status) => {
                println!("Status: {status}");
            }
        }
    }
}

impl RpcPubDelegate<RemoteAddr, Session> for Delegate {
    type PubMsg = PubMessage;

    fn on_msg_pub(&self, id: u16, msg: PubMessage) {
        #[cfg(feature = "log")]
        log::debug!("received pub message #{id}: {msg}");
        match msg {
            PubMessage::ReversePing(_nois) => {}
        }
    }
}
*/
