extern crate docopt;
extern crate prost;
#[macro_use]
extern crate prost_derive;
#[macro_use]
extern crate serde_derive;
extern crate byteorder;

mod args;
mod osmpbf;

use args::parse_args;

use byteorder::{BigEndian, ByteOrder, NetworkEndian};
use prost::Message;

use std::fs::File;
use std::io::prelude::*;

fn main() {
    let args = parse_args();
    println!("Hello, world! {:?}", args);

    let mut file = File::open(args.arg_input).unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();

    let blob_header_len: i32 = NetworkEndian::read_i32(&buf[0..4]);
    let x = osmpbf::BlobHeader::decode(&buf[4..blob_header_len]).unwrap();
    println!("{:?}", x);
}
