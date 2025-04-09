// Bitcoin Core blocks provider for BP Node
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2025 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2025 LNP/BP Labs, InDCS, Switzerland. All rights reserved.
// Copyright (C) 2025 Dr Maxim Orlovsky. All rights reserved.
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

use std::any::Any;
use std::path::PathBuf;

use bprpc::RemoteAddr;
use clap::Parser;

pub const BLOCK_SEPARATOR: u32 = 0xD9B4BEF9;

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Eq, PartialEq, Debug)]
#[command(author, version, about)]
pub struct Opts {
    /// Data directory for Bitcoin Core blocks
    #[arg(short, long)]
    pub data_dir: PathBuf,

    /// Bitcoin Core RPC address.
    pub bitcoin_core: RemoteAddr,

    /// BP Node block import interface address.
    pub bp_node: RemoteAddr,
}

fn main() -> Result<(), Box<dyn Any>> { Ok(()) }
