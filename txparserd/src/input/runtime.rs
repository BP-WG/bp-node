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


use std::sync::Arc;
use log::*;
use tokio::{
    sync::{mpsc::Sender, Mutex},
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

pub fn run(config: Config, mut parser: ParserChannel)
    -> Result<JoinHandle<Result<!, DaemonError>>, DaemonError>
{
    let context = zmq::Context::new();

    let req_socket_addr = config.req_socket.clone();
    let pub_socket_addr = config.pub_socket.clone();

    let responder = context.socket(zmq::REP)?;
    responder.bind(req_socket_addr.as_str())?;

    let publisher = context.socket(zmq::PUB)?;
    publisher.bind(pub_socket_addr.as_str())?;

    let busy = Arc::new(Mutex::new(false));
    let busy2 = busy.clone();

    let service = Service {
        config,
        stats: Stats::default(),
        responder,
        parser: parser.req,
        busy,
    };

    let mut parser_rep = parser.rep;
    tokio::spawn(async move {
        info!("Parser status notification service publishes data to {}", pub_socket_addr);
        loop {
            let resp = parser_rep.recv().await;
            trace!("Parser has completed with response {:?}", resp);
            *busy2.lock().await = false;
            publisher.send(zmq::Message::from("RDY"), 0);
        }
    });

    let task = tokio::spawn(async move {
        info!("Input service is listening for incoming blocks on {}", req_socket_addr);
        service.run_loop().await
    });

    Ok(task)
}

struct Service {
    config: Config,
    stats: Stats,
    responder: zmq::Socket,
    parser: Sender<parser::Request>,
    busy: Arc<Mutex<bool>>,
}

impl Service {
    async fn run_loop(mut self) -> ! {
        loop {
            match self.run().await {
                Ok(_) => debug!("Client request processing completed"),
                Err(err) => {
                    self.responder.send(zmq::Message::from("ERR"), 0);
                    error!("Error processing client's input: {:?}", err)
                },
            }
        }
    }

    async fn run(&mut self) -> Result<(), DaemonError> {
        let mut multipart = self.responder.recv_multipart(0)?;
        trace!("Incoming input message");
        let response = self.proc_cmd(multipart).await?;
        if !response.is_empty() {
            trace!("Received response from command processor: {:?}", response);
            self.responder.send(response, 0).map_err(|_| { DaemonError::IpcSocketError })
        } else {
            Ok(())
        }
    }

    async fn proc_cmd(&mut self, multipart: Vec<Vec<u8>>) -> Result<zmq::Message, DaemonError> {
        use std::str;

        let (command, multipart) = multipart.split_first()
            .ok_or(DaemonError::MalformedMessage)?;
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
        self.do_parser_blocks_req(req).await

        //self.parser.req.try_send(req).map_err(|_| DaemonError::IpcSocketError)?;
        //self.proc_reply_blocks().await
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
        self.do_parser_blocks_req(req).await

        //self.parser.req.try_send(req).map_err(|_| DaemonError::IpcSocketError);
        //self.proc_reply_blocks().await
    }

    async fn do_parser_blocks_req(&mut self, req: parser::Request) -> Result<zmq::Message, DaemonError> {
        if *self.busy.lock().await {
            self.parser.send(parser::Request{ id: 1, cmd: parser::Command::Statistics }).await
                .map_err(|_| DaemonError::IpcSocketError)?;
            trace!("Parser service is still busy, returning client `BUSY` status");
            return Ok(zmq::Message::from("BUSY"));
        }

        trace!("Sending block data to parser service");
        *self.busy.lock().await = true;
        self.parser.send(req).await.map_err(|_| DaemonError::IpcSocketError)?;

        trace!("Sending back to client `ACK` response");
        Ok(zmq::Message::from("ACK"))
    }
}