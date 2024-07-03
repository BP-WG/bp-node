// BP Node: sovereign bitcoin wallet backend.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2024 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2020-2024 Dr Maxim Orlovsky. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::path::PathBuf;

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
    /// Data location
    pub data_dir: PathBuf,
}

#[cfg(feature = "server")]
impl From<Opts> for Config {
    fn from(opts: Opts) -> Self {
        Config {
            data_dir: opts.data_dir,
        }
    }
}

impl From<bpd::Opts> for Config {
    fn from(opts: bpd::Opts) -> Config {
        let mut config = Config::from(opts.shared);
        config
    }
}

impl Config {
    pub fn set_rpc_endpoint(&mut self, endpoint: ServiceAddr) { self.rpc_endpoint = endpoint; }
}
