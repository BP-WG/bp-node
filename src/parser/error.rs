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


use std::error;
use diesel::result::Error as DieselError;
use lnpbp::bitcoin;


#[derive(PartialEq, Debug, Display)]
#[display_from(Debug)]
pub enum Error {
    BlockchainIndexesOutOfShortIdRanges,
    BlockValidationIncosistency,
    IndexDBIntegrityError,
    IndexDBError(DieselError),
    StateDBError(DieselError),
}

impl error::Error for Error {}

impl From<DieselError> for Error {
    fn from(err: DieselError) -> Self {
        Error::IndexDBError(err)
    }
}

impl From<bitcoin::hashes::Error> for Error {
    fn from(_: bitcoin::hashes::Error) -> Self {
        Error::IndexDBIntegrityError
    }
}
