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


use tokio::{
    sync::mpsc,
    task::JoinHandle
};
use txlib::lnpbp::bitcoin::{
    self,
    Block,
    consensus::deserialize
};
use super::{Config, Stats, ParserChannel};
use crate::{
    parser,
    error::DaemonError
};

pub fn run(config: Config, mut parser: ParserChannel) -> Result<JoinHandle<Result<!, DaemonError>>, DaemonError> {
    let context = zmq::Context::new();
    let responder = context.socket(zmq::REP).unwrap();
    responder.bind(config.socket.as_str())?;

    let service = Service {
        config,
        stats: Stats::default(),
        responder,
        parser,
    };

    let task = tokio::spawn(async move {
        service.run_loop().await
    });

    Ok(task)
}

struct Service {
    config: Config,
    stats: Stats,
    responder: zmq::Socket,
    parser: ParserChannel,
}

impl Service {
    async fn run_loop(mut self) -> Result<!, DaemonError> {
        loop {
            let mut multipart = self.responder.recv_multipart(0)?;
            let response = self.proc_cmd(multipart).await?;
            self.responder.send(response, 0)?;
        }
    }

    async fn proc_cmd(&mut self, multipart: Vec<Vec<u8>>) -> Result<zmq::Message, DaemonError> {
        use std::str;

        let (command, multipart) = multipart.split_first().ok_or(DaemonError::MalformedMessage)?;
        match str::from_utf8(&command[..]).map_err(|_| DaemonError::MalformedMessage)? {
            "BLOCK" => self.proc_cmd_blck(multipart).await,
            "BLOCKS" => self.proc_cmd_blcks(multipart).await,
            _ => Err(DaemonError::MalformedMessage),
        }
    }

    async fn proc_cmd_blck(&mut self, multipart: &[Vec<u8>]) -> Result<zmq::Message, DaemonError> {
        let block_data = match (multipart.first(), multipart.len()) {
            (Some(data), 0) => Ok(data),
            (_, _) => Err(DaemonError::MalformedMessage),
        }?;

        let block = deserialize(&block_data[..])?;

        let req = parser::Request { id: 0, cmd: parser::Command::Block(block) };
        let rep = self.parser.req.send(req).await.map_err(|_| DaemonError::IpcSocketError);

        let resp = zmq::Message::from("ACK");

        Ok(resp)
    }

    async fn proc_cmd_blcks(&mut self, multipart: &[Vec<u8>]) -> Result<zmq::Message, DaemonError> {
        let blocks = multipart
            .iter()
            .try_fold::<_, _, Result<Vec<Block>, bitcoin::consensus::encode::Error>>(Vec::<Block>::new(), |mut vec, block_data| {
                vec.push(deserialize(&block_data[..])?);
                Ok(vec)
            })?;

        let req = parser::Request { id: 0, cmd: parser::Command::Blocks(blocks) };
        let rep = self.parser.req.send(req).await.map_err(|_| DaemonError::IpcSocketError);

        let resp = zmq::Message::from("ACK");

        Ok(resp)
    }
}