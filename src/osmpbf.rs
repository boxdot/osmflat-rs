use byteorder::{ByteOrder, NetworkEndian};
use failure::Error;
use flate2::read::ZlibDecoder;
use prost::{self, Message};

use std::fs::File;
use std::io::{self, BufReader, Cursor, ErrorKind, Read, Seek, SeekFrom};
use std::path::Path;

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
    pub fn from_osmdata_blob(blob: &[u8]) -> Result<BlockType, io::Error> {
        const PRIMITIVE_GROUP_TAG: u32 = 2;
        const NODES_TAG: u32 = 1;
        const DENSE_NODES_TAG: u32 = 2;
        const WAY_STAG: u32 = 3;
        const RELATIONS_TAG: u32 = 4;
        const CHANGESETS_TAG: u32 = 5;

        let mut cursor = Cursor::new(&blob[..]);
        loop {
            // decode fields of PrimitiveBlock
            let (key, wire_type) = prost::encoding::decode_key(&mut cursor)?;
            if key != PRIMITIVE_GROUP_TAG {
                // primitive group
                prost::encoding::skip_field(wire_type, &mut cursor)?;
                continue;
            }

            // We found a PrimitiveGroup field. There could be several of them, but
            // follwoing the specs of OSMPBF, all of them will have the same single
            // optional field, which defines the type of the block.

            // Decode the number of primitive groups.
            let _ = prost::encoding::decode_varint(&mut cursor)?;
            // Decode the tag of the first primitive group defining the type.
            let (tag, _wire_type) = prost::encoding::decode_key(&mut cursor)?;
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockIndex {
    pub block_type: BlockType,
    pub blob_start: usize,
    pub blob_len: usize,
}

struct BlockIndexIterator {
    reader: BufReader<File>,
    cursor: usize,
    file_buf: Vec<u8>,
    blob_buf: Vec<u8>,
    is_open: bool,
}

impl BlockIndexIterator {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let file = File::open(path)?;
        Ok(Self {
            reader: BufReader::new(file),
            cursor: 0,
            file_buf: Vec::new(),
            blob_buf: Vec::new(),
            is_open: true,
        })
    }

    fn read_next(&mut self) -> Result<BlockIndex, io::Error> {
        // read size of blob header
        self.cursor += 4;
        self.file_buf.resize(4, 0);
        self.reader.read_exact(&mut self.file_buf)?;
        let blob_header_len: i32 = NetworkEndian::read_i32(&self.file_buf);

        // read blob header
        self.cursor += blob_header_len as usize;
        self.file_buf.resize(blob_header_len as usize, 0);
        self.reader.read_exact(&mut self.file_buf)?;
        let blob_header = BlobHeader::decode(&self.file_buf)?;

        let blob_start = self.cursor;
        let blob_len = blob_header.datasize as usize;
        self.cursor += blob_len;

        if blob_header.type_ == "OSMHeader" {
            self.reader.seek(SeekFrom::Current(blob_len as i64))?;
            Ok(BlockIndex {
                block_type: BlockType::Header,
                blob_start,
                blob_len,
            })
        } else if blob_header.type_ == "OSMData" {
            // read blob
            self.file_buf.resize(blob_header.datasize as usize, 0);
            self.reader.read_exact(&mut self.file_buf)?;
            let blob = Blob::decode(&self.file_buf)?;

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

            Ok(BlockIndex {
                block_type: BlockType::from_osmdata_blob(&blob_data[..])?,
                blob_start,
                blob_len,
            })
        } else {
            panic!("unknown blob type");
        }
    }
}

impl Iterator for BlockIndexIterator {
    type Item = Result<BlockIndex, io::Error>;
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

pub fn read_block<F: Read + Seek, T: prost::Message + Default>(
    reader: &mut F,
    idx: &BlockIndex,
) -> Result<T, Error> {
    reader.seek(io::SeekFrom::Start(idx.blob_start as u64))?;

    // TODO: allocate buffers outside of the function
    let mut buf = Vec::new();
    buf.resize(idx.blob_len, 0);
    reader.read_exact(&mut buf)?;
    let blob = Blob::decode(&buf)?;

    let mut blob_buf = Vec::new();
    let blob_data = if blob.raw.is_some() {
        blob.raw.as_ref().unwrap()
    } else if blob.zlib_data.is_some() {
        // decompress zlib data
        blob_buf.clear();
        let data: &Vec<u8> = blob.zlib_data.as_ref().unwrap();
        let mut decoder = ZlibDecoder::new(&data[..]);
        decoder.read_to_end(&mut blob_buf)?;
        &blob_buf
    } else {
        return Err(format_err!("invalid input data: unknown compression"));
    };
    Ok(T::decode(blob_data)?)
}

/// Reads the pbf file at the given path and builds an index of block types.
///
/// The index is sorted lexicographically by block type and position in the pbf
/// file.
pub fn build_block_index<P: AsRef<Path>>(path: P) -> Result<Vec<BlockIndex>, Error> {
    let mut index: Vec<_> = BlockIndexIterator::new(path)?
        .filter_map(|block| match block {
            Ok(b) => Some(b),
            Err(e) => {
                eprintln!("Skipping block due to error: {}", e);
                None
            }
        })
        .collect();
    index.sort();
    Ok(index)
}
