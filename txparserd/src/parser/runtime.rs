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

use std::io;
use log::*;
use tokio::task::JoinHandle;
use diesel::prelude::*;
use diesel::pg::PgConnection;

use txlib::lnpbp::bitcoin::{
    Block,
    consensus::encode::deserialize,
    network::stream_reader::StreamReader
};
use super::{Config, Stats, error::*, BulkParser};
use crate::{error::*, TryService, INPUT_PARSER_SOCKET, PARSER_PUB_SOCKET};

pub fn run(config: Config, context: &mut zmq::Context)
           -> Result<Vec<JoinHandle<!>>, BootstrapError>
{
    // Connecting to the database
    let index_conn = PgConnection::establish(&config.db_index_url)
        .map_err(|e| BootstrapError::IndexDBConnectionError(e))?;
    debug!("Index database connected");
    let state_conn = PgConnection::establish(&config.db_state_url)
        .map_err(|e| BootstrapError::StateDBConnectionError(e))?;
    debug!("State database connected");

    // Initializing parser
    let parser = BulkParser::restore_or_create(state_conn, index_conn)?;
    debug!("Parser state is restored");

    // Opening IPC REQ/REP communication socket with input thread
    let input = context.socket(zmq::REP)
        .map_err(|e| BootstrapError::IPCSocketError(e, IPCSocket::Input2Parser, None))?;
    input.connect(INPUT_PARSER_SOCKET)
        .map_err(|e| BootstrapError::IPCSocketError(e, IPCSocket::Input2Parser,
                                                    Some(INPUT_PARSER_SOCKET.into())))?;
    debug!("IPC ZMQ from Input to Parser threads is opened on Parser runtime side");

    // Opening IPC PUB/SUB publishing socket notifying about parser status changes
    let publisher = context.socket(zmq::PUB)
        .map_err(|e| BootstrapError::IPCSocketError(e, IPCSocket::ParserPublisher, None))?;
    publisher.bind(PARSER_PUB_SOCKET)
        .map_err(|e| BootstrapError::IPCSocketError(e, IPCSocket::ParserPublisher,
                                                    Some(PARSER_PUB_SOCKET.into())))?;
    debug!("IPC ZMQ Parser PUB socket is opened");

    let parser_service = ParserService::init(
        config,
        parser,
        publisher,
        input
    );

    Ok(vec![
        tokio::spawn(async move {
            info!("Parser service is running");
            parser_service.run_or_panic("Parser service").await
        }),
    ])
}

struct ParserService {
    config: Config,
    parser: BulkParser,
    input: zmq::Socket,
    publisher: zmq::Socket,
    error: Option<Error>,
    stats: Stats,
}

#[async_trait]
impl TryService for ParserService {
    type ErrorType = Error;

    async fn try_run_loop(mut self) -> Result<!, Error> {
        loop {
            self.run().await?
        }
    }
}

impl ParserService {
    pub fn init(config: Config,
                parser: BulkParser,
                publisher: zmq::Socket,
                input: zmq::Socket) -> Self {
        Self {
            config,
            parser,
            input,
            publisher,
            error: None,
            stats: Stats::default(),
        }
    }

    async fn run(&mut self) -> Result<(), Error> {
        let multipart = self.input.recv_multipart(0)?;

        trace!("Input request");
        if let Some(err) = &self.error {
            trace!("Returning immediately error from previous parse operation {}", err);
            self.error = None;
            return self.input.send(zmq::Message::from("ERR"), 0)
                .map_err(|e| Error::ParserIPCError(e))
        }

        self.proc_cmd(multipart)
            .await
            .or_else(|err| {
                trace!("Received error status from command processor: {}", err);
                self.input.send(zmq::Message::from("ERR"), 0)
                    .map_err(|e| Error::ParserIPCError(e))
            })
    }

    async fn proc_cmd(&mut self, multipart: Vec<Vec<u8>>) -> Result<(), Error> {
        use std::str;

        let (command, multipart) = multipart.split_first()
            .ok_or(Error::WrongNumberOfArgs)?;
        let cmd = str::from_utf8(&command[..]).map_err(|_| Error::UknownRequest)?;
        debug!("Processing {} command from input thread ...", cmd);
        match cmd {
            "BLOCK" => self.proc_cmd_blck(multipart, false).await,
            "BLOCKS" => self.proc_cmd_blck(multipart, true).await,
            // TODO: Add support for other commands
            _ => Err(Error::UknownRequest),
        }
    }

    async fn proc_cmd_blck(&mut self, multipart: &[Vec<u8>], multiple: bool) -> Result<(), Error> {
        let block_data = match (multipart.first(), multipart.len()) {
            (Some(data), 1) => Ok(data),
            (_, _) => Err(Error::WrongNumberOfArgs),
        }?;

        // Ignoring the result
        let _ = self.async_block_proc(block_data, multiple).await
            .or_else(|error| -> Result<(), Error> {
                error!("Error parsing block data: {}", error);
                self.error = Some(error);

                trace!("Sending error notification on failed parse");
                if let Err(err) = self.publisher.send(zmq::Message::from("ERR"), 0) {
                    // We can't gracefullt handle the error at this stage
                    panic!("Broken IPC communications, failing: {}", err);
                }

                Ok(())
            });

        // We can't fire error here since response to the client is already sent via REQ/REP socket
        Ok(())
    }

    async fn async_block_proc(&mut self, block_data: &Vec<u8>, multiple: bool) -> Result<(), Error> {
        trace!("Replying to input thread");
        self.input.send(zmq::Message::from("ACK"), 0)
            .map_err(|e| Error::ParserIPCError(e))?;

        trace!("Parsing received block data, {} bytes; deserializing", block_data.len());
        let blocks = match multiple {
            true => self.parse_block_file(block_data)?,
            false => vec![deserialize::<Block>(block_data)
                .map_err(|_| Error::BlockValidationIncosistency)?],
        };

        trace!("Parsing received {} blocks ...", blocks.len());
        let res = self.parser.feed(blocks);

        trace!("Parse task completed with {:?} result", res);
        let reply = match res {
            Ok(_) => "RDY",
            Err(_) => "ERR",
        };
        trace!("Sending `{}` notification on complete parse", reply);
        self.publisher.send(zmq::Message::from(reply), 0)
            .map_err(|e| Error::PubIPCError(e))
    }

    fn parse_block_file(&self, block_data: &Vec<u8>) -> Result<Vec<Block>, Error> {
        let mut stream_reader = StreamReader::new(
            io::BufReader::new(&block_data[..]),
            Some(block_data.len())
        );

        let mut blocks: Vec<Block> = Vec::new();
        loop {
            // Checking magic number
            match stream_reader.read_next::<u32>() {
                Ok(0xD9B4BEF9) => Ok(()),
                Err(_) => break,
                _ => Err(Error::MalformedBlockFile(BlockFileMalformation::WrongMagicNumber))
            }?;

            // Skipping block length
            let block_len = stream_reader.read_next::<u32>()
                .map_err(|_| Error::MalformedBlockFile(BlockFileMalformation::NoBlockLen))?;

            // Reading block
            let block = stream_reader.read_next::<Block>()
                .map_err(|_| Error::MalformedBlockFile(BlockFileMalformation::BlockDataCorrupted))?;

            blocks.push(block);
        }

        Ok(blocks)
    }
}
