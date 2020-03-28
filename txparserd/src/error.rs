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


use std::io;
use txlib::lnpbp::bitcoin;
use crate::parser;

#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum DaemonError {
    IoError(io::Error),
    ZmqError(zmq::Error),
    MalformedMessage,
    ConsensusEncodingError(bitcoin::consensus::encode::Error),
    IpcSocketError,
    ParserError(parser::Error),
    HttpMonitoringPortError,
}

impl From<zmq::Error> for DaemonError {
    fn from(err: zmq::Error) -> Self {
        DaemonError::ZmqError(err)
    }
}

impl From<io::Error> for DaemonError {
    fn from(err: io::Error) -> Self {
        DaemonError::IoError(err)
    }
}

impl From<parser::Error> for DaemonError {
    fn from(err: parser::Error) -> Self {
        DaemonError::ParserError(err)
    }
}

impl From<bitcoin::consensus::encode::Error> for DaemonError {
    fn from(err: bitcoin::consensus::encode::Error) -> Self {
        DaemonError::ConsensusEncodingError(err)
    }
}
