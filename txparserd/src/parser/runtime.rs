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
use diesel::prelude::*;
use diesel::pg::PgConnection;
use txlib::lnpbp::bitcoin::Block;
use super::{Config, Stats, Error, BulkParser};
use crate::error::DaemonError;

pub fn run(config: Config, mut rx: mpsc::Receiver<Vec<Block>>) -> Result<JoinHandle<Result<!, Error>>, Error> {
    let index_conn = PgConnection::establish(&config.db_index_url)?;
    let state_conn = PgConnection::establish(&config.db_state_url)?;

    let mut bulk_parser = BulkParser::restore_or_create(index_conn, state_conn)?;

    let service = Service {
        config,
        stats: Stats::default(),
        bulk_parser,
        input_receiver: rx,
    };

    let task = tokio::spawn(async move {
        service.run_loop().await
    });

    Ok(task)
}


struct Service {
    config: Config,
    stats: Stats,
    bulk_parser: BulkParser,
    input_receiver: mpsc::Receiver<Vec<Block>>,
}

impl Service {
    async fn run_loop(mut self) -> Result<!, Error> {
        while let Some(blocks) = self.input_receiver.recv().await {
            self.bulk_parser.feed(blocks)?;
        }
        Err(Error::InputThreadDropped)
    }
}
