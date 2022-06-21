// BP Node: bitcoin blockchain indexing and notification service
//
// Written in 2020-2022 by
//     Dr. Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2022 by LNP/BP Standards Association, Switzerland.
//
// You should have received a copy of the MIT License along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use bp_rpc::{Reply, Request};
use internet2::session::LocalSession;
use internet2::{
    CreateUnmarshaller, SendRecvMessage, TypedEnum, Unmarshall, Unmarshaller, ZmqSocketType,
};
use microservices::error::BootstrapError;
use microservices::node::TryService;
use microservices::rpc::ClientError;
use microservices::ZMQ_CONTEXT;

use crate::{Config, DaemonError, LaunchError};

pub fn run(config: Config) -> Result<(), BootstrapError<LaunchError>> {
    let runtime = Runtime::init(config)?;

    runtime.run_or_panic("bpd");

    Ok(())
}

pub struct Runtime {
    /// Original configuration object
    pub(crate) config: Config,

    /// Stored sessions
    pub(crate) session_rpc: LocalSession,

    /// Unmarshaller instance used for parsing RPC request
    pub(crate) unmarshaller: Unmarshaller<Request>,
}

impl Runtime {
    pub fn init(config: Config) -> Result<Self, BootstrapError<LaunchError>> {
        // debug!("Initializing storage provider {:?}", config.storage_conf());
        // let storage = storage::FileDriver::with(config.storage_conf())?;

        debug!("Opening RPC API socket {}", config.rpc_endpoint);
        let session_rpc = LocalSession::connect(
            ZmqSocketType::Rep,
            &config.rpc_endpoint,
            None,
            None,
            &ZMQ_CONTEXT,
        )?;

        info!("bpd runtime started successfully");

        Ok(Self {
            config,
            session_rpc,
            unmarshaller: Request::create_unmarshaller(),
        })
    }
}

impl TryService for Runtime {
    type ErrorType = ClientError;

    fn try_run_loop(mut self) -> Result<(), Self::ErrorType> {
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

impl Runtime {
    fn run(&mut self) -> Result<(), ClientError> {
        trace!("Awaiting for ZMQ RPC requests...");
        let raw = self.session_rpc.recv_raw_message()?;
        let reply = self.rpc_process(raw).unwrap_or_else(|err| err);
        trace!("Preparing ZMQ RPC reply: {:?}", reply);
        let data = reply.serialize();
        trace!("Sending {} bytes back to the client over ZMQ RPC", data.len());
        self.session_rpc.send_raw_message(&data)?;
        Ok(())
    }
}

impl Runtime {
    pub(crate) fn rpc_process(&mut self, raw: Vec<u8>) -> Result<Reply, Reply> {
        trace!("Got {} bytes over ZMQ RPC", raw.len());
        let request = (&*self.unmarshaller.unmarshall(raw.as_slice())?).clone();
        debug!("Received ZMQ RPC request #{}: {}", request.get_type(), request);
        match request {
            Request::Noop => Ok(Reply::Success) as Result<_, DaemonError>,
        }
        .map_err(Reply::from)
    }
}
