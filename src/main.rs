#[macro_use]
extern crate failure;
#[macro_use]
extern crate flatdata;
#[macro_use]
extern crate log;
#[macro_use]
extern crate prost_derive;
#[cfg(test)]
#[macro_use]
extern crate proptest;
#[macro_use]
extern crate structopt;

mod args;
mod ids;
mod osmflat;
mod osmpbf;
mod parallel;
mod stats;
mod strings;

use crate::osmpbf::{build_block_index, read_block, BlockIndex, BlockType};
use crate::stats::Stats;
use crate::strings::StringTable;

use colored::*;
use failure::Error;
use flatdata::{ArchiveBuilder, FileResourceStorage};
use itertools::Itertools;
use pbr::ProgressBar;
use structopt::StructOpt;

use std::collections::{hash_map, HashMap};
use std::fs::File;
use std::io;
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
    dedup: HashMap<(u64, u64), u64>, // deduplication table: (key_idx, val_idx) -> pos
}

impl<'a> TagSerializer<'a> {
    fn new(builder: &'a osmflat::OsmBuilder) -> io::Result<Self> {
        Ok(Self {
            tags: builder.start_tags()?,
            tags_index: builder.start_tags_index()?,
            dedup: HashMap::new(),
        })
    }

    fn serialize(&mut self, key_idx: u64, val_idx: u64) -> Result<(), Error> {
        let idx = match self.dedup.entry((key_idx, val_idx)) {
            hash_map::Entry::Occupied(entry) => *entry.get(),
            hash_map::Entry::Vacant(entry) => {
                let idx = self.tags.len() as u64;
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

    fn next_index(&self) -> u64 {
        self.tags_index.len() as u64
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
) -> Result<Vec<u64>, Error> {
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
                    tags_offset += 1;

                    if k == 0 {
                        break; // separator
                    }

                    let v = dense_nodes.keys_vals[tags_offset];
                    tags_offset += 1;

                    tags.serialize(string_refs[k as usize], string_refs[v as usize])?;
                }
            }
        }
        assert_eq!(tags_offset, dense_nodes.keys_vals.len());
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

            way.set_ref_first_idx(nodes_index.len() as u64);
            let mut node_ref = 0;
            for delta in &pbf_way.refs {
                node_ref += delta;
                let idx = nodes_id_to_idx.get(node_ref as u64).unwrap_or_else(|| {
                    stats.num_unresolved_node_ids += 1;
                    osmflat::INVALID_IDX
                });
                nodes_index.grow()?.set_value(idx);
            }
        }
        stats.num_ways += group.ways.len();
    }
    Ok(stats)
}

fn build_relations_index<I, FileCreator>(
    create_file: FileCreator,
    block_index: I,
) -> Result<ids::IdTable, Error>
where
    I: ExactSizeIterator<Item = BlockIndex> + Send + 'static,
    FileCreator: Fn() -> Result<File, Error>,
{
    let mut result = ids::IdTableBuilder::new();
    let mut pb = ProgressBar::new(block_index.len() as u64);
    pb.message("Building relations index...");
    parallel::parallel_process(
        block_index,
        create_file,
        |mut file, idx| read_block(&mut file, &idx),
        |data: Result<osmpbf::PrimitiveBlock, io::Error>| -> Result<(), Error> {
            for group in &data?.primitivegroup {
                for relation in &group.relations {
                    result.insert(relation.id as u64);
                }
            }
            pb.inc();
            Ok(())
        },
    )?;

    Ok(result.build())
}

