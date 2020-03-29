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
    Block(FeedReply),
    Blocks(FeedReply),
    Status(StatusReply),
    Statistics(Stats),
}

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub enum FeedReply {
    Consumed,
    Busy
}

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub struct FeedStatus {
    pub chained: u16,
    pub cached: u16,
    pub known: u16,
    pub invalid: u16,
}

#[derive(Clone, Debug, Display)]
#[display_from(Debug)]
pub enum StatusReply {
    Active(FeedStatus),
    Completed,
    NotFound,
}
