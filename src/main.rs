extern crate byteorder;
extern crate colored;
extern crate docopt;
#[macro_use]
extern crate failure;
extern crate flate2;
#[macro_use]
extern crate flatdata;
extern crate itertools;
extern crate prost;
#[macro_use]
extern crate prost_derive;
#[macro_use]
extern crate serde_derive;

mod args;
mod osmflat;
mod osmpbf;

use args::parse_args;
use std::io::Seek;

use byteorder::{ByteOrder, NetworkEndian};
use colored::*;
use failure::Error;
use flatdata::{ArchiveBuilder, FileResourceStorage};
use flate2::read::ZlibDecoder;
use itertools::Itertools;
use prost::Message;

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, BufReader, ErrorKind, Read};
use std::path::Path;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OsmGroupType {
    Nodes,
    DenseNodes,
    Ways,
    Relations,
}

fn get_group_type(group: &osmpbf::PrimitiveGroup) -> OsmGroupType {
    if !group.nodes.is_empty() {
        OsmGroupType::Nodes
    } else if group.dense.is_some() {
        OsmGroupType::DenseNodes
    } else if !group.ways.is_empty() {
        OsmGroupType::Ways
    } else if !group.relations.is_empty() {
        OsmGroupType::Relations
    } else {
        panic!("not supported group type")
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum BlockCompression {
    Uncompressed,
    Zlib,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BlockIndex {
    block_type: BlockType,
    blob_start: usize,
    blob_len: usize,
    compression: BlockCompression,
}

struct OsmBlockIndexIterator {
    reader: BufReader<File>,
    cursor: usize,
    file_buf: Vec<u8>,
    blob_buf: Vec<u8>,
    is_open: bool,
}

impl OsmBlockIndexIterator {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
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
        let blob_header = osmpbf::BlobHeader::decode(&self.file_buf)?;

        let blob_start = self.cursor;
        let blob_len = blob_header.datasize as usize;

        // read blob
        self.cursor += blob_header.datasize as usize;
        self.file_buf.resize(blob_header.datasize as usize, 0);
        self.reader.read_exact(&mut self.file_buf)?;
        let blob = osmpbf::Blob::decode(&self.file_buf)?;

        let compression;
        let blob_data = if blob.raw.is_some() {
            compression = BlockCompression::Uncompressed;
            // use raw bytes
            blob.raw.as_ref().unwrap()
        } else if blob.zlib_data.is_some() {
            compression = BlockCompression::Zlib;
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

        let block_type = if blob_header.type_ == "OSMHeader" {
            BlockType::Header
        } else if blob_header.type_ == "OSMData" {
            // TODO: Avoid decoding the full block. We need just the type of the primitive
            // group.
            let data = osmpbf::PrimitiveBlock::decode(blob_data)?;

            assert_eq!(data.primitivegroup.len(), 1);
            let group_type = get_group_type(&data.primitivegroup[0]);
            match group_type {
                OsmGroupType::Nodes => BlockType::Nodes,
                OsmGroupType::DenseNodes => BlockType::DenseNodes,
                OsmGroupType::Ways => BlockType::Ways,
                OsmGroupType::Relations => BlockType::Relations,
            }
        } else {
            panic!("unknown blob type");
        };

        Ok(BlockIndex {
            block_type,
            compression,
            blob_start,
            blob_len,
        })
    }
}

fn read_block<F: Read + Seek, T: prost::Message + Default>(
    reader: &mut F,
    idx: &BlockIndex,
) -> Result<T, Error> {
    reader.seek(io::SeekFrom::Start(idx.blob_start as u64))?;

    // TODO: allocate buffers outside
    let mut buf = Vec::new();
    buf.resize(idx.blob_len, 0);
    reader.read_exact(&mut buf)?;
    let blob = osmpbf::Blob::decode(&buf)?;

    let mut blob_buf = Vec::new();
    let blob_data = match idx.compression {
        BlockCompression::Uncompressed => blob.raw.as_ref().unwrap(),
        BlockCompression::Zlib => {
            // decompress zlib data
            blob_buf.clear();
            let data: &Vec<u8> = blob.zlib_data.as_ref().unwrap();
            let mut decoder = ZlibDecoder::new(&data[..]);
            decoder.read_to_end(&mut blob_buf)?;
            &blob_buf
        }
    };

    Ok(T::decode(blob_data)?)
}

impl Iterator for OsmBlockIndexIterator {
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

fn serialize_header(
    header_block: &osmpbf::HeaderBlock,
    builder: &mut osmflat::OsmBuilder,
    stringtable: &mut Vec<u8>,
) -> Result<(), io::Error> {
    let mut header = flatdata::StructBuf::<osmflat::Header>::new();

    if let Some(ref bbox) = header_block.bbox {
        header.set_bbox_left(bbox.left);
        header.set_bbox_right(bbox.right);
        header.set_bbox_top(bbox.top);
        header.set_bbox_bottom(bbox.bottom);
    };

    header.set_required_feature_first_idx(stringtable.len() as u32);
    header.set_required_features_size(header_block.required_features.len() as u32);
    for feature in &header_block.required_features {
        stringtable.extend(feature.as_bytes());
        stringtable.push(b'\0');
    }

    header.set_optional_feature_first_idx(stringtable.len() as u32);
    header.set_optional_features_size(header_block.optional_features.len() as u32);
    for feature in &header_block.optional_features {
        stringtable.extend(feature.as_bytes());
        stringtable.push(b'\0');
    }

    if let Some(ref writingprogram) = header_block.writingprogram {
        // TODO: Should we also add our name here?
        header.set_writingprogram_idx(stringtable.len() as u32);
        stringtable.extend(writingprogram.as_bytes());
        stringtable.push(b'\0');
    }

    if let Some(ref source) = header_block.source {
        header.set_source_idx(stringtable.len() as u32);
        stringtable.extend(source.as_bytes());
        stringtable.push(b'\0');
    }

    if let Some(timestamp) = header_block.osmosis_replication_timestamp {
        header.set_osmosis_replication_timestamp(timestamp);
    }

    if let Some(number) = header_block.osmosis_replication_sequence_number {
        header.set_osmosis_replication_sequence_number(number);
    }

    if let Some(ref url) = header_block.osmosis_replication_base_url {
        header.set_osmosis_replication_base_url_idx(stringtable.len() as u32);
        stringtable.extend(url.as_bytes());
        stringtable.push(b'\0');
    }

    builder.set_header(&header)?;
    Ok(())
}

fn serialize_dense_nodes(
    block: &osmpbf::PrimitiveBlock,
    nodes: &mut flatdata::ExternalVector<osmflat::Node>,
    tags: &mut flatdata::ExternalVector<osmflat::Tag>,
    stringtable: &mut Vec<u8>,
) -> Result<(), io::Error> {
    let group = &block.primitivegroup[0];
    let dense_nodes = group.dense.as_ref().unwrap();

    let granularity = block.granularity.unwrap_or(100);
    let lat_offset = block.lat_offset.unwrap_or(0);
    let lon_offset = block.lon_offset.unwrap_or(0);
    let mut lat = 0;
    let mut lon = 0;

    let mut tags_offset = 0;

    for i in 0..dense_nodes.id.len() {
        let mut node = nodes.grow()?;
        node.set_id(dense_nodes.id[i]);

        lat += dense_nodes.lat[i];
        lon += dense_nodes.lon[i];
        node.set_lat(lat_offset + (i64::from(granularity) * lat));
        node.set_lon(lon_offset + (i64::from(granularity) * lon));

        if tags_offset < dense_nodes.keys_vals.len() {
            node.set_tag_first_idx(tags.len() as u32);
            loop {
                let k = dense_nodes.keys_vals[tags_offset];
                if k == 0 {
                    break; // separator
                }
                let v = dense_nodes.keys_vals[tags_offset + 1];
                tags_offset += 2;

                let mut tag = tags.grow()?;
                tag.set_key_idx(stringtable.len() as u32);
                tag.set_value_idx(stringtable.len() as u32);

                stringtable.extend(&block.stringtable.s[k as usize]);
                stringtable.push(b'\0');
                stringtable.extend(&block.stringtable.s[v as usize]);
                stringtable.push(b'\0');
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BlockType {
    Header,
    Nodes,
    DenseNodes,
    Ways,
    Relations,
}

/// Reads the pbf file at the given path and builds an index of block types.
///
/// The index is sorted lexicographically by block type and position in the pbf
/// file.
fn build_block_index<P: AsRef<Path>>(path: P) -> Result<Vec<BlockIndex>, Error> {
    let mut index: Vec<_> = OsmBlockIndexIterator::new(path)?
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

fn run() -> Result<(), Error> {
    let args = parse_args();

    let storage = Rc::new(RefCell::new(FileResourceStorage::new(
        args.arg_output.clone().into(),
    )));
    let mut builder = osmflat::OsmBuilder::new(storage)?;

    // fill in dummy data for now
    let mut nodes = builder.start_nodes()?;
    let mut ways = builder.start_ways()?;
    let mut relations = builder.start_relations()?;
    let mut relation_members = builder.start_relation_members()?;
    let mut tags = builder.start_tags()?;
    let mut infos = builder.start_infos()?;

    // TODO: Would be nice not store all these strings in memory, but to flush them
    // from time to time to disk.
    let mut stringtable = Vec::new();
    stringtable.push(b'\0');

    let block_index = build_block_index(args.arg_input.clone())?;

    // TODO: move out into a function
    let groups = block_index.into_iter().group_by(|b| b.block_type);
    let mut pbf_header = None;
    let mut pbf_nodes = None;
    let mut pbf_dense_nodes = None;
    let mut pbf_ways = None;
    let mut pbf_relations = None;
    for (block_type, blocks) in &groups {
        match block_type {
            BlockType::Header => pbf_header = Some(blocks),
            BlockType::Nodes => pbf_nodes = Some(blocks),
            BlockType::DenseNodes => pbf_dense_nodes = Some(blocks),
            BlockType::Ways => pbf_ways = Some(blocks),
            BlockType::Relations => pbf_relations = Some(blocks),
        }
    }

    let mut file = File::open(args.arg_input)?;

    // Serialize header
    let mut index = pbf_header.ok_or_else(|| format_err!("missing header block"))?;
    let idx = index.next();
    let pbf_header: osmpbf::HeaderBlock = read_block(&mut file, &idx.unwrap())?;
    serialize_header(&pbf_header, &mut builder, &mut stringtable)?;
    ensure!(
        index.next().is_none(),
        "found multiple header blocks, which is not supported."
    );

    // Serialize nodes
    ensure!(
        pbf_nodes.is_none(),
        format_err!("found nodes, only dense nodes are supported now")
    );

    // Serialize dense nodes
    let index = pbf_dense_nodes.ok_or_else(|| format_err!("missing dense nodes"))?;
    for idx in index {
        let block: osmpbf::PrimitiveBlock = read_block(&mut file, &idx)?;
        serialize_dense_nodes(&block, &mut nodes, &mut tags, &mut stringtable)?;
    }

    // Serialize ways
    if let Some(_pbf_ways) = pbf_ways {
        println!("found ways => skipping since not implemented yet");
    }

    // Serialize ways
    if let Some(_pbf_relations) = pbf_relations {
        println!("found relations => skipping since not implemented yet");
    }

    // Finalize data structures
    stringtable.push(b'\0'); // add sentinel
    builder.set_stringtable(&stringtable)?;

    nodes.close()?;
    ways.close()?;
    relations.close()?;
    relation_members.close()?;
    tags.close()?;
    infos.close()?;

    println!(
        r#"Serialized:
  nodes: {},
  ways: {},
  relations: {}"#,
        nodes.len(),
        ways.len(),
        relations.len()
    );

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
}
