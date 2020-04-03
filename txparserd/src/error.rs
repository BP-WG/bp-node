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


use std::error::Error;
use tokio::task::JoinError;
use diesel::{
    ConnectionError,
    result::Error as DBError,
};

use crate::parser;

#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum BootstrapError {
    IPCSocketError(zmq::Error, IPCSocket, Option<String>),
    InputSocketError(zmq::Error, APISocket, Option<String>),
    MonitorSocketError(Box<dyn Error>),
    StateDBConnectionError(ConnectionError),
    IndexDBConnectionError(ConnectionError),
    IndexDBIntegrityError,
    IndexDBError(DBError),
    MultithreadError(JoinError)
}

impl From<parser::Error> for BootstrapError {
    fn from(err: parser::Error) -> Self {
        match err {
            parser::Error::IndexDBIntegrityError => BootstrapError::IndexDBIntegrityError,
            parser::Error::IndexDBError(err) => BootstrapError::IndexDBError(err),
            _ => panic!("Incomplete implementation: unsupported bootstrap error (1)"),
        }
    }
}

impl From<JoinError> for BootstrapError {
    fn from(err: JoinError) -> Self {
        BootstrapError::MultithreadError(err)
    }
}

#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum IPCSocket {
    Input2Parser,
    ParserPublisher,
    Monitor2Input,
    Monitor2Parser,
}

#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum APISocket {
    PubSub,
    ReqRep,
}
