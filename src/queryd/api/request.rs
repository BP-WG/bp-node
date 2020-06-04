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


use lnpbp::api::*;
use lnpbp::bp::short_id::ShortId;

use super::*;


#[non_exhaustive]
pub enum Request {
    Txid(VecEncoding<ShortId>),
    SpendingTxin(VecEncoding<ShortId>),
    Utxo(Query),
}

impl TryFrom<Multipart> for Request {
    type Error = Error;

    fn try_from(multipart: Multipart) -> Result<Self, Self::Error> {
        let (cmd, args) = split_cmd_args(&multipart)?;

        Ok(match cmd {
            REQID_QUERY => Request::Query(args.try_into()?),
            _ => Err(Error::UnknownCommand)?,
        })
    }
}

impl From<Request> for Multipart {
    fn from(command: Request) -> Self {
        use Request::*;

        match command {
            Query(query) => vec![
                zmq::Message::from(&REQID_QUERY.to_be_bytes()[..]),
            ].into_iter()
                .chain(Multipart::from(query))
                .collect::<Multipart>(),
            _ => unimplemented!()
        }
    }
}
