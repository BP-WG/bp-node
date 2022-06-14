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

use bp_rpc::BPD_RPC_ENDPOINT;
use clap::{Parser, ValueHint};
use internet2::addr::ServiceAddr;

#[cfg(any(target_os = "linux"))]
pub const BPD_DATA_DIR: &str = "~/.bp";
#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
pub const BPD_DATA_DIR: &str = "~/.bp";
#[cfg(target_os = "macos")]
pub const BPD_DATA_DIR: &str = "~/Library/Application Support/BP Node";
#[cfg(target_os = "windows")]
pub const BPD_DATA_DIR: &str = "~\\AppData\\Local\\BP Node";
#[cfg(target_os = "ios")]
pub const BPD_DATA_DIR: &str = "~/Documents";
#[cfg(target_os = "android")]
pub const BPD_DATA_DIR: &str = ".";

pub const BPD_CONFIG: &str = "{data_dir}/bpd.toml";

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
#[clap(author, version, name = "bpd", about = "bp node managing service")]
pub struct Opts {
    /// Set verbosity level.
    ///
    /// Can be used multiple times to increase verbosity
    #[clap(short, long, global = true, parse(from_occurrences))]
    pub verbose: u8,

    /// Data directory path.
    ///
    /// Path to the directory that contains stored data, and where ZMQ RPC
    /// socket files are located
    #[clap(
        short,
        long,
        global = true,
        default_value = BPD_DATA_DIR,
        env = "BPD_DATA_DIR",
        value_hint = ValueHint::DirPath
    )]
    pub data_dir: PathBuf,

    /// ZMQ socket name/address for bp node RPC interface.
    ///
    /// Internal interface for control PRC protocol communications.
    #[clap(
        short = 'x',
        long,
        env = "BPD_RPC_ENDPOINT",
        value_hint = ValueHint::FilePath,
        default_value = BPD_RPC_ENDPOINT
    )]
    pub rpc_endpoint: ServiceAddr,
}
