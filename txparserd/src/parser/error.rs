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


use diesel::result::Error as DbError;
use diesel::ConnectionError;

#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum Error {
    IndexDbIntegrityError,
    BlockchainIndexesOutOfShortIdRanges,
    BlockValidationIncosistency,
    IndexDbError(DbError),
    StateDbError(DbError),
    DbConnectionError(ConnectionError),
    InputThreadDropped,
}

impl From<DbError> for Error {
    fn from(err: DbError) -> Self {
        Error::IndexDbError(err)
    }
}

impl From<ConnectionError> for Error {
    fn from(err: ConnectionError) -> Self {
        Error::DbConnectionError(err)
    }
}
