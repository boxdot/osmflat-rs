use byteorder::{ByteOrder, NetworkEndian};
use flate2::read::ZlibDecoder;
use log::info;
use prost::{self, Message};
use rayon::prelude::*;

use std::io::{self, Read};

include!(concat!(env!("OUT_DIR"), "/osmpbf.rs"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BlockType {
    Header,
    Nodes,
    DenseNodes,
    Ways,
    Relations,
}

impl BlockType {
    /// Decode block type from PrimitiveBlock protobuf message
    ///
    /// This does not decode any fields, it just checks which tags are present
    /// in PrimitiveGroup fields of the message.
    ///
    /// `blob` should contain decompressed data of an OSMData PrimitiveBlock.
    ///
    /// Note: We use public API of `prost` crate, which though is not exposed in
    /// the crate and marked with comment that it should be only used from
    /// `prost::Message`.
    pub fn from_osmdata_blob(mut blob: &[u8]) -> io::Result<BlockType> {
        const PRIMITIVE_GROUP_TAG: u32 = 2;
        const NODES_TAG: u32 = 1;
        const DENSE_NODES_TAG: u32 = 2;
        const WAY_STAG: u32 = 3;
        const RELATIONS_TAG: u32 = 4;
        const CHANGESETS_TAG: u32 = 5;

        loop {
            // decode fields of PrimitiveBlock
            let (key, wire_type) = prost::encoding::decode_key(&mut blob)?;
            if key != PRIMITIVE_GROUP_TAG {
                // primitive group
                prost::encoding::skip_field(
                    wire_type,
                    key,
                    &mut blob,
                    prost::encoding::DecodeContext::default(),
                )?;
                continue;
            }

            // We found a PrimitiveGroup field. There could be several of them, but
            // follwoing the specs of OSMPBF, all of them will have the same single
            // optional field, which defines the type of the block.

            // Decode the number of primitive groups.
            let _ = prost::encoding::decode_varint(&mut blob)?;
            // Decode the tag of the first primitive group defining the type.
            let (tag, _wire_type) = prost::encoding::decode_key(&mut blob)?;
            let block_type = match tag {
                NODES_TAG => BlockType::Nodes,
                DENSE_NODES_TAG => BlockType::DenseNodes,
                WAY_STAG => BlockType::Ways,
                RELATIONS_TAG => BlockType::Relations,
                CHANGESETS_TAG => {
                    panic!("found block containing unsupported changesets");
                }
                _ => {
                    panic!("invalid input data: malformed primitive block");
                }
            };
            return Ok(block_type);
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct BlockIndex {
    pub block_type: BlockType,
    pub blob_start: usize,
    pub blob_len: usize,
}

struct BlockIndexIterator<'a> {
    data: &'a [u8],
    cursor: usize,
}

enum BlobInfo {
    Header(BlockIndex),
    Unknown(usize, usize, Vec<u8>),
}

impl<'a> BlockIndexIterator<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, cursor: 0 }
    }

    fn read(&mut self, len: usize) -> &[u8] {
        let data = &self.data[self.cursor..self.cursor + len];
        self.cursor += len;
        data
    }

    fn next_blob(&mut self) -> Result<BlobInfo, io::Error> {
        // read size of blob header
        let blob_header_len: i32 = NetworkEndian::read_i32(self.read(4));

        // read blob header
        let blob_header = BlobHeader::decode(self.read(blob_header_len as usize))?;

        let blob_start = self.cursor;
        let blob_len = blob_header.datasize as usize;

        if blob_header.r#type == "OSMHeader" {
            self.cursor += blob_len;
            Ok(BlobInfo::Header(BlockIndex {
                block_type: BlockType::Header,
                blob_start,
                blob_len,
            }))
        } else if blob_header.r#type == "OSMData" {
            // read blob
            Ok(BlobInfo::Unknown(
                blob_start,
                blob_len,
                self.read(blob_header.datasize as usize).to_vec(),
            ))
        } else {
            panic!("unknown blob type");
        }
    }
}

impl<'a> Iterator for BlockIndexIterator<'a> {
    type Item = Result<BlobInfo, io::Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor < self.data.len() {
            Some(self.next_blob())
        } else {
            None
        }
    }
}

pub fn read_block<T: prost::Message + Default>(
    data: &[u8],
    idx: &BlockIndex,
) -> Result<T, io::Error> {
    let blob = Blob::decode(&data[idx.blob_start..idx.blob_start + idx.blob_len])?;

    let mut blob_buf = Vec::new();
    let blob_data = if blob.raw.is_some() {
        blob.raw.as_ref().unwrap()
    } else if blob.zlib_data.is_some() {
        // decompress zlib data
        let data: &Vec<u8> = blob.zlib_data.as_ref().unwrap();
        let mut decoder = ZlibDecoder::new(&data[..]);
        decoder.read_to_end(&mut blob_buf)?;
        &blob_buf
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unknown compression",
        ));
    };
    Ok(T::decode(blob_data.as_slice())?)
}

fn blob_type_from_blob_info(
    blob_start: usize,
    blob_len: usize,
    blob: Vec<u8>,
) -> Result<BlockIndex, io::Error> {
    let blob = Blob::decode(blob.as_slice())?;

    let mut blob_buf = Vec::new();
    let blob_data = if blob.raw.is_some() {
        // use raw bytes
        blob.raw.as_ref().unwrap()
    } else if blob.zlib_data.is_some() {
        // decompress zlib data
        let data: &Vec<u8> = blob.zlib_data.as_ref().unwrap();
        let mut decoder = ZlibDecoder::new(&data[..]);
        decoder.read_to_end(&mut blob_buf)?;
        &blob_buf
    } else {
        panic!("can only read raw or zlib compressed blob");
    };
    assert_eq!(
        blob_data.len(),
        blob.raw_size.unwrap_or(blob_data.len() as i32) as usize
    );

    Ok(BlockIndex {
        block_type: BlockType::from_osmdata_blob(&blob_data[..])?,
        blob_start,
        blob_len,
    })
}

pub fn build_block_index(pbf_data: &[u8]) -> Vec<BlockIndex> {
    let mut result: Vec<BlockIndex> = BlockIndexIterator::new(pbf_data)
        .par_bridge()
        .filter_map(|blob| {
            let block = match blob {
                Ok(BlobInfo::Header(b)) => Ok(b),
                Ok(BlobInfo::Unknown(start, len, blob)) => {
                    blob_type_from_blob_info(start, len, blob)
                }
                Err(e) => Err(e),
            };
            match block {
                Ok(b) => Some(b),
                Err(e) => {
                    eprintln!("Skipping block due to error: {}", e);
                    None
                }
            }
        })
        .collect();
    result.par_sort_unstable();
    info!("Found {} blocks", result.len());
    result
}
