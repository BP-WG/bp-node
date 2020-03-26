
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
