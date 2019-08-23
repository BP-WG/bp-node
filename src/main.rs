#[macro_use]
extern crate clap;
extern crate bitcoin;

use std::fs::File;
use std::io;
use clap::{App, ArgMatches};
use bitcoin::network::stream_reader::StreamReader;
use bitcoin::{Block, BitcoinHash};


#[cfg(feature = "yaml")]
fn main() -> std::io::Result<()> {
    eprintln!("\nBitcoin protocol dumping tool\n");

    let yaml = load_yaml!("../cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    match matches.value_of("COMMAND").unwrap() {
        "block" => parse_block(matches),
        _ => unreachable!(),
    }
}

#[cfg(not(feature = "yaml"))]
fn main() {
    // As stated above, if clap is not compiled with the YAML feature, it is disabled.
    eprintln!("YAML feature is disabled.");
    eprintln!("Pass --features yaml to cargo when trying this example.");
}


fn parse_block(matches: ArgMatches) -> std::io::Result<()> {
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
        let _ = stream_reader.read_next::<u32>()?;
        // Reading block
        match stream_reader.read_next::<Block>() {
            Err(err) => {
                eprintln!("{}", err);
                break;
            },
            Ok(block) => {
                eprintln!("* read block no {}, id {}", blkno, block.bitcoin_hash());
                println!("{:#?}", block.header);
                println!("{:#?}", block.txdata[0]);
                blkno += 1;
            },
        }
    }
    Ok(())
}
