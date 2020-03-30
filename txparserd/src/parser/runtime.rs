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

use std::ops::Deref;
use log::*;
use tokio::{
    sync::mpsc,
    task::JoinHandle
};
use futures::{Future, FutureExt};
use diesel::prelude::*;
use diesel::pg::PgConnection;
use txlib::lnpbp::bitcoin::Block;
use super::{Config, Stats, Error, BulkParser, channel::*};
use crate::error::DaemonError;

pub fn run(config: Config, mut input: InputChannel) -> Result<JoinHandle<Result<!, Error>>, Error> {
    let index_conn = PgConnection::establish(&config.db_index_url)?;
    let state_conn = PgConnection::establish(&config.db_state_url)?;

    let mut bulk_parser = BulkParser::restore_or_create(state_conn, index_conn)?;

    let service = Service {
        config,
        bulk_parser,
        input,
        active_req: None,
        stats: Stats::default(),
    };

    let task = tokio::spawn(async move {
        service.run_loop().inspect(|status| {
            match status {
                Ok(_) => panic!("Normally parser thread run loop should never return"),
                Err(err) => error!("Got error from parser run loop: {:?}", err),
            }
        }).await
    });

    info!("Parser thread initialized");

    Ok(task)
}

struct Service {
    config: Config,
    bulk_parser: BulkParser,
    input: InputChannel,
    active_req: Option<u64>,
    stats: Stats,
}

impl Service {
    async fn run_loop(mut self) -> Result<!, Error> {
        while let Some(req) = self.input.rep.recv().await {
            trace!("Received request {}", req);
            let rep = match req.cmd {
                Command::Block(block) =>
                    Reply::Block(self.proc_cmd_blocks(req.id, vec![block])),
                Command::Blocks(blocks) =>
                    Reply::Blocks(self.proc_cmd_blocks(req.id, blocks)),
                // FIXME: support other IPC requests
                _ => Reply::Block(FeedReply::Busy),
                //Command::Status(id) => self.proc_cmd_status(req.id),
                //Command::Statistics => self.proc_cmd_statistics(),
            };
            trace!("Sending {} reply back to input service", rep);
            self.input.req.send(rep).await.map_err(|_| Error::InputThreadDropped)?;
            trace!("Reply sent, waiting for new requests to arrive");
        }
        Err(Error::InputThreadDropped)
    }

    fn proc_cmd_blocks(&mut self, req_id: u64, blocks: Vec<Block>) -> FeedReply {
        trace!("Processing received {} blocks ...", blocks.len());
        let mut active_req = &mut self.active_req;
        if active_req.is_some() {
            trace!("Busy with processing previous request, returning BUSY status");
            return FeedReply::Busy;
        }
        trace!("Sending data to bulk parser ...");
        *active_req = Some(req_id);
        //tokio::spawn(async move {
            self.bulk_parser
                .feed(blocks)
                .inspect(|status| {
                    trace!("Bulk parser has finished processing with {:?} status", status);
                    *active_req = None
                });
        //});
        trace!("Returning CONSUMED status");
        FeedReply::Consumed
    }
}
