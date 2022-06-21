// BP Node: bitcoin blockchain indexing and notification service
//
// Written in 2020-2022 by
//     Dr. Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2022 by LNP/BP Standards Association, Switzerland.
//
// You should have received a copy of the MIT License along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use std::path::PathBuf;

use bp_rpc::BP_NODE_RPC_ENDPOINT;
use internet2::addr::ServiceAddr;
use lnpbp::chain::Chain;

#[cfg(feature = "server")]
use crate::bpd;
#[cfg(feature = "server")]
use crate::opts::Opts;

/// Final configuration resulting from data contained in config file environment
/// variables and command-line options. For security reasons node key is kept
/// separately.
#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display(Debug)]
pub struct Config {
    /// ZMQ socket for RPC API
    pub rpc_endpoint: ServiceAddr,

    /// ZMQ socket for RPC API.
    pub ctl_endpoint: ServiceAddr,

    /// ZMQ socket for Store service RPC.
    pub store_endpoint: ServiceAddr,

    /// Data location
    pub data_dir: PathBuf,

    /// URL for the electrum server connection
    pub electrum_url: String,

    /// Indicates whether deamons should be spawned as threads (true) or as child processes (false)
    pub threaded: bool,
}

// TODO: Move to descriptor wallet
fn default_electrum_port(chain: &Chain) -> u16 {
    match chain {
        Chain::Mainnet => 50001,
        Chain::Testnet3 | Chain::Regtest(_) => 60001,
        Chain::Signet | Chain::SignetCustom(_) => 60601,
        Chain::LiquidV1 => 50501,
        Chain::Other(_) => 60001,
        _ => 60001,
    }
}

#[cfg(feature = "server")]
impl From<Opts> for Config {
    fn from(opts: Opts) -> Self {
        let electrum_url = format!(
            "{}:{}",
            opts.electrum_server,
            opts.electrum_port.unwrap_or_else(|| default_electrum_port(&opts.chain))
        );

        Config {
            data_dir: opts.data_dir,
            rpc_endpoint: BP_NODE_RPC_ENDPOINT.parse().expect("error in constant value"),
            ctl_endpoint: opts.ctl_endpoint,
            store_endpoint: opts.store_endpoint,
            electrum_url,
            threaded: true,
        }
    }
}

impl From<bpd::Opts> for Config {
    fn from(opts: bpd::Opts) -> Config {
        let mut config = Config::from(opts.shared);
        config.set_rpc_endpoint(opts.rpc_endpoint);
        config.threaded = opts.threaded_daemons;
        config
    }
}

impl Config {
    pub fn set_rpc_endpoint(&mut self, endpoint: ServiceAddr) { self.rpc_endpoint = endpoint; }
}
