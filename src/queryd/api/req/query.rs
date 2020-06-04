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


use std::convert::TryFrom;

use lnpbp::rpc::{Multipart, Error};

use super::*;


#[derive(Clone, Debug, Display)]
#[display_from(Debug)]
pub struct Query {
    pub query: String,
}

impl TryFrom<&[zmq::Message]> for Query {
    type Error = Error;

    fn try_from(args: &[zmq::Message]) -> Result<Self, Self::Error> {
        if args.len() != 1 { Err(Error::WrongNumberOfArguments)? }

        let query = String::from_utf8(args[0][..].to_vec())
            .map_err(|_| Error::MalformedArgument)?;

        Ok(Self {
            query
        })
    }
}

impl From<Query> for Multipart {
    fn from(proc: Query) -> Self {
        vec![
            zmq::Message::from(&proc.query),
        ]
    }
}
