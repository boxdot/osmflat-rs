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

struct OsmPbfIterator {
    reader: BufReader<File>,
    file_buf: Vec<u8>,
    blob_buf: Vec<u8>,
    is_open: bool,
}

impl OsmPbfIterator {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<OsmPbfIterator, Error> {
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

    Ok(builder.set_header(&header)?)
}

fn serialize_datablock(
    block: &osmpbf::PrimitiveBlock,
    builder: &mut osmflat::OsmBuilder,
    nodes: &mut flatdata::ExternalVector<osmflat::Node>,
    tags: &mut flatdata::ExternalVector<osmflat::Tag>,
    stringtable: &mut Vec<u8>,
) -> Result<(), io::Error> {
    assert_eq!(block.primitivegroup.len(), 1);
    let group_type = get_group_type(&block.primitivegroup[0]);
    if group_type == OsmGroupType::DenseNodes {
        serialize_dense_nodes(block, builder, nodes, tags, stringtable)
    } else {
        Ok(println!(
            "Serialization for {:?} is not yet implemented",
            group_type
        ))
    }
}

fn serialize_dense_nodes(
    block: &osmpbf::PrimitiveBlock,
    _builder: &mut osmflat::OsmBuilder,
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
        node.set_lat(lat_offset + (granularity as i64 * lat));
        node.set_lon(lon_offset + (granularity as i64 * lon));

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

fn run() -> Result<(), Error> {
    let args = parse_args();

    let storage = Rc::new(RefCell::new(FileResourceStorage::new(
        args.arg_output.into(),
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

    // TODO: For now we make two cvery rude assumptions that:
    //
    // * the blocks are written in the order: Header, Nodes, Ways, Relations;
    // * every primitivegroup contains a single element in a block.
    for block in OsmPbfIterator::new(args.arg_input.clone())? {
        let block = block?;
        match block {
            OsmPbfBlock::OsmHeader(header) => {
                serialize_header(&header, &mut builder, &mut stringtable)?;
            }
            OsmPbfBlock::OsmData(block) => {
                serialize_datablock(
                    &block,
                    &mut builder,
                    &mut nodes,
                    &mut tags,
                    &mut stringtable,
                )?;
            }
        }
    }

    stringtable.push(b'\0'); // add sentinel
    builder.set_stringtable(&stringtable)?;

    nodes.close()?;
    ways.close()?;
    relations.close()?;
    relation_members.close()?;
    tags.close()?;
    infos.close()?;

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
}
