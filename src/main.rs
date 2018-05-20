extern crate byteorder;
extern crate colored;
extern crate docopt;
extern crate failure;
extern crate flate2;
#[macro_use]
extern crate flatdata;
extern crate prost;
#[macro_use]
extern crate prost_derive;
#[macro_use]
extern crate serde_derive;

mod args;
mod osmflat;
mod osmpbf;

use args::parse_args;

use byteorder::{ByteOrder, NetworkEndian};
use colored::*;
use failure::Error;
use flatdata::{ArchiveBuilder, FileResourceStorage};
use flate2::read::ZlibDecoder;
use prost::Message;

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, BufReader, ErrorKind, Read};
use std::path::Path;
use std::rc::Rc;

#[derive(Debug, Clone)]
enum OsmPbfBlock {
    OsmHeader(osmpbf::HeaderBlock),
    OsmData(osmpbf::PrimitiveBlock),
}

struct OsmPbfIterator {
    reader: BufReader<File>,
    file_buf: Vec<u8>,
    blob_buf: Vec<u8>,
    is_open: bool,
}

impl OsmPbfIterator {
    fn new<P: AsRef<Path>>(path: P) -> Result<OsmPbfIterator, Error> {
        let file = File::open(path)?;
        Ok(OsmPbfIterator {
            reader: BufReader::new(file),
            file_buf: Vec::new(),
            blob_buf: Vec::new(),
            is_open: true,
        })
    }

    fn read_next(&mut self) -> Result<OsmPbfBlock, io::Error> {
        // read size of blob header
        self.file_buf.resize(4, 0);
        self.reader.read_exact(&mut self.file_buf)?;
        let blob_header_len: i32 = NetworkEndian::read_i32(&self.file_buf);

        // read blob header
        self.file_buf.resize(blob_header_len as usize, 0);
        self.reader.read_exact(&mut self.file_buf)?;
        let blob_header = osmpbf::BlobHeader::decode(&self.file_buf)?;

        // read blob
        self.file_buf.resize(blob_header.datasize as usize, 0);
        self.reader.read_exact(&mut self.file_buf)?;
        let blob = osmpbf::Blob::decode(&self.file_buf)?;

        let blob_data = if blob.raw.is_some() {
            // use raw bytes
            blob.raw.as_ref().unwrap()
        } else if blob.zlib_data.is_some() {
            // decompress zlib data
            self.blob_buf.clear();
            let data: &Vec<u8> = blob.zlib_data.as_ref().unwrap();
            let mut decoder = ZlibDecoder::new(&data[..]);
            decoder.read_to_end(&mut self.blob_buf)?;
            &self.blob_buf
        } else {
            panic!("can only read raw or zlib compressed blob");
        };
        assert_eq!(
            blob_data.len(),
            blob.raw_size.unwrap_or_else(|| blob_data.len() as i32) as usize
        );

        Ok(if blob_header.type_ == "OSMHeader" {
            OsmPbfBlock::OsmHeader(osmpbf::HeaderBlock::decode(blob_data)?)
        } else if blob_header.type_ == "OSMData" {
            OsmPbfBlock::OsmData(osmpbf::PrimitiveBlock::decode(blob_data)?)
        } else {
            panic!("unknown blob type");
        })
    }
}

impl Iterator for OsmPbfIterator {
    type Item = Result<OsmPbfBlock, io::Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.is_open {
            let next = self.read_next();
            if let Err(e) = next {
                if e.kind() == ErrorKind::UnexpectedEof {
                    self.is_open = false;
                    None
                } else {
                    Some(Err(e))
                }
            } else {
                Some(next)
            }
        } else {
            None
        }
    }
}

fn run() -> Result<(), Error> {
    let args = parse_args();

    let pbf_iter = OsmPbfIterator::new(args.arg_input)?;
    for block in pbf_iter {
        let block = block?;
        match block {
            OsmPbfBlock::OsmHeader(header) => println!("{:?}", header),
            OsmPbfBlock::OsmData(data) => println!("{}", data.stringtable.s.len()),
        }
    }

    let storage = Rc::new(RefCell::new(FileResourceStorage::new(
        args.arg_output.into(),
    )));
    let _builder = osmflat::OsmBuilder::new(storage);

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
}
