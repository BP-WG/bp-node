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
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//        http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
// or implied. See the License for the specific language governing permissions and limitations under
// the License.

#[macro_use]
extern crate strict_encoding;

use std::io::{Read, Seek};
use std::path::PathBuf;
use std::process::exit;
use std::{fs, io, thread};

use bc::{Block, ConsensusDecode};
use bpclient::exporter::BlockExporter;
use bpclient::rpc::{AgentInfo, ExporterPub, RemoteAddr, Version};
use clap::Parser;
use invoice::Network;
use loglevel::LogLevel;
use netservices::client::Client;
use strict_encoding::Ident;

pub const AGENT: &str = "BC_BP";

pub const BITCOIN_BLOCK_SEPARATOR: [u8; 4] = [0xF9, 0xBE, 0xB4, 0xD9];
pub const TESTNET_BLOCK_SEPARATOR: [u8; 4] = [0x0B, 0x11, 0x09, 0x07];
pub const TESTNET4_BLOCK_SEPARATOR: [u8; 4] = [0x1c, 0x16, 0x3f, 0x28];
pub const SIGNET_BLOCK_SEPARATOR: [u8; 4] = [0x0A, 0x03, 0xCF, 0x40];
pub const REGTEST_BLOCK_SEPARATOR: [u8; 4] = [0xFA, 0xBF, 0xB5, 0xDA];

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Eq, PartialEq, Debug)]
#[command(author, version, about)]
pub struct Args {
    /// Set verbosity level
    ///
    /// Can be used multiple times to increase verbosity.
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[arg(short, long, default_value = "testnet4")]
    pub network: Network,

    /// Data directory for Bitcoin Core blocks
    #[arg(short, long, default_value = "/var/lib/bitcoin/blocks")]
    pub data_dir: PathBuf,

    /// Bitcoin Core RPC address
    #[arg(long, default_value = "127.0.0.1:8332")]
    pub bitcoin_core: RemoteAddr,

    /// BP Node block import interface address
    #[arg(long, default_value = "127.0.0.1:42500")]
    pub bp_node: RemoteAddr,
}

fn main() -> Result<(), io::Error> {
    let args = Args::parse();
    LogLevel::from_verbosity_flag_count(args.verbose).apply();
    log::debug!("Command-line arguments: {:#?}", &args);

    log::info!("Connecting to BP Node at {}", args.bp_node);
    let client =
        Client::new(BlockExporter::new(), args.bp_node.clone()).expect("Unable to create client");

    let agent: AgentInfo = AgentInfo {
        agent: ident!(AGENT),
        version: Version::new(0, 1, 0),
        network: Ident::try_from(args.network.to_string()).unwrap(),
        features: 0,
    };
    if let Err(err) = client.send(ExporterPub::Hello(agent)) {
        log::error!("Unable to send hello message to BP Node due to {err}");
        exit(1);
    }

    thread::spawn(move || {
        read_blocks(client, args);
    })
    .join()
    .expect("Unable to join fs thread");

    Ok(())
}

fn read_blocks(client: Client<ExporterPub>, args: Args) {
    log::info!("Reading block files in '{}'", args.data_dir.display());
    if !fs::exists(&args.data_dir).expect("Unable to access data directory") {
        log::error!("Data directory '{}' does not exist", args.data_dir.display());
        exit(1);
    }

    // Select the correct block separator according to the network type
    let block_separator = match args.network {
        Network::Mainnet => BITCOIN_BLOCK_SEPARATOR,
        Network::Testnet3 => TESTNET_BLOCK_SEPARATOR,
        Network::Testnet4 => TESTNET4_BLOCK_SEPARATOR,
        Network::Signet => SIGNET_BLOCK_SEPARATOR,
        Network::Regtest => REGTEST_BLOCK_SEPARATOR,
    };

    let mut file_no: u32 = 0;
    let mut total_blocks: u32 = 0;
    let mut total_tx: u64 = 0;
    let mut buf = [0u8; 4];
    while let Ok(mut file) = fs::File::open(args.data_dir.join(format!("blk{file_no:05}.dat")))
        .or_else(|err| match err.kind() {
            io::ErrorKind::NotFound => Err(()),
            io::ErrorKind::PermissionDenied => {
                log::error!(
                    "Unable to open file 'blk{file_no:05}.dat' with the current user permissions"
                );
                exit(2);
            }
            _ => {
                log::error!("Unable to open file 'blk{file_no:05}.dat' due to {err}");
                exit(3);
            }
        })
    {
        log::info!("Processing block file 'blk{file_no:05}.dat'");

        let mut block_no = 1u64;
        let mut block_txes = 0u64;
        let mut thousands = 0u64;
        loop {
            // Checking magic number
            match file.read_exact(&mut buf) {
                Ok(_) => {}
                Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(err) => {
                    log::error!("Unable to read block #{block_no} due to {err}");
                    exit(4);
                }
            }

            if buf == [0x00, 0x00, 0x00, 0x00] {
                log::info!("Reached end of block file");
                break;
            }

            if buf != block_separator {
                log::error!(
                    "Invalid block separator 0x{:02X}{:02X}{:02X}{:02X} before block #{block_no}",
                    buf[0],
                    buf[1],
                    buf[2],
                    buf[3]
                );
                exit(5);
            }

            // Reading block, checking its length
            let pos = file.stream_position().expect("Unable to get file position");
            file.read_exact(&mut buf).expect("Broken block file");
            let len = u32::from_le_bytes(buf) as u64;
            let block = Block::consensus_decode(&mut file).unwrap_or_else(|err| {
                log::error!("Unable to decode block #{block_no} due to {err}");
                exit(6);
            });
            let new_pos = file.stream_position().expect("Unable to get file position");
            if new_pos != pos + len + 4 {
                log::error!(
                    "Invalid block length for block #{block_no}; expected {len}, got {}",
                    new_pos - pos - 4
                );
                exit(7);
            }

            let txes = block.transactions.len() as u64;
            log::debug!(
                "Processing block #{block_no} {} ({len} bytes, {txes} transactions)",
                block.block_hash(),
            );

            if let Err(err) = client.send(ExporterPub::Block(block)) {
                log::error!("Unable to send block #{block_no} due to {err}");
                exit(8);
            }

            block_no += 1;
            total_blocks += 1;
            block_txes += txes;
            total_tx += txes;

            if total_tx / 1000 > thousands {
                thousands = total_tx / 1000;
                log::info!("Processed {total_blocks} blocks, {total_tx} transactions");
            }
        }

        log::info!(
            "Block file 'blk{file_no:05}.dat' with {block_no} blocks and {block_txes} \
             transactions has being processed"
        );
        file_no += 1;
    }

    log::info!(
        "{file_no} block files with {total_blocks} blocks and {total_tx} transactions has being \
         processed"
    );

    client.terminate().expect("Unable to terminate connection");
}
