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


use diesel::{
    prelude::*,
    pg::PgConnection
};
use bitcoin::Block;

use crate::db::schema;
use super::*;


pub struct BulkParser {
    pub state: State,
    pub state_conn: PgConnection,
    pub index_conn: PgConnection,
}

impl BulkParser {
    // TODO: Remove state connection
    pub fn init_from_scratch(state_conn: PgConnection, index_conn: PgConnection) -> Self {
        Self {
            state: State::default(),
            state_conn,
            index_conn,
        }
    }

    pub fn feed(&mut self, blocks: Vec<Block>) -> Result<(), Error> {
        let mut ephemeral_state = State::inherit_state(&self.state);

        debug!("Processing {} blocks", blocks.len());
        let block_chain = ephemeral_state.order_blocks(blocks, &self.state);

        trace!("Running block processor for each of the blocks ...");
        let data = block_chain
            .into_iter()
            .try_fold(
                ParseData::init(ephemeral_state),
                |mut data, block| -> Result<ParseData, Error> {
                    BlockParser::parse(block, &mut data, &self.state.utxo)?;
                    Ok(data)
                }
            )?;
        trace!("{}", data);

        trace!("Per-block processing has completed; inserting data into database ...");
        let mut state_clone = self.state.clone();
        self.state_conn.transaction(|| {
            self.index_conn.transaction(|| {
                let data = data.clone();
                diesel::insert_into(schema::block::table)
                    .values(data.blocks)
                    .execute(&self.index_conn)?;
                diesel::insert_into(schema::tx::table)
                    .values(data.txs)
                    .execute(&self.index_conn)?;
                diesel::insert_into(schema::txout::table)
                    .values(data.txouts)
                    .execute(&self.index_conn)?;
                let r = diesel::insert_into(schema::txin::table)
                    .values(data.txins)
                    .execute(&self.index_conn);

                // Applying new state as a base state
                trace!("Updating bulk parsing state data");
                state_clone += data.state;

                // TODO: Move state storage transaction to indexer module
                // state_clone.store(&self.state_conn, &self.index_conn)
                r
            })
        })?;
        self.state = state_clone;
        trace!("{}", self.state);

        trace!("Returning success from the bulk parser");
        Ok(())
    }

    pub fn clear_database(&mut self) -> Result<(), Error> {
        debug!("Deleting all data from the index database");

        self.index_conn.transaction(|| {
            trace!("Deleting all data from blocks table");
            diesel::delete(schema::block::table)
                .execute(&self.index_conn)?;

            trace!("Deleting all data from transactions table");
            diesel::delete(schema::tx::table)
                .execute(&self.index_conn)?;

            trace!("Deleting all data from transaction outputs table");
            diesel::delete(schema::txout::table)
                .execute(&self.index_conn)?;

            trace!("Deleting all data from transaction inputs table");
            diesel::delete(schema::txin::table)
                .execute(&self.index_conn)
        })?;

        trace!("Clearing state & cache data");
        self.state = State::default();

        Ok(())
    }

    /*
    pub fn get_stats(&self) -> State {
        self.state.clone()
    }
    */
}
