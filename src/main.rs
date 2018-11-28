extern crate byteorder;
extern crate bytes;
extern crate colored;
#[macro_use]
extern crate failure;
extern crate flate2;
#[macro_use]
extern crate flatdata;
extern crate itertools;
#[macro_use]
extern crate log;
extern crate prost;
#[macro_use]
extern crate prost_derive;
extern crate pbr;
#[cfg(test)]
#[macro_use]
extern crate proptest;
extern crate stderrlog;
#[macro_use]
extern crate structopt;

mod args;
mod ids;
mod osmflat;
mod osmpbf;
mod stats;
mod strings;

use osmpbf::{build_block_index, read_block, BlockIndex, BlockType};
use stats::Stats;
use strings::StringTable;

use colored::*;
use failure::Error;
use flatdata::{ArchiveBuilder, FileResourceStorage};
use itertools::Itertools;
use pbr::ProgressBar;
use structopt::StructOpt;

use std::collections::{hash_map, HashMap};
use std::fs::File;
use std::io::{self, Read, Seek};
use std::str;

fn serialize_header(
    header_block: &osmpbf::HeaderBlock,
    builder: &osmflat::OsmBuilder,
    stringtable: &mut StringTable,
) -> io::Result<()> {
    let mut header_buf = flatdata::StructBuf::<osmflat::Header>::new();
    let mut header = header_buf.get_mut();

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

    builder.set_header(header.into_ref())?;
    Ok(())
}

/// Holds tags external vector and deduplicates tags.
struct TagSerializer<'a> {
    tags: flatdata::ExternalVector<'a, osmflat::Tag>,
    tags_index: flatdata::ExternalVector<'a, osmflat::TagIndex>,
    dedup: HashMap<(u32, u32), u32>, // deduplication table: (key_idx, val_idx) -> pos
}

impl<'a> TagSerializer<'a> {
    fn new(builder: &'a osmflat::OsmBuilder) -> io::Result<Self> {
        Ok(Self {
            tags: builder.start_tags()?,
            tags_index: builder.start_tags_index()?,
            dedup: HashMap::new(),
        })
    }

    fn serialize(&mut self, key_idx: u32, val_idx: u32) -> Result<(), Error> {
        let idx = match self.dedup.entry((key_idx, val_idx)) {
            hash_map::Entry::Occupied(entry) => *entry.get(),
            hash_map::Entry::Vacant(entry) => {
                let idx = self.tags.len() as u32;
                let mut tag = self.tags.grow()?;
                tag.set_key_idx(key_idx);
                tag.set_value_idx(val_idx);
                entry.insert(idx);
                idx
            }
        };

        let mut tag_index = self.tags_index.grow()?;
        tag_index.set_value(idx);

        Ok(())
    }

    fn next_index(&self) -> u32 {
        self.tags_index.len() as u32
    }

    fn close(self) {
        if let Err(e) = self.tags.close() {
            panic!("failed to close tags: {}", e);
        }
        if let Err(e) = self.tags_index.close() {
            panic!("failed to close tags index: {}", e);
        }
    }
}

/// adds all strings in a table to the lookup and returns a vectors of
/// references to be used instead
fn add_string_table(
    pbf_stringtable: &osmpbf::StringTable,
    stringtable: &mut StringTable,
) -> Result<Vec<u32>, Error> {
    let mut result = Vec::new();
    for x in &pbf_stringtable.s {
        let string = str::from_utf8(&x)?;
        result.push(stringtable.insert(string));
    }
    Ok(result)
}

fn serialize_dense_nodes(
    block: &osmpbf::PrimitiveBlock,
    nodes: &mut flatdata::ExternalVector<osmflat::Node>,
    nodes_id_to_idx: &mut ids::IdTableBuilder,
    stringtable: &mut StringTable,
    tags: &mut TagSerializer,
) -> Result<Stats, Error> {
    let mut stats = Stats::default();
    let string_refs = add_string_table(&block.stringtable, stringtable)?;
    for group in block.primitivegroup.iter() {
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

            let index = nodes_id_to_idx.insert(id as u64);
            assert_eq!(index as usize, nodes.len());

            let mut node = nodes.grow()?;
            node.set_id(id);

            lat += dense_nodes.lat[i];
            lon += dense_nodes.lon[i];
            node.set_lat(lat_offset + (i64::from(granularity) * lat));
            node.set_lon(lon_offset + (i64::from(granularity) * lon));

            if tags_offset < dense_nodes.keys_vals.len() {
                node.set_tag_first_idx(tags.next_index());
                loop {
                    let k = dense_nodes.keys_vals[tags_offset];
                    if k == 0 {
                        break; // separator
                    }
                    let v = dense_nodes.keys_vals[tags_offset + 1];
                    tags_offset += 2;
                    tags.serialize(string_refs[k as usize], string_refs[v as usize])?;
                }
            }
        }
        stats.num_nodes += dense_nodes.id.len();
    }
    Ok(stats)
}

