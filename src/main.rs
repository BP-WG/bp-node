#[macro_use]
extern crate clap;
#[macro_use]
extern crate diesel;
extern crate chrono;
extern crate dotenv;
extern crate lnpbp;

use std::fs::File;
use std::io;
use std::env;
use std::convert::{TryFrom, TryInto};
use std::collections::{HashMap, hash_map::Entry};
use chrono::NaiveDateTime;
use clap::{App, ArgMatches};
use diesel::prelude::*;
use diesel::pg::PgConnection;
use dotenv::dotenv;
use lnpbp::bitcoin::network::stream_reader::StreamReader;
use lnpbp::bitcoin::{Block, TxIn, TxOut, Txid};
use lnpbp::bp::short_id::{ShortId, self};
use lnpbp::bp::{BlockChecksum, Descriptor};

mod schema;
mod models;

fn main() -> std::io::Result<()> {
    eprintln!("\nBitcoin protocol dumping tool\n");

    let yaml = load_yaml!("../cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    match matches.value_of("COMMAND").unwrap() {
        "block" => parse_block(matches),
        _ => unreachable!(),
    }
}

pub fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

fn parse_block(matches: ArgMatches) -> std::io::Result<()> {
    let conn= establish_connection();

    let filename = matches.value_of("INPUT");
    eprintln!("Parsing block file {}", filename.unwrap_or("STDIN"));

    let mut stream_reader = match filename {
        Some(name) => {
            let buf_read: Box<dyn io::Read> = Box::new(io::BufReader::new(File::open(name)?));
            StreamReader::new(buf_read, None)
        },
        None => {
            let stdin: Box<dyn io::Read> = Box::new(io::stdin());
            StreamReader::new(stdin, None)
        },
    };

    eprintln!("Clearning database tables");
    diesel::delete(schema::txin::table).execute(&conn);
    diesel::delete(schema::txout::table).execute(&conn);
    diesel::delete(schema::tx::table).execute(&conn);
    diesel::delete(schema::block::table).execute(&conn);

    let mut block_height = 0;
    loop {
        // Reading magick number
        match stream_reader.read_next::<u32>() {
            Ok(0xD9B4BEF9) => (),
            _ => {
                eprintln!("No magick number found");
                break;
            }
        }
        eprintln!("Magick number ok");

        // Skipping block length
        let _ = stream_reader.read_next::<u32>();

        let mut utxo: HashMap<Txid, HashMap<u16, Descriptor>> = HashMap::with_capacity(100_000);

        // Reading block
        match stream_reader.read_next::<Block>() {
            Err(err) => {
                eprintln!("{}", err);
                break;
            },
            Ok(block) => {
                let block_checksum = BlockChecksum::from(block.block_hash());

                let block_usbid: u64 = ShortId::try_from(short_id::Descriptor::OnchainBlock { height: block_height })
                    .expect("Descriptor is constructed from real blockchain data so it must not fail")
                    .into();
                eprintln!("Block {:x}, hash {}:", block_usbid, block.block_hash());
                diesel::insert_into(schema::block::table).values(&models::Block {
                    id: block_usbid as i64,
                    block_id: block.block_hash().to_vec(),
                    merkle_root: block.merkle_root().to_vec(),
                    ts: NaiveDateTime::from_timestamp(block.header.time as i64, 0),
                    difficulty: block.header.bits as i64,
                    nonce: block.header.nonce as i32,
                    ver: block.header.version as i32,
                    tx_count: block.txdata.len() as i32
                })
                    .get_result::<models::Block>(&conn)
                    .expect("Error saving block");

                for (tx_index, tx) in block.txdata.iter().enumerate() {
                    let tx_usbid: u64 = ShortId::try_from(short_id::Descriptor::OnchainTransaction {
                        block_height,
                        block_checksum,
                        tx_index: tx_index as u16
                    }).expect("Descriptor is constructed from real blockchain data so it must not fail")
                      .into();
                    eprintln!("\tTransaction {}, {} inputs, {} outputs", tx.txid(), tx.input.len(), tx.output.len());
                    diesel::insert_into(schema::tx::table).values(&models::Tx {
                        id: tx_usbid as i64,
                        ver: tx.version as i32,
                        locktime: tx.lock_time as i32,
                        out_count: tx.output.len() as i16,
                        in_count: tx.input.len() as i16,
                        fee: None
                    })
                        .get_result::<models::Tx>(&conn)
                        .expect("Error saving transaction");

                    diesel::insert_into(schema::txin::table)
                        .values(tx.input.iter().enumerate().map(|(input_index, txin)| {
                            let prev_vout: u16 = txin.previous_output.vout as u16;

                            let txo_descriptor = if tx.is_coin_base() {
                                let descriptor = short_id::Descriptor::OnchainBlock { height: block_height };
                                let cb_usbid: u64 = ShortId::try_from(descriptor)
                                    .expect("Descriptor is constructed from real blockchain data so it must not fail")
                                    .into();
                                diesel::insert_into(schema::txout::table)
                                    .values(models::Txout {
                                        id: cb_usbid as i64,
                                        amount: tx.output[0].value as i64,
                                        script: vec![]
                                    })
                                    .get_results::<models::Txout>(&conn)
                                    .expect("Error inserting coinbase input");
                                descriptor
                            } else {
                                let mut txoset = utxo.get_mut(&txin.previous_output.txid)
                                    .expect("Validated transaction always spends existing transaction");
                                let descriptor = txoset.remove(&prev_vout)
                                    .expect("Validated transaction always spends existing transaction output");
                                if txoset.is_empty() {
                                    utxo.remove(&txin.previous_output.txid);
                                }
                                descriptor
                            };

                            let txin_usbid: u64 = ShortId::try_from(short_id::Descriptor::OnchainTxInput {
                                block_height,
                                block_checksum,
                                tx_index: tx_index as u16,
                                input_index: input_index as u16
                            }).expect("Descriptor is constructed from real blockchain data so it must not fail")
                              .into();
                            let txout_fk_usbid: u64 = ShortId::try_from(txo_descriptor)
                                .expect("Descriptor is constructed from real blockchain data so it must not fail")
                                .into();
                            models::Txin {
                                id: txin_usbid as i64,
                                seq: txin.sequence as i32,
                                txout_id: txout_fk_usbid as i64
                            }
                        }).collect::<Vec<models::Txin>>())
                        .get_results::<models::Txin>(&conn)
                        .expect("Error saving transaction inputs");

                    diesel::insert_into(schema::txout::table)
                        .values(tx.output.iter().enumerate().map(|(output_index, txout)| {
                            let txout_descriptor = short_id::Descriptor::OnchainTxOutput {
                                block_height,
                                block_checksum,
                                tx_index: tx_index as u16,
                                output_index: output_index as u16
                            };
                            let txout_usbid: u64 = ShortId::try_from(txout_descriptor)
                                .expect("Descriptor is constructed from real blockchain data so it must not fail")
                                .into();

                            let txid = tx.txid();
                            let mut txoset = match utxo.entry(txid) {
                                Entry::Vacant(entry) => entry.insert(HashMap::new()),
                                Entry::Occupied(entry) => entry.into_mut(),
                            };
                            txoset.insert(output_index as u16, txout_descriptor);

                            models::Txout {
                                id: txout_usbid as i64,
                                amount: txout.value as i64,
                                script: txout.script_pubkey.to_bytes()
                            }
                        }).collect::<Vec<models::Txout>>())
                        .get_results::<models::Txout>(&conn)
                        .expect("Error saving transaction outputs");
                }

                //eprintln!("* read block no {}, id {}", block_height, block.block_hash());
                //println!("{:#?}", block.header);
                //println!("{:#?}", block.txdata[0]);
                block_height += 1;
            },
        }
    }
    Ok(())
}
