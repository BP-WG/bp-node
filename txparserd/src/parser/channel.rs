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

use tokio::sync::mpsc::{Sender, Receiver};
use txlib::lnpbp::bitcoin::Block;
use super::Stats;


pub struct InputChannel {
    pub req: Receiver<Request>,
    pub rep: Sender<Reply>,
}

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub struct Request {
    pub id: u64,
    pub cmd: Command,
}

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub enum Command {
    Block(Block),
    Blocks(Vec<Block>),
    Status(u64),
    Statistics,
}

#[derive(Clone, Debug, Display)]
#[display_from(Debug)]
pub enum Reply {
    Block(BlocksReply),
    Blocks(BlocksReply),
    Status(StatusReply),
    Statistics(Stats),
}

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub enum BlockReply {
    Consumed,
    Invalid,
    Busy
}

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub enum BlocksReply {
    Consumed {
        chained: u16,
        cached: u16,
        known: u16,
        invalid: u16,
    },
    Busy
}

#[derive(Clone, Debug, Display)]
#[display_from(Debug)]
pub enum StatusReply {
    Active(Stats),
    Completed,
    NotFound,
}
