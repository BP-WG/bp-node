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
pub enum Reply {
    Okay,
    Ack,
    Success,
    Done,
    Failure,
}

impl TryFrom<Multipart> for Reply {
    type Error = Error;

    fn try_from(multipart: Multipart) -> Result<Self, Self::Error> {
        let (cmd, args) = multipart.split_first()
            .ok_or(Error::MalformedReply)
            .and_then(|(cmd_data, args)| {
                if cmd_data.len() != 2 {
                    Err(Error::MalformedStatus)?
                }
                let mut buf = [0u8; 2];
                buf.clone_from_slice(&cmd_data[0..2]);
                Ok((u16::from_be_bytes(buf), args))
            })?;

        Ok(match cmd {
            REPID_OKAY => Reply::Okay,
            REPID_ACK => Reply::Ack,
            REPID_SUCCESS => Reply::Success,
            REPID_DONE => Reply::Done,
            REPID_FAILURE => Reply::Failure,
            _ => Err(Error::UnknownStatus)?,
        })
    }
}

impl From<Reply> for Multipart {
    fn from(reply: Reply) -> Self {
        use Reply::*;

        match reply {
            Okay => vec![zmq::Message::from(&REPID_OKAY.to_be_bytes()[..])],
            Ack => vec![zmq::Message::from(&REPID_ACK.to_be_bytes()[..])],
            Success => vec![zmq::Message::from(&REPID_SUCCESS.to_be_bytes()[..])],
            Done => vec![zmq::Message::from(&REPID_DONE.to_be_bytes()[..])],
            Failure => vec![zmq::Message::from(&REPID_FAILURE.to_be_bytes()[..])],
        }
    }
}
