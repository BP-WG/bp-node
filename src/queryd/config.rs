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


use std::net::SocketAddr;
use clap::Clap;

use lnpbp::internet::{InetSocketAddr, InetAddr};


const MONITOR_ADDR_DEFAULT: &str = "0.0.0.0:9665";

#[derive(Clap)]
#[clap(
    name = "queryd",
    version = "0.0.1",
    author = "Dr Maxim Orlovsky <orlovsky@pandoracore.com>",
    about =  "BP queryd: Bitcoin blockchain & transaction query daemon; part of Bitcoin network protocol node"
)]
pub struct Opts {
    /// Path and name of the configuration file
    #[clap(short = "c", long = "config", default_value = "wired.toml")]
    pub config: String,

    /// Sets verbosity level; can be used multiple times to increase verbosity
    #[clap(global = true, short = "v", long = "verbose", min_values = 0, max_values = 4, parse(from_occurrences))]
    pub verbose: u8,

    /// IPv4, IPv6 or Tor address to listen for incoming API requests
    #[clap(short = "i", long = "inet-addr", default_value = "0.0.0.0", env="BP_QUERYD_INET_ADDR",
           parse(try_from_str))]
    address: InetAddr,

    /// Use custom port to listen for incoming API requests
    #[clap(short = "a", long = "api-port", default_value = "9713", env="BP_QUERYD_API_PORT")]
    api_port: u16,

    /// Use custom port to listen for incoming API requests
    #[clap(short = "p", long = "push-port", default_value = "9716", env="BP_QUERYD_PUSH_PORT")]
    push_port: u16,

    /// Address for Prometheus monitoring information exporter
    #[clap(short = "m", long = "monitor", default_value = MONITOR_ADDR_DEFAULT, env="BP_QUERYD_MONITOR",
           parse(try_from_str))]
    monitor: SocketAddr,
}


// We need config structure since not all of the parameters can be specified
// via environment and command-line arguments. Thus we need a config file and
// default set of configuration
#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub struct Config {
    pub verbose: u8,
    pub monitor_addr: SocketAddr,
    pub msgbus_peer_api_addr: InetSocketAddr,
    pub msgbus_peer_push_addr: InetSocketAddr,
}

impl From<Opts> for Config {
    fn from(opts: Opts) -> Self {
        Self {
            verbose: opts.verbose,
            monitor_addr: opts.monitor,
            msgbus_peer_api_addr: InetSocketAddr::new(opts.address, opts.api_port),
            msgbus_peer_push_addr: InetSocketAddr::new(opts.address, opts.push_port),
            ..Config::default()
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            verbose: 0,
            monitor_addr: MONITOR_ADDR_DEFAULT.parse().expect("Constant default value parse fail"),
            msgbus_peer_api_addr: InetSocketAddr::default(),
            msgbus_peer_push_addr: InetSocketAddr::default()
        }
    }
}