fn serialize_relations(
    block: &osmpbf::PrimitiveBlock,
    nodes_id_to_idx: &ids::IdTable,
    ways_id_to_idx: &ids::IdTable,
    relations_id_to_idx: &ids::IdTable,
    stringtable: &mut StringTable,
    relations: &mut flatdata::ExternalVector<osmflat::Relation>,
    relation_members: &mut flatdata::MultiVector<osmflat::IndexType40, osmflat::RelationMembers>,
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
                        let idx = nodes_id_to_idx.get(memid as u64).unwrap_or_else(|| {
                            stats.num_unresolved_node_ids += 1;
                            osmflat::INVALID_IDX
                        });

                        let mut member = members.add_node_member();
                        member.set_node_idx(idx);
                        member.set_role_idx(string_refs[pbf_relation.roles_sid[i] as usize]);
                    }
                    osmpbf::relation::MemberType::Way => {
                        let idx = ways_id_to_idx.get(memid as u64).unwrap_or_else(|| {
                            stats.num_unresolved_way_ids += 1;
                            osmflat::INVALID_IDX
                        });

                        let mut member = members.add_way_member();
                        member.set_way_idx(idx);
                        member.set_role_idx(string_refs[pbf_relation.roles_sid[i] as usize]);
                    }
                    osmpbf::relation::MemberType::Relation => {
                        let idx = relations_id_to_idx.get(memid as u64).unwrap_or_else(|| {
                            stats.num_unresolved_rel_ids += 1;
                            osmflat::INVALID_IDX
                        });

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

fn serialize_dense_node_blocks<FileCreator>(
    builder: &osmflat::OsmBuilder,
    blocks: Vec<BlockIndex>,
    create_file: FileCreator,
    tags: &mut TagSerializer,
    stringtable: &mut StringTable,
    stats: &mut Stats,
) -> Result<ids::IdTable, Error>
where
    FileCreator: Fn() -> Result<File, Error>,
{
    let mut nodes_id_to_idx = ids::IdTableBuilder::new();
    let mut nodes = builder.start_nodes()?;
    let mut pb = ProgressBar::new(blocks.len() as u64);
    pb.message("Converting dense nodes...");
    parallel::parallel_process(
        blocks.into_iter(),
        create_file,
        |mut file, idx| read_block(&mut file, &idx),
        |data| -> Result<(), Error> {
            *stats +=
                serialize_dense_nodes(&data?, &mut nodes, &mut nodes_id_to_idx, stringtable, tags)?;
            pb.inc();
            Ok(())
        },
    )?;
    // fill tag_first_idx of the sentry, since it contains the end of the tag range
    // of the last node
    nodes.grow()?.set_tag_first_idx(tags.next_index());
    nodes.close()?;
    info!("Dense nodes converted.");
    info!("Building dense nodes index...");
    let nodes_id_to_idx = nodes_id_to_idx.build();
    info!("Dense nodes index built.");
    Ok(nodes_id_to_idx)
}

fn serialize_way_blocks<FileCreator>(
    builder: &osmflat::OsmBuilder,
    blocks: Vec<BlockIndex>,
    create_file: FileCreator,
    nodes_id_to_idx: &ids::IdTable,
    tags: &mut TagSerializer,
    stringtable: &mut StringTable,
    stats: &mut Stats,
) -> Result<ids::IdTable, Error>
where
    FileCreator: Fn() -> Result<File, Error>,
{
    let mut ways_id_to_idx = ids::IdTableBuilder::new();
    let mut ways = builder.start_ways()?;
    let mut pb = ProgressBar::new(blocks.len() as u64);
    let mut nodes_index = builder.start_nodes_index()?;
    pb.message("Converting ways...");
    parallel::parallel_process(
        blocks.into_iter(),
        create_file,
        |mut file, idx| read_block(&mut file, &idx),
        |data| -> Result<(), Error> {
            *stats += serialize_ways(
                &data?,
                &nodes_id_to_idx,
                &mut ways,
                &mut ways_id_to_idx,
                stringtable,
                tags,
                &mut nodes_index,
            )?;
            pb.inc();
            Ok(())
        },
    )?;

    {
        let mut sentinel = ways.grow()?;
        sentinel.set_tag_first_idx(tags.next_index());
        sentinel.set_ref_first_idx(nodes_index.len() as u64);
    }
    ways.close()?;
    nodes_index.close()?;

    info!("Ways converted.");
    info!("Building ways index...");
    let ways_id_to_idx = ways_id_to_idx.build();
    info!("Way index built.");
    Ok(ways_id_to_idx)
}

fn serialize_relation_blocks<FileCreator>(
    builder: &osmflat::OsmBuilder,
    blocks: Vec<BlockIndex>,
    create_file: FileCreator,
    nodes_id_to_idx: &ids::IdTable,
    ways_id_to_idx: &ids::IdTable,
    tags: &mut TagSerializer,
    stringtable: &mut StringTable,
    stats: &mut Stats,
) -> Result<(), Error>
where
    FileCreator: Fn() -> Result<File, Error> + Copy,
{
    // We need to build the index of relation ids first, since relations can refer
    // again to relations.
    let relations_id_to_idx = build_relations_index(create_file, blocks.clone().into_iter())?;

    let mut relations = builder.start_relations()?;
    let mut relation_members = builder.start_relation_members()?;

    let mut pb = ProgressBar::new(blocks.len() as u64);
    pb.message("Converting relations...");
    parallel::parallel_process(
        blocks.into_iter(),
        create_file,
        |mut file, idx| read_block(&mut file, &idx),
        |data| -> Result<(), Error> {
            *stats += serialize_relations(
                &data?,
                &nodes_id_to_idx,
                &ways_id_to_idx,
                &relations_id_to_idx,
                stringtable,
                &mut relations,
                &mut relation_members,
                tags,
            )?;
            pb.inc();
            Ok(())
        },
    )?;

    {
        let mut sentinel = relations.grow()?;
        sentinel.set_tag_first_idx(tags.next_index());
    }

    relations.close()?;
    relation_members.close()?;

    info!("Relations converted.");

    Ok(())
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
    let mut tags = TagSerializer::new(&builder)?;
    let infos = builder.start_infos()?; // TODO: Actually put some data in here
    info!(
        "Initialized new osmflat archive at: {}",
        &args.output.display()
    );

    info!("Building index of PBF blocks...");
    let block_index = build_block_index(&args.input)?;

    // TODO: move out into a function
    let groups = block_index.into_iter().group_by(|b| b.block_type);
    let mut pbf_header = Vec::new();
    let mut pbf_dense_nodes = Vec::new();
    let mut pbf_ways = Vec::new();
    let mut pbf_relations = Vec::new();
    for (block_type, blocks) in &groups {
        match block_type {
            BlockType::Header => pbf_header = blocks.collect(),
            BlockType::Nodes => panic!("Found nodes block, only dense nodes are supported now"),
            BlockType::DenseNodes => pbf_dense_nodes = blocks.collect(),
            BlockType::Ways => pbf_ways = blocks.collect(),
            BlockType::Relations => pbf_relations = blocks.collect(),
        }
    }
    info!("PBF block index built.");

    let mut file = File::open(&args.input)?;

    // Serialize header
    if pbf_header.len() != 1 {
        return Err(format_err!(
            "Require exactly one header block, but found {}",
            pbf_header.len()
        ));
    }
    let idx = &pbf_header[0];
    let pbf_header: osmpbf::HeaderBlock = read_block(&mut file, &idx)?;
    serialize_header(&pbf_header, &builder, &mut stringtable)?;
    info!("Header written.");

    let mut stats = Stats::default();
    let create_input_file = || -> Result<File, Error> { Ok(File::open(&args.input)?) };

    let nodes_id_to_idx = serialize_dense_node_blocks(
        &builder,
        pbf_dense_nodes,
        create_input_file,
        &mut tags,
        &mut stringtable,
        &mut stats,
    )?;

    let ways_id_to_idx = serialize_way_blocks(
        &builder,
        pbf_ways,
        create_input_file,
        &nodes_id_to_idx,
        &mut tags,
        &mut stringtable,
        &mut stats,
    )?;

    serialize_relation_blocks(
        &builder,
        pbf_relations,
        create_input_file,
        &nodes_id_to_idx,
        &ways_id_to_idx,
        &mut tags,
        &mut stringtable,
        &mut stats,
    )?;

    // Finalize data structures
    tags.close(); // drop the reference to stringtable

    info!("Writing stringtable to disk...");
    builder.set_stringtable(&stringtable.into_bytes())?;

    infos.close()?;

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
