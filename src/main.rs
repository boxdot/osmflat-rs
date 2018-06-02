// TODO:
//
// 1. Deduplicate strings: size of the stringtable for Berlin is 40M without
//    dedup.

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
extern crate bytes;

mod args;
mod osmflat;
mod osmpbf;
mod stats;

use args::parse_args;
use stats::Stats;

use byteorder::{ByteOrder, NetworkEndian};
use colored::*;
use failure::Error;
use flatdata::{ArchiveBuilder, FileResourceStorage};
use flate2::read::ZlibDecoder;
use itertools::Itertools;
use prost::Message;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Cursor, ErrorKind, Read, Seek, SeekFrom};
use std::path::Path;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BlockType {
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
    fn from_osmdata_blob(blob: &[u8]) -> Result<BlockType, io::Error> {
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
struct BlockIndex {
    block_type: BlockType,
    blob_start: usize,
    blob_len: usize,
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

fn read_block<F: Read + Seek, T: prost::Message + Default>(
    reader: &mut F,
    idx: &BlockIndex,
) -> Result<T, Error> {
    reader.seek(io::SeekFrom::Start(idx.blob_start as u64))?;

    // TODO: allocate buffers outside of the function
    let mut buf = Vec::new();
    buf.resize(idx.blob_len, 0);
    reader.read_exact(&mut buf)?;
    let blob = osmpbf::Blob::decode(&buf)?;

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
    nodes_id_to_idx: &mut HashMap<i64, u32>,
    tags: &mut flatdata::ExternalVector<osmflat::Tag>,
    stringtable: &mut Vec<u8>,
) -> Result<Stats, io::Error> {
    let group = &block.primitivegroup[0];
    let dense_nodes = group.dense.as_ref().unwrap();

    let granularity = block.granularity.unwrap_or(100);
    let lat_offset = block.lat_offset.unwrap_or(0);
    let lon_offset = block.lon_offset.unwrap_or(0);
    let mut lat = 0;
    let mut lon = 0;

    let mut tags_offset = 0;

    let mut id = 0;
    for i in 0..dense_nodes.id.len() {
        id += dense_nodes.id[i];

        nodes_id_to_idx.insert(id, nodes.len() as u32);

        let mut node = nodes.grow()?;
        node.set_id(id);

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
                stringtable.extend(&block.stringtable.s[k as usize]);
                stringtable.push(b'\0');
                tag.set_value_idx(stringtable.len() as u32);
                stringtable.extend(&block.stringtable.s[v as usize]);
                stringtable.push(b'\0');
            }
        }
    }
    let mut stats = Stats::default();
    stats.num_nodes = dense_nodes.id.len();
    Ok(stats)
}

fn serialize_ways(
    block: &osmpbf::PrimitiveBlock,
    nodes_id_to_idx: &HashMap<i64, u32>,
    ways: &mut flatdata::ExternalVector<osmflat::Way>,
    ways_id_to_idx: &mut HashMap<i64, u32>,
    tags: &mut flatdata::ExternalVector<osmflat::Tag>,
    nodes_index: &mut flatdata::ExternalVector<osmflat::NodeIndex>,
    stringtable: &mut Vec<u8>,
) -> Result<Stats, io::Error> {
    let mut stats = Stats::default();
    for group in &block.primitivegroup {
        for pbf_way in &group.ways {
            ways_id_to_idx.insert(pbf_way.id, ways.len() as u32);

            let mut way = ways.grow()?;
            way.set_id(pbf_way.id);

            debug_assert_eq!(pbf_way.keys.len(), pbf_way.vals.len(), "invalid input data");
            way.set_tag_first_idx(tags.len() as u32);

            for i in 0..pbf_way.keys.len() {
                let mut tag = tags.grow()?;
                tag.set_key_idx(stringtable.len() as u32);
                stringtable.extend(&block.stringtable.s[pbf_way.keys[i] as usize]);
                stringtable.push(b'\0');
                tag.set_value_idx(stringtable.len() as u32);
                stringtable.extend(&block.stringtable.s[pbf_way.vals[i] as usize]);
                stringtable.push(b'\0');
            }

            // TODO: serialize info

            way.set_ref_first_idx(nodes_index.len() as u32);
            let mut node_ref = 0;
            for delta in &pbf_way.refs {
                node_ref += delta;
                let mut node_idx = nodes_index.grow()?;
                let idx = match nodes_id_to_idx.get(&node_ref) {
                    Some(idx) => *idx,
                    None => {
                        stats.num_unresolved_node_ids += 1;
                        osmflat::INVALID_IDX
                    }
                };
                node_idx.set_value(idx);
            }
        }
        stats.num_ways += group.ways.len();
    }
    Ok(stats)
}

fn build_relations_index<'a, F: Read + Seek, I: 'a + Iterator<Item = &'a BlockIndex>>(
    reader: &mut F,
    block_index: I,
) -> Result<HashMap<i64, u32>, Error> {
    let mut idx = 1; // we start counting with 1, since 0 is reserved for invalid relation.
    let mut result = HashMap::new();
    for block_idx in block_index {
        let block: osmpbf::PrimitiveBlock = read_block(reader, &block_idx)?;
        for group in &block.primitivegroup {
            for relation in &group.relations {
                result.insert(relation.id, idx);
                idx += 1;
            }
        }
    }
    Ok(result)
}