fn serialize_ways(
    block: &osmpbf::PrimitiveBlock,
    nodes_id_to_idx: &ids::IdTable,
    ways: &mut flatdata::ExternalVector<osmflat::Way>,
    ways_id_to_idx: &mut ids::IdTableBuilder,
    stringtable: &mut StringTable,
    tags: &mut TagSerializer,
    nodes_index: &mut flatdata::ExternalVector<osmflat::NodeIndex>,
) -> Result<Stats, Error> {
    let mut stats = Stats::default();
    let string_refs = add_string_table(&block.stringtable, stringtable)?;
    for group in &block.primitivegroup {
        for pbf_way in &group.ways {
            let index = ways_id_to_idx.insert(pbf_way.id as u64);
            assert_eq!(index as usize, ways.len());

            let mut way = ways.grow()?;
            way.set_id(pbf_way.id);

            debug_assert_eq!(pbf_way.keys.len(), pbf_way.vals.len(), "invalid input data");
            way.set_tag_first_idx(tags.next_index());

            for i in 0..pbf_way.keys.len() {
                tags.serialize(
                    string_refs[pbf_way.keys[i] as usize],
                    string_refs[pbf_way.vals[i] as usize],
                )?;
            }

            // TODO: serialize info

            way.set_ref_first_idx(nodes_index.len() as u32);
            let mut node_ref = 0;
            for delta in &pbf_way.refs {
                node_ref += delta;
                let mut node_idx = nodes_index.grow()?;
                let idx = match nodes_id_to_idx.get(node_ref as u64) {
                    Some(idx) => idx,
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
) -> Result<ids::IdTable, Error> {
    let mut result = ids::IdTableBuilder::new();
    result.skip(1); // Id 0 is reserved elsewhere
    for block_idx in block_index {
        let block: osmpbf::PrimitiveBlock = read_block(reader, &block_idx)?;
        for group in &block.primitivegroup {
            for relation in &group.relations {
                result.insert(relation.id as u64);
            }
        }
    }
    Ok(result.build())
}

fn serialize_relations(
    block: &osmpbf::PrimitiveBlock,
    nodes_id_to_idx: &ids::IdTable,
    ways_id_to_idx: &ids::IdTable,
    relations_id_to_idx: &ids::IdTable,
    stringtable: &mut StringTable,
    relations: &mut flatdata::ExternalVector<osmflat::Relation>,
    relation_members: &mut flatdata::MultiVector<osmflat::IndexType32, osmflat::RelationMembers>,
    tags: &mut TagSerializer,
) -> Result<Stats, Error> {
    let mut stats = Stats::default();
    let string_refs = add_string_table(&block.stringtable, stringtable)?;
    for group in &block.primitivegroup {
        for pbf_relation in &group.relations {
            let mut relation = relations.grow()?;
            relation.set_id(pbf_relation.id);

            debug_assert_eq!(
                pbf_relation.keys.len(),
                pbf_relation.vals.len(),
                "invalid input data"
            );
            relation.set_tag_first_idx(tags.next_index());
            for i in 0..pbf_relation.keys.len() {
                tags.serialize(
                    string_refs[pbf_relation.keys[i] as usize],
                    string_refs[pbf_relation.vals[i] as usize],
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
                        let idx = match nodes_id_to_idx.get(memid as u64) {
                            Some(idx) => idx,
                            None => {
                                stats.num_unresolved_node_ids += 1;
                                osmflat::INVALID_IDX
                            }
                        };

                        let mut member = members.add_node_member();
                        member.set_node_idx(idx);
                        member.set_role_idx(string_refs[pbf_relation.roles_sid[i] as usize]);
                    }
                    osmpbf::relation::MemberType::Way => {
                        let idx = match ways_id_to_idx.get(memid as u64) {
                            Some(idx) => idx,
                            None => {
                                stats.num_unresolved_way_ids += 1;
                                osmflat::INVALID_IDX
                            }
                        };

                        let mut member = members.add_way_member();
                        member.set_way_idx(idx);
                        member.set_role_idx(string_refs[pbf_relation.roles_sid[i] as usize]);
                    }
                    osmpbf::relation::MemberType::Relation => {
                        let idx = match relations_id_to_idx.get(memid as u64) {
                            Some(idx) => idx,
                            None => {
                                stats.num_unresolved_rel_ids += 1;
                                osmflat::INVALID_IDX
                            }
                        };

                        let mut member = members.add_relation_member();
                        member.set_relation_idx(idx);
                        member.set_role_idx(string_refs[pbf_relation.roles_sid[i] as usize]);
                    }
                }
            }
            stats.num_relations += 1;
        }
    }
    Ok(stats)
}

fn run() -> Result<(), Error> {
    let args = args::Args::from_args();
    stderrlog::new()
        .module(module_path!())
        .timestamp(stderrlog::Timestamp::Second)
        .verbosity(args.verbose as usize + 2)
        .init()
        .unwrap();

    let storage = FileResourceStorage::new(args.output.clone());
    let builder = osmflat::OsmBuilder::new(storage)?;

    // TODO: Would be nice not store all these strings in memory, but to flush them
    // from time to time to disk.
    let mut stringtable = StringTable::new();
    stringtable.push("");
    let mut tags = TagSerializer::new(&builder)?;
    let infos = builder.start_infos()?; // TODO: Actually put some data in here
    let mut nodes_index = builder.start_nodes_index()?;
    info!(
        "Initialized new osmflat archive at: {}",
        &args.output.display()
    );

    info!("Building index of PBF blocks...");
    let block_index = build_block_index(&args.input)?;

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
    info!("PBF block index built.");

    let mut file = File::open(args.input)?;

    // Serialize header
    let mut index = pbf_header.ok_or_else(|| format_err!("missing header block"))?;
    let idx = index.next();
    let pbf_header: osmpbf::HeaderBlock = read_block(&mut file, &idx.unwrap())?;
    serialize_header(&pbf_header, &builder, &mut stringtable)?;
    ensure!(
        index.next().is_none(),
        "found multiple header blocks, which is not supported."
    );
    info!("Header written.");

    // Serialize nodes
    // TODO: Implement!
    ensure!(
        pbf_nodes.is_none(),
        format_err!("found nodes, only dense nodes are supported now")
    );

    let mut stats = Stats::default();

    // Serialize dense nodes
    let mut nodes_id_to_idx = ids::IdTableBuilder::new();
    if let Some(index) = pbf_dense_nodes {
        let index: Vec<_> = index.collect();

        let mut pb = ProgressBar::new(index.len() as u64);
        pb.message("Converting dense nodes...");
        let mut nodes = builder.start_nodes()?;
        for idx in index {
            let block: osmpbf::PrimitiveBlock = read_block(&mut file, &idx)?;
            stats += serialize_dense_nodes(
                &block,
                &mut nodes,
                &mut nodes_id_to_idx,
                &mut stringtable,
                &mut tags,
            )?;
            pb.inc();
        }
        {
            let mut sentinel = nodes.grow()?;
            sentinel.set_tag_first_idx(tags.next_index());
        }
        nodes.close()?;
        pb.finish();
    }
    info!("Dense nodes converted.");
    let nodes_id_to_idx = nodes_id_to_idx.build();
    info!("Dense index build.");

    // Serialize ways
    let mut ways_id_to_idx = ids::IdTableBuilder::new();
    if let Some(index) = pbf_ways {
        let index: Vec<_> = index.collect();

        let mut pb = ProgressBar::new(index.len() as u64);
        pb.message("Converting ways...");

        let mut ways = builder.start_ways()?;
        ways.grow()?; // index 0 is reserved for invalid way
        ways_id_to_idx.skip(1);

        for idx in index {
            let block: osmpbf::PrimitiveBlock = read_block(&mut file, &idx)?;
            stats += serialize_ways(
                &block,
                &nodes_id_to_idx,
                &mut ways,
                &mut ways_id_to_idx,
                &mut stringtable,
                &mut tags,
                &mut nodes_index,
            )?;
            pb.inc();
        }
        {
            let mut sentinel = ways.grow()?;
            sentinel.set_tag_first_idx(tags.next_index());
            sentinel.set_ref_first_idx(nodes_index.len() as u32);
        }
        ways.close()?;
        pb.finish();
    };
    info!("Ways converted.");
    let ways_id_to_idx = ways_id_to_idx.build();
    info!("Way index build.");

    // Serialize relations
    if let Some(index) = pbf_relations {
        let index: Vec<_> = index.collect();

        info!("Building relations index...");

        // We need to build the index of relation ids first, since relations can refer
        // again to relations.
        let relations_id_to_idx = build_relations_index(&mut file, index.iter())?;
        info!("Relations index built.");

        let mut pb = ProgressBar::new(index.len() as u64);
        pb.message("Converting relations...");

        let mut relations = builder.start_relations()?;
        relations.grow()?; // index 0 is reserved for invalid relation

        let mut relation_members = builder.start_relation_members()?;
        relation_members.grow()?; // index 0 is ALSO reserved for invalid relation

        for idx in index {
            let block: osmpbf::PrimitiveBlock = read_block(&mut file, &idx)?;
            stats += serialize_relations(
                &block,
                &nodes_id_to_idx,
                &ways_id_to_idx,
                &relations_id_to_idx,
                &mut stringtable,
                &mut relations,
                &mut relation_members,
                &mut tags,
            )?;
            pb.inc();
        }
        {
            let mut sentinel = relations.grow()?;
            sentinel.set_tag_first_idx(tags.next_index());
        }

        relations.close()?;
        relation_members.close()?;
        pb.finish();
    };
    info!("Relations converted.");

    // Finalize data structures
    tags.close(); // drop the reference to stringtable

    info!("Writing stringtable to disk...");
    builder.set_stringtable(&stringtable.into_bytes())?;

    infos.close()?;
    nodes_index.close()?;

    info!("osmflat archive built.");

    println!("{}", stats);
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
}
