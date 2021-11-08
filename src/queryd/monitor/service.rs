// Bitcoin protocol (BP) daemon node
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

use prometheus::Encoder;
use tiny_http;

use microservices::node::Service;

use super::{error::Error, *};
use crate::error::*;

pub struct MonitorService {
    config: Config,
    context: zmq::Context,
    http_server: tiny_http::Server,
}

impl Service for MonitorService {
    fn run_loop(mut self) -> () {
        loop {
            match self.run() {
                Ok(_) => debug!("Monitoring client request processing completed"),
                Err(err) => {
                    error!("Error processing monitoring client request: {}", err)
                }
            }
        }
    }
}

impl MonitorService {
    pub fn init(config: Config, context: zmq::Context) -> Result<Self, BootstrapError> {
        let socket_addr = config.socket_addr.clone();
        let http_server = tiny_http::Server::http(socket_addr.clone())
            .map_err(|err| BootstrapError::MonitorSocketError(err))?;

        Ok(Self {
            config,
            context,
            http_server,
        })
    }

    fn run(&mut self) -> Result<(), Error> {
        let request = self
            .http_server
            .recv()
            .map_err(|err| Error::APIRequestError(err))?;

        let mut buffer = vec![];
        prometheus::TextEncoder::new().encode(&prometheus::gather(), &mut buffer)?;

        let response = tiny_http::Response::from_data(buffer);
        request
            .respond(response)
            .map_err(|err| Error::APIResponseError(err))
    }
}
