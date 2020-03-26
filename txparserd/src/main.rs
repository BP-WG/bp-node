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


/*
mod state;
mod schema;
mod parser;
*/

//mod input;
/*
mod controller {
    use crate::input;

    pub struct Config {
        pub input_socket: String
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                input_socket: String::from("tcp://0.0.0.0:88318")
            }
        }
    }


    pub struct Server {
        //parser: Parser,
        //monitor: Monitor,
        input: input::Server,
    }

    pub struct Stats {
        pub input: input::Stats
    }

    impl Server {
        pub fn init_and_run(config: Config) -> Result<Self, tokio_zmq::Error> {
            let server = Self {
                //parser: (),
                //monitor: (),
                input: input::Server.init_and_run(config.into())
            };
            Ok(server)
        }

        pub fn get_stats(&self) -> Stats {
            Stats { input: self.input.get_stats() }
        }
    }

    pub struct StateReport {

    }
}*/


#[macro_use]
extern crate tokio;
extern crate futures;
extern crate zmq;

use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

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
                        process(socket).await
                    });
                }
            })
        },

        {
            let context = zmq::Context::new();
            let responder = context.socket(zmq::REP).unwrap();

            assert!(responder.bind("tcp://*:5555").is_ok());

            tokio::spawn(async move {
                let mut msg = zmq::Message::new();
                loop {
                    responder.recv(&mut msg, 0).unwrap();
                    println!("Received {}", msg.as_str().unwrap());
                    responder.send("World", 0).unwrap();
                }
            })
        }
    );

    Ok(())

    // TODO:
    //   1. Read and parse config
    //   2. Init internal state
    //   3. Init main threads

//    controller::Server::init_and_run(controller::Config::default());
}

async fn process(socket: TcpStream) {

}
