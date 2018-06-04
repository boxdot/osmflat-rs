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
#[cfg(test)]
#[macro_use]
extern crate proptest;

mod args;
mod osmflat;
mod osmpbf;
mod stats;
mod strings;

use args::parse_args;
use osmpbf::{build_block_index, read_block, BlockIndex, BlockType};
use stats::Stats;
use strings::StringTable;

use colored::*;
use failure::Error;
use flatdata::{ArchiveBuilder, FileResourceStorage};
use itertools::Itertools;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek};
use std::rc::Rc;
use std::str;

fn serialize_header(
    header_block: &osmpbf::HeaderBlock,
    builder: &mut osmflat::OsmBuilder,
    stringtable: &mut StringTable,
) -> Result<(), io::Error> {
    let mut header = flatdata::StructBuf::<osmflat::Header>::new();

    if let Some(ref bbox) = header_block.bbox {
        header.set_bbox_left(bbox.left);
        header.set_bbox_right(bbox.right);
        header.set_bbox_top(bbox.top);
        header.set_bbox_bottom(bbox.bottom);
    };

    header.set_required_feature_first_idx(stringtable.next_index());
    header.set_required_features_size(header_block.required_features.len() as u32);
    for feature in &header_block.required_features {
        stringtable.push(feature.clone());
    }

    header.set_optional_feature_first_idx(stringtable.next_index());
    header.set_optional_features_size(header_block.optional_features.len() as u32);
    for feature in &header_block.optional_features {
        stringtable.push(feature.clone());
    }

    if let Some(ref writingprogram) = header_block.writingprogram {
        // TODO: Should we also add our name here?
        header.set_writingprogram_idx(stringtable.push(writingprogram.clone()));
    }

    if let Some(ref source) = header_block.source {
        header.set_source_idx(stringtable.push(source.clone()));
    }

    if let Some(timestamp) = header_block.osmosis_replication_timestamp {
        header.set_osmosis_replication_timestamp(timestamp);
    }

    if let Some(number) = header_block.osmosis_replication_sequence_number {
        header.set_osmosis_replication_sequence_number(number);
    }

    if let Some(ref url) = header_block.osmosis_replication_base_url {
        header.set_osmosis_replication_base_url_idx(stringtable.push(url.clone()));
    }

    builder.set_header(&header)?;
    Ok(())
}

fn serialize_tag(
    pbf_stringtable: &osmpbf::StringTable,
    k: u32,
    v: u32,
    tags: &mut flatdata::ExternalVector<osmflat::Tag>,
    stringtable: &mut StringTable,
) -> Result<u32, Error> {
    let idx = tags.len() as u32;
    let mut tag = tags.grow()?;
    let key = str::from_utf8(&pbf_stringtable.s[k as usize])?;
    tag.set_key_idx(stringtable.insert(key));
    let val = str::from_utf8(&pbf_stringtable.s[v as usize])?;
    tag.set_value_idx(stringtable.insert(val));
    Ok(idx)
}

fn serialize_dense_nodes(
    block: &osmpbf::PrimitiveBlock,
    nodes: &mut flatdata::ExternalVector<osmflat::Node>,
    nodes_id_to_idx: &mut HashMap<i64, u32>,
    tags: &mut flatdata::ExternalVector<osmflat::Tag>,
    stringtable: &mut StringTable,
) -> Result<Stats, Error> {
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

                serialize_tag(&block.stringtable, k as u32, v as u32, tags, stringtable)?;
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
    stringtable: &mut StringTable,
) -> Result<Stats, Error> {
    let mut stats = Stats::default();
    for group in &block.primitivegroup {
        for pbf_way in &group.ways {
            ways_id_to_idx.insert(pbf_way.id, ways.len() as u32);

            let mut way = ways.grow()?;
            way.set_id(pbf_way.id);

            debug_assert_eq!(pbf_way.keys.len(), pbf_way.vals.len(), "invalid input data");
            way.set_tag_first_idx(tags.len() as u32);

            for i in 0..pbf_way.keys.len() {
                serialize_tag(
                    &block.stringtable,
                    pbf_way.keys[i],
                    pbf_way.vals[i],
                    tags,
                    stringtable,
                )?;
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
    stringtable: &mut StringTable,
) -> Result<Stats, Error> {
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
                serialize_tag(
                    &block.stringtable,
                    pbf_relation.keys[i],
                    pbf_relation.vals[i],
                    tags,
                    stringtable,
                )?;
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
                        let role = str::from_utf8(
                            &block.stringtable.s[pbf_relation.roles_sid[i] as usize],
                        )?;
                        member.set_role_idx(stringtable.insert(role));
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
                        let role = str::from_utf8(
                            &block.stringtable.s[pbf_relation.roles_sid[i] as usize],
                        )?;
                        member.set_role_idx(stringtable.insert(role));
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
                        let role = str::from_utf8(
                            &block.stringtable.s[pbf_relation.roles_sid[i] as usize],
                        )?;
                        member.set_role_idx(stringtable.insert(role));
                    }
                }
            }
            stats.num_relations += 1;
        }
    }
    Ok(stats)
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
    let mut stringtable = StringTable::new();
    stringtable.push("");

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

        for idx in index {
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
    builder.set_stringtable(&stringtable.into_bytes())?;

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
