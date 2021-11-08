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

use crate::msgbus::{Error, Multipart, Query};

use internet2::addr::InetSocketAddrExt;
use microservices::node::TryService;

use super::*;
use crate::BootstrapError;

pub struct ApiService {
    config: Config,
    context: zmq::Context,
    subscriber: zmq::Socket,
}

impl TryService for ApiService {
    type ErrorType = Error;

    fn try_run_loop(mut self) -> Result<(), Error> {
        loop {
            match self.run() {
                Ok(_) => debug!("API request processing complete"),
                Err(err) => {
                    error!("Error processing API request: {}", err);
                    Err(err)?;
                }
            }
        }
    }
}

impl ApiService {
    pub fn init(config: Config, context: zmq::Context) -> Result<Self, BootstrapError> {
        trace!("Opening API socket on {} ...", config.socket_addr);
        let addr = InetSocketAddrExt::tcp(config.socket_addr.address, config.socket_addr.port);
        let subscriber = context
            .socket(zmq::REP)
            .map_err(|e| BootstrapError::SubscriptionError(e))?;
        subscriber
            .connect(&addr.to_string())
            .map_err(|e| BootstrapError::SubscriptionError(e))?;
        //subscriber.set_subscribe("".as_bytes())
        //    .map_err(|e| BootstrapError::SubscriptionError(e))?;
        debug!("API socket opened");

        Ok(Self {
            config,
            context,
            subscriber,
        })
    }

    fn run(&mut self) -> Result<(), Error> {
        let req: Multipart = self
            .subscriber
            .recv_multipart(0)
            .map_err(|err| Error::MessageBusError(err))?
            .into_iter()
            .map(zmq::Message::from)
            .collect();
        trace!("New API request");

        trace!("Received API request {:x?}, processing ... ", req[0]);
        let reply = self
            .proc_command(req)
            .unwrap_or(Reply::Failure);

        trace!(
            "Received response from command processor: `{}`; replying to client",
            reply
        );
        self.subscriber
            .send_multipart(Multipart::from(Reply::Success), 0)?;
        debug!("Sent reply {}", Reply::Success);

        Ok(())
    }

    fn proc_command(&mut self, req: Multipart) -> Result<Reply, Error> {
        use Request::*;

        let command = Request::try_from(req)?;

        match command {
            Utxo(query) => self.command_query(query),
            _ => Err(Error::UnknownCommand),
        }
    }

    fn command_query(&mut self, query: Query) -> Result<Reply, Error> {
        debug!("Got QUERY {}", query);

        // TODO: Do query processing

        Ok(Reply::Success)
    }
}
