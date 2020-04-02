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
#![feature(unwrap_infallible)]
#![feature(in_band_lifetimes)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate derive_wrapper;
#[macro_use]
extern crate async_trait;
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
mod traits;

pub use traits::*;

use std::env;
use log::*;
use futures::future;
use tokio::{
    sync::mpsc,
    task::JoinHandle,
    net::{TcpListener, TcpStream}
};
use crate::{
    error::*,
    config::Config,
};

const INPUT_PARSER_SOCKET: &str = "inproc://input-parser";
const PARSER_PUB_SOCKET: &str = "inproc://parser-input";

async fn run(config: Config) -> Result<(), BootstrapError> {
    let mut context = zmq::Context::new();

    let parser_task = parser::run(config.clone().into(), &mut context)?;
    let input_task = input::run(config.clone().into(), &mut context)?;
    let monitor_task = monitor::run(config.clone().into(), &mut context)?;

    let tasks: Vec<JoinHandle<!>> = vec![
        input_task, parser_task, monitor_task
    ].into_iter().flatten().collect();
    future::try_join_all(tasks).await;

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
