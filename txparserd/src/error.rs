
use std::io;

#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum Error {
    IoError(io::Error),
    ZmqError(zmq::Error)
}

impl From<zmq::Error> for Error {
    fn from(err: zmq::Error) -> Self {
        Error::ZmqError(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}
