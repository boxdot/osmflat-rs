extern crate byteorder;
extern crate docopt;
extern crate prost;
#[macro_use]
extern crate prost_derive;
#[macro_use]
extern crate serde_derive;
extern crate flate2;

mod args;
mod osmpbf;

use args::parse_args;

use byteorder::{ByteOrder, NetworkEndian};
use flate2::read::ZlibDecoder;
use prost::Message;

use std::fs::File;
use std::io::{BufReader, ErrorKind, Read};

fn primitrive_group_type(group: &osmpbf::PrimitiveGroup) -> String {
    if !group.nodes.is_empty() {
        format!("Nodes {}", group.nodes.len())
    } else if group.dense.is_some() {
        String::from("DenseNodes")
    } else if !group.ways.is_empty() {
        format!("Ways {}", group.ways.len())
    } else if !group.relations.is_empty() {
        format!("Relations {}", group.relations.len())
    } else if !group.changesets.is_empty() {
        format!("ChangeSet {}", group.changesets.len())
    } else {
        "unknown type".into()
    }
}

fn main() {
    let args = parse_args();

    let file = File::open(args.arg_input).unwrap();
    let mut reader = BufReader::new(file);

    let mut buf = Vec::new();
    let mut blob_buf = Vec::new();

    loop {
        // read size of blob header
        buf.resize(4, 0);
        if let Err(e) = reader.read_exact(&mut buf) {
            if e.kind() == ErrorKind::UnexpectedEof {
                println!("End of file");
                return;
            }
        }
        let blob_header_len: i32 = NetworkEndian::read_i32(&buf);
        println!("blob_header_len = {}", blob_header_len);

        // read blob header
        buf.resize(blob_header_len as usize, 0);
        reader.read_exact(&mut buf).unwrap();
        let blob_header = osmpbf::BlobHeader::decode(&buf).unwrap();
        println!("{:?}", blob_header);

        // read blob
        buf.resize(blob_header.datasize as usize, 0);
        reader.read_exact(&mut buf).unwrap();
        let blob = osmpbf::Blob::decode(&buf).unwrap();
        println!("Blob {{ raw_size = {:?}, ... }}", blob.raw_size,);

        let blob_data = if blob.raw.is_some() {
            // use raw bytes
            blob.raw.as_ref().unwrap()
        } else if blob.zlib_data.is_some() {
            // decompress bz data
            blob_buf.clear();
            let data: &Vec<u8> = blob.zlib_data.as_ref().unwrap();
            let mut decoder = ZlibDecoder::new(&data[..]);
            decoder.read_to_end(&mut blob_buf).unwrap();
            &blob_buf
        } else {
            assert!(false, "can only read raw or zlib compressed blob");
            return;
        };
        assert_eq!(
            blob_data.len(),
            blob.raw_size.unwrap_or_else(|| blob_data.len() as i32) as usize
        );

        println!("Decompressed size: {}", blob_data.len());

        if blob_header.type_ == "OSMHeader" {
            let header = osmpbf::HeaderBlock::decode(blob_data).unwrap();
            println!("{:?}", header);
        } else if blob_header.type_ == "OSMData" {
            let block = osmpbf::PrimitiveBlock::decode(blob_data).unwrap();
            for group in block.primitivegroup.iter() {
                println!("Group: {:?}", primitrive_group_type(group));
            }
        } else {
            assert!(false, "unknown blob type");
            return;
        }
    }
}
