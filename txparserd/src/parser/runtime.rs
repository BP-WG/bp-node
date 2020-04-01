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

use std::{
    io,
    ops::Deref,
};
use log::*;
use tokio::{
    sync::mpsc,
    task::JoinHandle
};
use futures::{Future, FutureExt};
use diesel::prelude::*;
use diesel::pg::PgConnection;

use txlib::lnpbp::bitcoin::{
    Block,
    consensus::encode::deserialize,
    network::stream_reader::StreamReader
};
use super::{Config, Stats, error::*, BulkParser, channel::*};
use crate::{input, error::*, TryService, INPUT_PARSER_SOCKET};

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
    let mut parser = BulkParser::restore_or_create(state_conn, index_conn)?;
    debug!("Parser state is restored");

    // Opening IPC socket to input thread
    let input = context.socket(zmq::REP)
        .map_err(|e| BootstrapError::IPCSocketError(e, IPCSocket::Input2Parser, None))?;
    input.connect(INPUT_PARSER_SOCKET)
        .map_err(|e| BootstrapError::IPCSocketError(e, IPCSocket::Input2Parser,
                                                    Some(String::from(INPUT_PARSER_SOCKET))))?;
    debug!("IPC ZMQ from Input to Parser threads is opened on Parser runtime side");

    let parser_service = ParserService::init(
        config,
        parser,
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
                input: zmq::Socket) -> Self {
        Self {
            config,
            parser,
            input,
            stats: Stats::default(),
        }
    }

    async fn run(&mut self) -> Result<(), Error> {
        let mut multipart = self.input.recv_multipart(0)?;

        trace!("Incoming input API request");
        let response = self.proc_cmd(multipart)
            .await
            .or::<Error>(Ok(zmq::Message::from("ERR")))
            .into_ok();
        trace!("Received response from command processor: {:?}", response);
        self.input.send_msg(response, 0);
        Ok(())
    }

    async fn proc_cmd(&mut self, multipart: Vec<Vec<u8>>) -> Result<zmq::Message, Error> {
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

    async fn proc_cmd_blck(&mut self, multipart: &[Vec<u8>], multiple: bool) -> Result<zmq::Message, Error> {
        let block_data = match (multipart.first(), multipart.len()) {
            (Some(data), 1) => Ok(data),
            (_, _) => Err(Error::WrongNumberOfArgs),
        }?;

        let blocks = match multiple {
            true => self.parse_block_file(block_data)?,
            false => vec![deserialize::<Block>(block_data)
                .map_err(|_| Error::BlockValidationIncosistency)?],
        };

        trace!("Processing received {} blocks ...", blocks.len());
        self.parser.feed(blocks)?;

        trace!("Bulk parser has finished processing");
        Ok(zmq::Message::from("OK"))
    }

    fn parse_block_file(&self, block_data: &Vec<u8>) -> Result<Vec<Block>, Error> {
        trace!("Parsing received block data, {} bytes", block_data.len());
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
