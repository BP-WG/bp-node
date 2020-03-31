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
#[macro_use]
extern crate log;
extern crate env_logger;
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

use std::env;
use log::*;
use tokio::{
    sync::mpsc,
    task::JoinHandle,
    net::{TcpListener, TcpStream}
};
use crate::{
    config::Config,
    error::DaemonError,
    parser::InputChannel,
    input::ParserChannel,
};

async fn run(config: Config) -> Result<(), DaemonError> {
    // TODO: Take buffer size from the configuration options
    let (mut parser_sender, mut parser_receiver) = mpsc::channel(100);
    let (mut input_sender, mut input_receiver) = mpsc::channel(100);
    let mut parser_channel = ParserChannel { req: parser_sender, rep: input_receiver };
    let mut input_channel = InputChannel { req: input_sender, rep: parser_receiver };

    debug!("Sending request via channel");
    parser_channel.req.send(parser::Request{id:0, cmd:parser::Command::Statistics}).await;
    debug!("Request sent; waiting for receiving the request");
    match input_channel.rep.recv().await {
        Some(rep) => debug!("Received request: {:?}", rep),
        None => error!("Channel is broken"),
    }

    let parser_task = parser::run(config.clone().into(), input_channel)?;
    let input_task = input::run(config.clone().into(), parser_channel)?;
    let monitor_task = monitor::run(config.clone().into())?;

    tokio::join!(
        input_task,
        parser_task,
        monitor_task
    );

    Ok(())
}

#[tokio::main]
async fn main() {
    println!("\ntxparserd: Bitcoin blockchain parser tool adding the data from it to the index database\n");

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "trace");
    }
    env_logger::init();
    log::set_max_level(LevelFilter::Trace);

    // TODO: Init config from command-line arguments, environment and config file

    let config = Config::default();

    if let Err(err) = run(config).await {
        eprintln!("Error running daemon: {:?}", err);
    };
}
