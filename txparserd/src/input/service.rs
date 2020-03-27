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


use tokio::task::JoinHandle;

use super::*;

pub struct Service {
    config: Config,
    stats: Stats,
    pub task: JoinHandle<Result<!, Error>>
}

impl Service {
    pub fn init_and_run(config: Config) -> Result<Self, Error> {
        let context = zmq::Context::new();
        let responder = context.socket(zmq::REP).unwrap();

        assert!(responder.bind(config.socket.as_str()).is_ok());
        println!("Listening on {}", config.socket);

        let task = tokio::spawn(async move {
            let mut msg = zmq::Message::new();
            loop {
                responder.recv(&mut msg, 0)?;
                println!("Received {}", msg.as_str().unwrap());
                responder.send("World", 0)?;
            }
        });

        Ok(Self {
            config,
            stats: Stats::default(),
            task,
        })
    }
}
