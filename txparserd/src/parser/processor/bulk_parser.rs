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


use diesel::{
    prelude::*,
    pg::PgConnection
};
use txlib::{
    schema,
    lnpbp::bitcoin::Block
};
use crate::parser;
use super::*;


pub struct BulkParser {
    state: State,
    state_conn: PgConnection,
    index_conn: PgConnection,
}

impl BulkParser {
    pub fn restore(state_conn: PgConnection, index_conn: PgConnection) -> Result<Self, parser::Error> {
        Ok(Self {
            state: State::restore(&state_conn, &index_conn)?,
            state_conn,
            index_conn,
        })
    }

    pub fn init_from_scratch(state_conn: PgConnection, index_conn: PgConnection) -> Self {
        Self {
            state: State::default(),
            state_conn,
            index_conn,
        }
    }

    pub fn feed(&mut self, blocks: Vec<Block>) -> Result<(), Error> {
        let mut ephemeral_state = State::default();

        debug!("Processing {} blocks", blocks.len());
        let block_chain = ephemeral_state.order_blocks(blocks, &self.state);

        trace!("Running block processor for each of the blocks ...");
        let data = block_chain
            .into_iter()
            .try_fold(
                ParseData::init(ephemeral_state.known_height),
                |mut data, block| -> Result<ParseData, Error> {
                    BlockParser::parse(block, &mut data, &self.state.utxo)?;
                    Ok(data)
                }
            )?;

        trace!("Per-block processing has completed; inserting data into database as a transaction ...");
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
                diesel::insert_into(schema::txin::table)
                    .values(data.txins)
                    .execute(&self.index_conn)?;

                // Applying new state as a base state
                trace!("Updating bulk parsing state data");
                state_clone += ephemeral_state;

                state_clone.store(&self.state_conn, &self.index_conn)
            })
        })?;
        self.state = state_clone;


        trace!("Returning success from the bulk parser");
        Ok(())
    }

    /*
    pub fn get_stats(&self) -> State {
        self.state.clone()
    }
    */
}
