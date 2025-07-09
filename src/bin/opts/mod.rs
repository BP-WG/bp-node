// BP Node: sovereign bitcoin wallet backend.
//
// SPDX-License-Identifier: Apache-2.0
//
// Designed & written in 2020-2025 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2025 LNP/BP Labs, InDCS, Switzerland. All rights reserved.
// Copyright (C) 2020-2025 Dr Maxim Orlovsky. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//        http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
// or implied. See the License for the specific language governing permissions and limitations under
// the License.

use std::net::SocketAddr;
use std::path::PathBuf;

use bpstd::Network;
use clap::ValueHint;

use crate::bpnode::Config;

pub const BP_NODE_CONFIG: &str = "{data_dir}/bp-node.toml";
pub const BP_NETWORK_ENV: &str = "BP_NETWORK";
pub const BP_NO_NETWORK_PREFIX_ENV: &str = "BP_NO_NETWORK_PREFIX";

pub const BP_DATA_DIR_ENV: &str = "BP_DATA_DIR";
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
pub const BP_DATA_DIR: &str = "~/.local/share/bp-node";
#[cfg(target_os = "macos")]
pub const BP_DATA_DIR: &str = "~/Library/Application Support/BP Node";
#[cfg(target_os = "windows")]
pub const BP_DATA_DIR: &str = "~\\AppData\\Local\\BP Node";
#[cfg(target_os = "ios")]
pub const BP_DATA_DIR: &str = "~/Documents";
#[cfg(target_os = "android")]
pub const BP_DATA_DIR: &str = ".";

// Uses XDG_DATA_HOME if set, otherwise falls back to RGB_DATA_DIR
fn default_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("bp-node")
    } else {
        PathBuf::from(BP_DATA_DIR)
    }
}

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Eq, PartialEq, Debug)]
#[command(author, version, about)]
pub struct Opts {
    /// Set a verbosity level
    ///
    /// Can be used multiple times to increase verbosity.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Location of the data directory
    #[clap(
        short,
        long,
        global = true,
        default_value_os_t = default_data_dir(),
        env = BP_DATA_DIR_ENV,
        value_hint = ValueHint::DirPath
    )]
    pub data_dir: PathBuf,

    /// Bitcoin network
    #[arg(short, long, global = true, default_value = "testnet4", env = BP_NETWORK_ENV)]
    pub network: Network,

    /// Do not add network name as a prefix to the data directory
    #[arg(long, global = true, env = BP_NO_NETWORK_PREFIX_ENV)]
    pub no_network_prefix: bool,

    /// Address(es) to listen for client RPC connections
    #[arg(short, long, default_value = "127.0.0.1:4250")] // Port - ASCII 'BP'
    pub listen: Vec<SocketAddr>,

    /// Address(es) to listen for block provider connections
    #[arg(short, long, default_value = "127.0.0.1:42500")]
    pub blocks: Vec<SocketAddr>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Command {
    Init,
}

impl Opts {
    pub fn process(&mut self) {
        self.data_dir =
            PathBuf::from(shellexpand::tilde(&self.data_dir.display().to_string()).to_string());
    }

    pub fn base_dir(&self) -> PathBuf {
        let mut dir = self.data_dir.clone();
        if !self.no_network_prefix {
            dir.push(self.network.to_string());
        }
        dir
    }

    pub fn conf_path(&self, name: &'static str) -> PathBuf {
        let mut conf_path = self.base_dir();
        conf_path.push(name);
        conf_path.set_extension("toml");
        conf_path
    }
}

impl From<Opts> for Config {
    fn from(opts: Opts) -> Self {
        Config {
            data_dir: opts.data_dir,
            network: opts.network,
            rpc: opts.listen,
            import: opts.blocks,
        }
    }
}
