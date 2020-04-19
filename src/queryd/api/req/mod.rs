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

mod query;
pub use query::*;

pub(self) use super::*;


use core::slice::Iter;
use std::marker::PhantomData;

use lnpbp::wrapper;
use lnpbp::bp::short_id::ShortId;
use lnpbp::bitcoin;
use zmq::Message;

pub(super) const REQID_QUERY: u16 = 0x0010;

pub(super) const REPID_OKAY: u16 = 0x0001;
pub(super) const REPID_ACK: u16 = 0x0002;
pub(super) const REPID_SUCCESS: u16 = 0x0003;
pub(super) const REPID_DONE: u16 = 0x0004;
pub(super) const REPID_FAILURE: u16 = 0x0005;

