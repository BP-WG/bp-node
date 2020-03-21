#[macro_use]
extern crate clap;
#[macro_use]
extern crate diesel;
extern crate chrono;
extern crate dotenv;
extern crate bitcoin;

use std::fs::File;
use std::io;
use std::env;
use chrono::NaiveDateTime;
use clap::{App, ArgMatches};
use diesel::prelude::*;
use diesel::pg::PgConnection;
use dotenv::dotenv;
use bitcoin::network::stream_reader::StreamReader;
use bitcoin::{Block, BitcoinHash};

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

    let mut blkno = 0;
    loop {
        // Reading magick number
        match stream_reader.read_next::<u32>() {
            Ok(0xD9B4BEF9) => (),
            _ => {
                eprintln!("No magick number found");
                break;
            }
        }
        // Skipping block length
        eprintln!("Magick number ok");
        let _ = stream_reader.read_next::<u32>();

        // Reading block
        match stream_reader.read_next::<Block>() {
            Err(err) => {
                eprintln!("{}", err);
                break;
            },
            Ok(block) => {

                diesel::insert_into(schema::block::table).values(&models::Block {
                    id: blkno,
                    block_id: block.bitcoin_hash().to_vec(),
                    merkle_root: block.merkle_root().to_vec(),
                    ts: NaiveDateTime::from_timestamp(block.header.time as i64, 0),
                    difficulty: block.header.bits as i64,
                    nonce: block.header.nonce as i32,
                    ver: block.header.version as i32,
                    tx_count: block.txdata.len() as i32
                })
                .get_result::<models::Block>(&conn)
                .expect("Error saving block");

                eprintln!("* read block no {}, id {}", blkno, block.bitcoin_hash());
                println!("{:#?}", block.header);
                println!("{:#?}", block.txdata[0]);
                blkno += 1;
            },
        }
    }
    Ok(())
}
