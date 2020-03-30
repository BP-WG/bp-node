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


use log::*;
use tokio::{
    sync::mpsc,
    task::JoinHandle
};
use futures::{
    FutureExt,
    TryFutureExt
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
    let socket = config.socket.clone();
    responder.bind(socket.as_str())?;

    let service = Service {
        config,
        stats: Stats::default(),
        responder,
        parser,
    };

    let task = tokio::spawn(async move {
        service.run_loop().await
    });

    info!("Input service is listening for incoming blocks on {}", socket);

    Ok(task)
}

struct Service {
    config: Config,
    stats: Stats,
    responder: zmq::Socket,
    parser: ParserChannel,
}

impl Service {
    async fn run_loop(mut self) -> ! {
        loop {
            self.run().inspect(|status| {
                match status {
                    Ok(_) => debug!("Client request processing completed"),
                    Err(err) => error!("Error processing client's input: {:?}", err),
                }
            }).await;
        }
    }

    async fn run(&mut self) -> Result<(), DaemonError> {
        let mut multipart = self.responder.recv_multipart(0)?;
        trace!("Incoming input message");
        let response = self.proc_cmd(multipart).await?;
        trace!("Received response from command processor: {:?}", response);
        self.responder.send(response, 0).map_err(|_| { DaemonError::IpcSocketError })
    }

    async fn proc_cmd(&mut self, multipart: Vec<Vec<u8>>) -> Result<zmq::Message, DaemonError> {
        use std::str;

        let (command, multipart) = multipart.split_first().ok_or(DaemonError::MalformedMessage)?;
        let cmd = str::from_utf8(&command[..]).map_err(|_| DaemonError::MalformedMessage)?;
        debug!("Processing {} command from client ...", cmd);
        match cmd {
            "BLOCK" => self.proc_cmd_blck(multipart).await,
            "BLOCKS" => self.proc_cmd_blcks(multipart).await,
            // TODO: Add support for other commands
            _ => Err(DaemonError::MalformedMessage),
        }
    }

    async fn proc_cmd_blck(&mut self, multipart: &[Vec<u8>]) -> Result<zmq::Message, DaemonError> {
        let block_data = match (multipart.first(), multipart.len()) {
            (Some(data), 1) => Ok(data),
            (_, _) => Err(DaemonError::MalformedMessage),
        }?;

        let block = deserialize(&block_data[..])?;

        let req = parser::Request { id: 0, cmd: parser::Command::Block(block) };
        self.parser.req.try_send(req).map_err(|_| DaemonError::IpcSocketError)?;
        self.proc_reply_blocks().await
    }

    async fn proc_cmd_blcks(&mut self, multipart: &[Vec<u8>]) -> Result<zmq::Message, DaemonError> {
        let blocks = multipart
            .iter()
            .try_fold::<_, _, Result<Vec<Block>, bitcoin::consensus::encode::Error>>(
                Vec::<Block>::new(),
                |mut vec, block_data| {
                    vec.push(deserialize(&block_data[..])?);
                    Ok(vec)
                })?;

        let req = parser::Request { id: 0, cmd: parser::Command::Blocks(blocks) };
        self.parser.req.send(req).await.map_err(|_| DaemonError::IpcSocketError);
        self.proc_reply_blocks().await
    }

    async fn proc_reply_blocks(&mut self) -> Result<zmq::Message, DaemonError> {
        trace!("Waiting for reply from parser service on block processing ...");
        let parser_reply = self.parser.rep.recv().await.ok_or(DaemonError::IpcSocketError)?;
        trace!("Got {} reply from parser", parser_reply);
        let our_reply = zmq::Message::from(match parser_reply {
            parser::Reply::Block(parser::FeedReply::Consumed)
            | parser::Reply::Blocks(parser::FeedReply::Consumed) => "ACK",
            parser::Reply::Block(parser::FeedReply::Busy)
            | parser::Reply::Blocks(parser::FeedReply::Busy) => "BUSY",
            _ => "ERR",
        });
        trace!("Sending back to client {:?} response", our_reply);
        Ok(our_reply)
    }
}