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


use std::convert::TryFrom;
use futures::TryFutureExt;

use lnpbp::rpc::{Multipart, Error};
use lnpbp::TryService;
use amplify::internet::InetSocketAddrExt;

use crate::BootstrapError;
use super::*;


pub struct ApiService {
    config: Config,
    context: zmq::Context,
    subscriber: zmq::Socket,
}

#[async_trait]
impl TryService for ApiService {
    type ErrorType = Error;

    async fn try_run_loop(mut self) -> Result<!, Error> {
        loop {
            match self.run().await {
                Ok(_) => debug!("API request processing complete"),
                Err(err) => {
                    error!("Error processing API request: {}", err);
                    Err(err)?;
                },
            }
        }
    }
}

impl ApiService {
    pub fn init(config: Config,
                context: zmq::Context
    ) -> Result<Self, BootstrapError> {
        trace!("Opening API socket on {} ...", config.socket_addr);
        let addr = InetSocketAddrExt::tcp(config.socket_addr.address, config.socket_addr.port);
        let subscriber = context.socket(zmq::REP)
            .map_err(|e| BootstrapError::SubscriptionError(e))?;
        subscriber.connect(&addr.to_string())
            .map_err(|e| BootstrapError::SubscriptionError(e))?;
        //subscriber.set_subscribe("".as_bytes())
        //    .map_err(|e| BootstrapError::SubscriptionError(e))?;
        debug!("API sucket opened");

        Ok(Self {
            config,
            context,
            subscriber
        })
    }

    async fn run(&mut self) -> Result<(), Error> {
        let req: Multipart = self.subscriber
            .recv_multipart(0)
            .map_err(|err| Error::SocketError(err))?
            .into_iter()
            .map(zmq::Message::from)
            .collect();
        trace!("New API request");

        trace!("Received API request {:x?}, processing ... ", req[0]);
        let reply = self.proc_command(req)
            .inspect_err(|err| error!("Error processing request: {}", err))
            .await
            .unwrap_or(Reply::Failure);

        trace!("Received response from command processor: `{}`; replying to client", reply);
        self.subscriber.send_multipart(Multipart::from(Reply::Success), 0)?;
        debug!("Sent reply {}", Reply::Success);

        Ok(())
    }

    async fn proc_command(&mut self, req: Multipart) -> Result<Reply, Error> {
        use Request::*;

        let command = Request::try_from(req)?;

        match command {
            Utxo(query) => self.command_query(query).await,
            _ => Err(Error::UnknownCommand)
        }
    }

    async fn command_query(&mut self, query: Query) -> Result<Reply, Error> {
        debug!("Got QUERY {}", query);

        // TODO: Do query processing

        Ok(Reply::Success)
    }
}
