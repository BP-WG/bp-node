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

/*
mod state;
mod schema;
mod parser;
*/

mod input;
mod monitor;
mod error;

pub mod controller {
    use tokio::net::{TcpListener, TcpStream};
    use crate::{input, monitor, error::Error};

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

    pub struct Server {
        //parser: Parser,
        pub monitor: monitor::Server,
        pub input: input::Server,
    }

    impl Server {
        pub fn init_and_run(config: Config) -> Result<Self, Error> {
            let config = Config::default();

            let input = input::Server::init_and_run(config.clone().into())?;
            let monitor = monitor::Server::init_and_run(config.clone().into())?;

            Ok(Self {
                input,
                monitor,
            })
        }
    }
}


#[tokio::main]
async fn main() {
    // TODO:
    //   1. Read and parse config
    //   2. Init internal state
    //   3. Init main threads

    let controller = controller::Server::init_and_run(controller::Config::default()).unwrap();
    tokio::join!(
        controller.input.task,
        controller.monitor.task
    );
}
