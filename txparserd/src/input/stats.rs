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


use std::collections::HashMap;
use chrono::NaiveDateTime;


#[derive(Clone, Default, Debug, Display)]
#[display_from(Debug)]
pub struct Stats {
    pub totals: StatsBlock,
    pub clients_served: u32,
    pub clients_banned: Vec<Client>,
    pub clients_active: HashMap<Client, StatsBlock>,
}

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub struct Client {
    pub address: String,
    pub connected: NaiveDateTime
}


#[derive(Clone, PartialEq, Eq, Default, Debug, Display)]
#[display_from(Debug)]
pub struct StatsBlock {
    pub requests_processed: u64,
    pub requests_failed: u32,
    pub blocks_processed: u32,
    pub txs_processed: u64,
    pub txins_processed: u64,
    pub txouts_processed: u64,
    pub satoshis_procssed: u64,
    pub bytes_processed: u64,
}
