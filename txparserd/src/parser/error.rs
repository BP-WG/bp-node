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


use diesel::result::Error as DBError;


#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum BlockFileMalformation {
    WrongMagicNumber,
    NoBlockLen,
    BlockDataCorrupted,
}

#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum Error {
    ParserIPCError(zmq::Error),
    PubIPCError(zmq::Error),
    UknownRequest,
    WrongNumberOfArgs,
    MalformedBlockFile(BlockFileMalformation),
    BlockchainIndexesOutOfShortIdRanges,
    BlockValidationIncosistency,
    IndexDBIntegrityError,
    IndexDBError(DBError),
    StateDBError(DBError),
}

impl std::error::Error for Error {}

impl From<zmq::Error> for Error {
    fn from(err: zmq::Error) -> Self {
        Error::ParserIPCError(err)
    }
}

impl From<DBError> for Error {
    fn from(err: DBError) -> Self {
        Error::IndexDBError(err)
    }
}

impl Into<!> for Error {
    fn into(self) -> ! {
        panic!("Compile-time error (2)");
    }
}