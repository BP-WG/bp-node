// Bitcoin protocol (BP) daemon node
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


use std::{io, fs};
use diesel::prelude::*;
use diesel::pg::PgConnection;
use lnpbp::service::*;
use lnpbp::bitcoin::{
    Block,
    network::stream_reader::StreamReader
};

use super::*;
use crate::parser::BulkParser;


pub struct Runtime {
    config: Config,
    parser: BulkParser,
    blckfile_no: u16,
}

impl Runtime {
    pub fn init(config: Config) -> Result<Self, Error> {
        // TODO: Check directory

        // Connecting to the database
        let index_conn = PgConnection::establish(&config.index_db)
            .map_err(|e| Error::IndexDBConnectionError(e))?;
        info!("Index database connected");
        let state_conn = PgConnection::establish(&config.state_db)
            .map_err(|e| Error::StateDBConnectionError(e))?;
        info!("State database connected");

        let parser = BulkParser::restore(state_conn, index_conn)?;
        info!("Parser state is restored");

        Ok(Self {
            config,
            parser,
            blckfile_no: 0
        })
    }
}

#[async_trait]
impl TryService for Runtime {
    type ErrorType = Error;

    async fn try_run_loop(mut self) -> Result<!, Self::ErrorType> {
        loop {
            self.parse_block_file()?;
        }
    }
}

impl Runtime {
    pub fn clear_db(&mut self) -> Result<(), Error> {
        info!("Clearing database data");
        self.parser.clear_database()?;
        Ok(())
    }

    fn parse_block_file(&mut self) -> Result<(), Error> {
        let blckfile_name = format!("{}/blocks/blk{:05}.dat", self.config.data_dir, self.blckfile_no);
        info!("Reading blocks from {} ...", blckfile_name);

        let blckfile = fs::File::open(blckfile_name)?;

        let mut stream_reader = StreamReader::new(io::BufReader::new(blckfile), None);
        self.blckfile_no += 1;

        let mut blocks: Vec<Block> = vec![];
        loop {
            // Reading magick number
            match stream_reader.read_next::<u32>() {
                Ok(0xD9B4BEF9) => (),
                _ => {
                    error!("No magick number found");
                    break;
                }
            }
            // Skipping block length
            let block_len = stream_reader.read_next::<u32>();
            // Reading block
            blocks.push(stream_reader.read_next::<Block>()?);
        }
        info!("Block file parse complete: {} blocks read", blocks.len());

        trace!("Parsing received {} blocks ...", blocks.len());
        let res = self.parser.feed(blocks);

        Ok(())
    }
}
