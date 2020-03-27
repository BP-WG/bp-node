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

#![feature(never_type)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate derive_wrapper;
extern crate dotenv;
extern crate chrono;
extern crate tiny_http;
extern crate prometheus;
#[macro_use]
extern crate txlib;

#[macro_use]
extern crate tokio;
extern crate futures;
extern crate zmq;

mod error;
mod schema;
mod parser;
mod input;
mod monitor;

use tokio::net::{TcpListener, TcpStream};
use crate::error::Error;

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub struct Config {
    pub input_socket: String,
    pub monitor_socket: String,
}

impl Default for Config {
    fn default() -> Self {
        let input_config = input::Config::default();
        let monitor_config = monitor::Config::default();
        Self {
            input_socket: input_config.socket,
            monitor_socket: monitor_config.socket,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // TODO:
    //   1. Read and parse config
    //   2. Init internal state
    //   3. Init main threads

    let config = Config::default();

    let input = input::Service::init_and_run(config.clone().into())?;
    let monitor = monitor::Service::init_and_run(config.clone().into())?;

    tokio::join!(
        input.task,
        monitor.task
    );

    Ok(())
}
