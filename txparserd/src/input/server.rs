use tokio::task::JoinHandle;

use super::*;

pub struct Server {
    config: Config,
    stats: Stats,
    pub task: JoinHandle<Result<!, Error>>
}

impl Server {
    pub fn init_and_run(config: Config) -> Result<Self, Error> {
        let context = zmq::Context::new();
        let responder = context.socket(zmq::REP).unwrap();

        assert!(responder.bind("tcp://*:5555").is_ok());
        println!("Listening on 127.0.0.1:5555");

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

    pub fn get_stats(&self) -> Stats {
        self.stats.clone()
    }
}