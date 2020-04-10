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


use super::*;


#[derive(Clone, Debug, Display)]
#[display_from(Debug)]
#[non_exhaustive]
pub enum Request {
    Query(Query),
}

impl TryFrom<Multipart> for Request {
    type Error = Error;

    fn try_from(multipart: Multipart) -> Result<Self, Self::Error> {
        let (cmd, args) = multipart.split_first()
            .ok_or(Error::MalformedRequest)
            .and_then(|(cmd_data, args)| {
                if cmd_data.len() != 2 {
                    Err(Error::MalformedCommand)?
                }
                let mut buf = [0u8; 2];
                buf.clone_from_slice(&cmd_data[0..2]);
                Ok((u16::from_be_bytes(buf), args))
            })?;

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
        }
    }
}
