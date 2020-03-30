// Bitcoin Core blocks directory processor
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

use std::{io, fs};
use bitcoin::{
    Block,
    consensus::encode::serialize,
    network::stream_reader::StreamReader
};

fn main() -> io::Result<()> {
    let mut block_file_no: u16 = 0;

    let context = zmq::Context::new();
    let client = context.socket(zmq::REQ).unwrap();
    client.connect("tcp://127.0.0.1:18318").expect("Can't connect to parser daemon");
    println!("Connected to txparserd daemon");

    loop {
        let blockchain_path = "/var/lib/bitcoin/blocks";
        let block_file_name = format!("{}/blk{:05}.dat", blockchain_path, block_file_no);

        println!("Parsing blocks from {} ", block_file_name);

        let block_file = fs::File::open(block_file_name)?;
        let mut stream_reader = StreamReader::new(io::BufReader::new(block_file), None);
        block_file_no += 1;

        let mut block_no = 0;
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
            let block_len = stream_reader.read_next::<u32>();

            // Reading block
            match stream_reader.read_next::<Block>() {
                Err(err) => {
                    eprintln!("{}", err);
                    break;
                },
                Ok(mut block) => {
                    client.send_multipart(vec![b"BLOCK".to_vec(), serialize(&block)], 0)
                        .expect("Can't send data to parser daemon");
                    let print = match client.recv_string(0)
                        .expect("Parser response must be string")
                        .expect("Can't receive parser daemon response")
                        .as_str() {
                        "ACK" => ".",
                        "ERR" => "!",
                        _ => "_"
                    };
                    if block_no % 80 == 0 {
                        println!("{}", print);
                    } else {
                        print!("{}", print);
                    }
                }
            }

            block_no += 1;
        }

        println!(" {} blocks parsed", block_no);
    }
}