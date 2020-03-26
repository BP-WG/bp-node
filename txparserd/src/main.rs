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

mod error;

pub mod controller {
    use tokio::net::{TcpListener, TcpStream};
    use crate::{input, error::Error};

    pub struct Config {
        pub input_socket: String
    }

    impl Default for Config {
        fn default() -> Self {
            let input_config = input::Config::default();
            Self {
                input_socket: input_config.socket
            }
        }
    }

    pub struct Server {
        //parser: Parser,
        //monitor: Monitor,
        //input: input::Server,
    }

    pub struct Stats {
        //pub input: input::Stats
    }

    impl Default for Stats {
        fn default() -> Self {
            Self {}
        }
    }

    impl Server {
        #[tokio::main]
        pub async fn init_and_run(config: Config) -> Result<Self, Error> {
            let server = Self {
                //parser: (),
                //monitor: (),
                //input: input::Server.init_and_run(config.into())
            };

            let config = Config::default();
            let input_server = input::Server::init_and_run(config.into())?;

            tokio::join!(
                {
                    let mut listener = TcpListener::bind("127.0.0.1:7897").await?;
                    println!("Listening on 127.0.0.1:7897");

                    tokio::spawn(async move {
                        loop {
                            let (socket, _) = listener.accept().await.unwrap();
                            println!("New client on 7897 port");

                            tokio::spawn(async move {
                                // Process each socket concurrently.
                                //process(socket).await
                            });
                        }
                    })
                },

                input_server.task
            );


            Ok(server)
        }

        pub fn get_stats(&self) -> Stats {
            Stats::default()
            //Stats { input: self.input.get_stats() }
        }
    }

    pub struct StateReport {

    }
}



fn main() -> Result<(), Box<dyn std::error::Error>> {
    controller::Server::init_and_run(controller::Config::default());

    Ok(())

    // TODO:
    //   1. Read and parse config
    //   2. Init internal state
    //   3. Init main threads

//    controller::Server::init_and_run(controller::Config::default());
}
