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
mod config;

use tokio::{
    sync::mpsc,
    task::JoinHandle,
    net::{TcpListener, TcpStream}
};
use crate::error::DaemonError;
use crate::config::Config;

#[tokio::main]
async fn main() -> Result<(), DaemonError> {
    // TODO: Init config from command-line arguments, environment and config file

    let config = Config::default();

    // TODO: Take buffer size from the configuration options
    let (mut parser_sender, mut parser_receiver) = mpsc::channel(100);

    let parser_task = parser::run(config.clone().into(), parser_receiver)?;
    let input_task = input::run(config.clone().into(), parser_sender)?;
    let monitor_task = monitor::run(config.clone().into())?;

    tokio::join!(
        input_task,
        parser_task,
        monitor_task
    );

    Ok(())
}
