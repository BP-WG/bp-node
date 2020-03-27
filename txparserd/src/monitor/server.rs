use tiny_http;
use tokio::task::JoinHandle;
use prometheus::{Opts, Registry, Counter, TextEncoder, Encoder};

use super::*;

pub struct Server {
    pub task: JoinHandle<Result<!, Error>>
}

impl Server {
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