fn serialize_relations(
    block: &osmpbf::PrimitiveBlock,
    nodes_id_to_idx: &HashMap<i64, u32>,
    ways_id_to_idx: &HashMap<i64, u32>,
    relations_id_to_idx: &HashMap<i64, u32>,
    relations: &mut flatdata::ExternalVector<osmflat::Relation>,
    relation_members: &mut flatdata::MultiVector<osmflat::IndexType32, osmflat::RelationMembers>,
    tags: &mut flatdata::ExternalVector<osmflat::Tag>,
    stringtable: &mut Vec<u8>,
) -> Result<Stats, io::Error> {
    let mut stats = Stats::default();
    for group in &block.primitivegroup {
        for pbf_relation in &group.relations {
            let mut relation = relations.grow()?;
            relation.set_id(pbf_relation.id);

            debug_assert_eq!(
                pbf_relation.keys.len(),
                pbf_relation.vals.len(),
                "invalid input data"
            );
            relation.set_tag_first_idx(tags.len() as u32);
            for i in 0..pbf_relation.keys.len() {
                let mut tag = tags.grow()?;
                tag.set_key_idx(stringtable.len() as u32);
                stringtable.extend(&block.stringtable.s[pbf_relation.keys[i] as usize]);
                stringtable.push(b'\0');
                tag.set_value_idx(stringtable.len() as u32);
                stringtable.extend(&block.stringtable.s[pbf_relation.vals[i] as usize]);
                stringtable.push(b'\0');
            }

            // TODO: Serialized infos

            debug_assert!(
                pbf_relation.roles_sid.len() == pbf_relation.memids.len() &&
                pbf_relation.memids.len() == pbf_relation.types.len()
                "invalid input data"
            );

            let mut memid = 0;
            let mut members = relation_members.grow()?;
            for i in 0..pbf_relation.roles_sid.len() {
                memid += pbf_relation.memids[i];

                let member_type = osmpbf::relation::MemberType::from_i32(pbf_relation.types[i]);
                debug_assert!(member_type.is_some());

                match member_type.unwrap() {
                    osmpbf::relation::MemberType::Node => {
                        let idx = match nodes_id_to_idx.get(&memid) {
                            Some(idx) => *idx,
                            None => {
                                stats.num_unresolved_node_ids += 1;
                                osmflat::INVALID_IDX
                            }
                        };

                        let mut member = members.add_node_member();
                        member.set_node_idx(idx);
                        member.set_role_idx(stringtable.len() as u32);
                        stringtable
                            .extend(&block.stringtable.s[pbf_relation.roles_sid[i] as usize]);
                        stringtable.push(b'\0');
                    }
                    osmpbf::relation::MemberType::Way => {
                        let idx = match ways_id_to_idx.get(&memid) {
                            Some(idx) => *idx,
                            None => {
                                stats.num_unresolved_way_ids += 1;
                                osmflat::INVALID_IDX
                            }
                        };

                        let mut member = members.add_way_member();
                        member.set_way_idx(idx);
                        member.set_role_idx(stringtable.len() as u32);
                        stringtable
                            .extend(&block.stringtable.s[pbf_relation.roles_sid[i] as usize]);
                        stringtable.push(b'\0');
                    }
                    osmpbf::relation::MemberType::Relation => {
                        let idx = match relations_id_to_idx.get(&memid) {
                            Some(idx) => *idx,
                            None => {
                                stats.num_unresolved_rel_ids += 1;
                                osmflat::INVALID_IDX
                            }
                        };

                        let mut member = members.add_relation_member();
                        member.set_relation_idx(idx);
                        stringtable
                            .extend(&block.stringtable.s[pbf_relation.roles_sid[i] as usize]);
                        stringtable.push(b'\0');
                    }
                }
            }
            stats.num_relations += 1;
        }
    }
    Ok(stats)
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
    let mut builder = osmflat::OsmBuilder::new(storage.clone())?;

    // fill in dummy data for now
    let mut tags = builder.start_tags()?;
    let mut infos = builder.start_infos()?;
    let mut nodes_index = builder.start_nodes_index()?;

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

    let mut stats = Stats::default();

    // Serialize dense nodes
    let mut nodes_id_to_idx: HashMap<i64, u32> = HashMap::new();
    if let Some(index) = pbf_dense_nodes {
        let mut nodes = builder.start_nodes()?;
        for idx in index {
            let block: osmpbf::PrimitiveBlock = read_block(&mut file, &idx)?;
            stats += serialize_dense_nodes(
                &block,
                &mut nodes,
                &mut nodes_id_to_idx,
                &mut tags,
                &mut stringtable,
            )?;
        }
        {
            let mut sentinel = nodes.grow()?;
            sentinel.set_tag_first_idx(tags.len() as u32);
        }
        nodes.close()?;
    }

    // Serialize ways
    let mut ways_id_to_idx: HashMap<i64, u32> = HashMap::new();
    if let Some(index) = pbf_ways {
        let mut ways = builder.start_ways()?;
        ways.grow()?; // index 0 is reserved for invalid way

        for idx in index {
            let block: osmpbf::PrimitiveBlock = read_block(&mut file, &idx)?;
            stats += serialize_ways(
                &block,
                &nodes_id_to_idx,
                &mut ways,
                &mut ways_id_to_idx,
                &mut tags,
                &mut nodes_index,
                &mut stringtable,
            )?;
        }
        {
            let mut sentinel = ways.grow()?;
            sentinel.set_tag_first_idx(tags.len() as u32);
            sentinel.set_ref_first_idx(nodes_index.len() as u32);
        }
        ways.close()?;
    };

    // Serialize relations
    if let Some(index) = pbf_relations {
        // We need to build the index of relation ids first, since relations can refer
        // again to relations.
        let index: Vec<_> = index.collect();
        let relations_id_to_idx = build_relations_index(&mut file, index.iter())?;

        let mut relations = builder.start_relations()?;
        relations.grow()?; // index 0 is reserved for invalid relation

        let mut relation_members = builder.start_relation_members()?;

        for idx in index.into_iter() {
            let block: osmpbf::PrimitiveBlock = read_block(&mut file, &idx)?;
            stats += serialize_relations(
                &block,
                &nodes_id_to_idx,
                &ways_id_to_idx,
                &relations_id_to_idx,
                &mut relations,
                &mut relation_members,
                &mut tags,
                &mut stringtable,
            )?;
        }
        {
            let mut sentinel = relations.grow()?;
            sentinel.set_tag_first_idx(tags.len() as u32);
        }

        relations.close()?;
        relation_members.close()?;
    };

    // Finalize data structures
    stringtable.push(b'\0'); // add sentinel
    builder.set_stringtable(&stringtable)?;

    tags.close()?;
    infos.close()?;
    nodes_index.close()?;

    println!("{}", stats);
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
}
