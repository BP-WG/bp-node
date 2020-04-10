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


use std::io;
use diesel::ConnectionError;
use diesel::result::Error as DieselError;

use lnpbp::bitcoin;

use crate::parser;


#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum Error {
    IOError(io::Error),
    CorruptedShortId,
    CurruptBlockFile,
    BlockValidationIncosistency,
    StateDBConnectionError(ConnectionError),
    IndexDBConnectionError(ConnectionError),
    IndexDBIntegrityError,
    IndexDBError(DieselError),
    StateDBError(DieselError),
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IOError(err)
    }
}

impl From<DieselError> for Error {
    fn from(err: DieselError) -> Self {
        Error::StateDBError(err)
    }
}

impl From<bitcoin::consensus::encode::Error> for Error {
    fn from(err: bitcoin::consensus::encode::Error) -> Self {
        Error::CurruptBlockFile
    }
}

impl From<bitcoin::hashes::Error> for Error {
    fn from(_: bitcoin::hashes::Error) -> Self {
        Error::IndexDBIntegrityError
    }
}

impl From<parser::Error> for Error {
    fn from(err: parser::Error) -> Self {
        use parser::Error::*;
        match err {
            IndexIntegrityError => Error::IndexDBIntegrityError,
            IndexError(e) => Error::IndexDBError(e),
            CorruptedShortId => Error::CorruptedShortId,
            BlockValidationIncosistency => Error::BlockValidationIncosistency,
        }
    }
}