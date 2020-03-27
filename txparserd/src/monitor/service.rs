// Bitcoin transaction processing & database indexing daemon
// Written in 2020 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.


use tiny_http;
use tokio::task::JoinHandle;
use prometheus::{Opts, Registry, Counter, TextEncoder, Encoder};

use super::*;

pub struct Service {
    pub task: JoinHandle<Result<!, Error>>
}

impl Service {
    pub fn init_and_run(config: Config) -> Result<Self, Error> {
        let http_server = tiny_http::Server::http(config.socket).unwrap_or_else(|e| {
            panic!(
                "failed to start monitoring HTTP server"
            )
        });

        let task = tokio::spawn(async move {
            loop {
                let request = http_server.recv().unwrap();
                let mut buffer = vec![];
                prometheus::TextEncoder::new()
                    .encode(&prometheus::gather(), &mut buffer)
                    .unwrap();
                let response = tiny_http::Response::from_data(buffer);
                request.respond(response);
            }
        });

        Ok(Self {
            task,
        })
    }
}
