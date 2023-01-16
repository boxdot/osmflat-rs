mod args;
mod ids;
mod osmpbf;
mod parallel;
mod stats;
mod strings;

use crate::osmpbf::{build_block_index, read_block, BlockIndex, BlockType};
use crate::stats::Stats;
use crate::strings::StringTable;

use clap::Parser;
use colored::*;
use flatdata::FileResourceStorage;
use itertools::Itertools;
use log::info;
use memmap2::Mmap;
use pbr::ProgressBar;

use ahash::AHashMap;
use std::collections::hash_map;
use std::fs::File;
use std::io;
use std::str;

type Error = Box<dyn std::error::Error>;

fn serialize_header(
    header_block: &osmpbf::HeaderBlock,
    coord_scale: i32,
    builder: &osmflat::OsmBuilder,
    stringtable: &mut StringTable,
) -> io::Result<()> {
    let mut header = osmflat::Header::new();

    header.set_coord_scale(coord_scale);

    if let Some(ref bbox) = header_block.bbox {
        header.set_bbox_left((bbox.left / (1000000000 / coord_scale) as i64) as i32);
        header.set_bbox_right((bbox.right / (1000000000 / coord_scale) as i64) as i32);
        header.set_bbox_top((bbox.top / (1000000000 / coord_scale) as i64) as i32);
        header.set_bbox_bottom((bbox.bottom / (1000000000 / coord_scale) as i64) as i32);
    };

    header.set_writingprogram_idx(stringtable.insert("osmflatc"));

    if let Some(ref source) = header_block.source {
        header.set_source_idx(stringtable.insert(source));
    }

    if let Some(timestamp) = header_block.osmosis_replication_timestamp {
        header.set_replication_timestamp(timestamp);
    }

    if let Some(number) = header_block.osmosis_replication_sequence_number {
        header.set_replication_sequence_number(number);
    }

    if let Some(ref url) = header_block.osmosis_replication_base_url {
        header.set_replication_base_url_idx(stringtable.insert(url));
    }

    builder.set_header(&header)?;
    Ok(())
}

#[derive(PartialEq, Eq, Copy, Clone)]
struct I40 {
    x: [u8; 5],
}

impl I40 {
    fn from_u64(x: u64) -> Self {
        let x = x.to_le_bytes();
        debug_assert_eq!((x[5], x[6], x[7]), (0, 0, 0));
        Self {
            x: [x[0], x[1], x[2], x[3], x[4]],
        }
    }

    fn to_u64(self) -> u64 {
        let extented = [
            self.x[0], self.x[1], self.x[2], self.x[3], self.x[4], 0, 0, 0,
        ];
        u64::from_le_bytes(extented)
    }
}

#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for I40 {
    fn hash<H>(&self, h: &mut H)
    where
        H: std::hash::Hasher,
    {
        // We manually implement Hash like this, since [u8; 5] is slower to hash
        // than u64 for some/many hash functions
        self.to_u64().hash(h)
    }
}

/// Holds tags external vector and deduplicates tags.
struct TagSerializer<'a> {
    tags: flatdata::ExternalVector<'a, osmflat::Tag>,
    tags_index: flatdata::ExternalVector<'a, osmflat::TagIndex>,
    dedup: AHashMap<(I40, I40), I40>, // deduplication table: (key_idx, val_idx) -> pos
}

impl<'a> TagSerializer<'a> {
    fn new(builder: &'a osmflat::OsmBuilder) -> io::Result<Self> {
        Ok(Self {
            tags: builder.start_tags()?,
            tags_index: builder.start_tags_index()?,
            dedup: AHashMap::new(),
        })
    }

    fn serialize(&mut self, key_idx: u64, val_idx: u64) -> Result<(), Error> {
        let idx = match self
            .dedup
            .entry((I40::from_u64(key_idx), I40::from_u64(val_idx)))
        {
            hash_map::Entry::Occupied(entry) => entry.get().to_u64(),
            hash_map::Entry::Vacant(entry) => {
                let idx = self.tags.len() as u64;
                let tag = self.tags.grow()?;
                tag.set_key_idx(key_idx);
                tag.set_value_idx(val_idx);
                entry.insert(I40::from_u64(idx));
                idx
            }
        };

        let tag_index = self.tags_index.grow()?;
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
    let mut result = Vec::with_capacity(pbf_stringtable.s.len());
    for x in &pbf_stringtable.s {
        let string = str::from_utf8(x)?;
        result.push(stringtable.insert(string));
    }
    Ok(result)
}

fn serialize_dense_nodes(
    block: &osmpbf::PrimitiveBlock,
    granularity: i32,
    nodes: &mut flatdata::ExternalVector<osmflat::Node>,
    node_ids: &mut Option<flatdata::ExternalVector<osmflat::Id>>,
    nodes_id_to_idx: &mut ids::IdTableBuilder,
    stringtable: &mut StringTable,
    tags: &mut TagSerializer,
) -> Result<Stats, Error> {
    let mut stats = Stats::default();
    let string_refs = add_string_table(&block.stringtable, stringtable)?;
    for group in block.primitivegroup.iter() {
        let dense_nodes = group.dense.as_ref().unwrap();

        let pbf_granularity = block.granularity.unwrap_or(100);
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

            let node = nodes.grow()?;
            if let Some(ids) = node_ids {
                ids.grow()?.set_value(id as u64);
            }

            lat += dense_nodes.lat[i];
            lon += dense_nodes.lon[i];
            node.set_lat(
                ((lat_offset + (i64::from(pbf_granularity) * lat)) / granularity as i64) as i32,
            );
            node.set_lon(
                ((lon_offset + (i64::from(pbf_granularity) * lon)) / granularity as i64) as i32,
            );

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

fn resolve_ways(
    block: &osmpbf::PrimitiveBlock,
    nodes_id_to_idx: &ids::IdTable,
) -> (Vec<Option<u64>>, Stats) {
    let mut result = Vec::new();
    let mut stats = Stats::default();
    for group in &block.primitivegroup {
        for pbf_way in &group.ways {
            let mut node_ref = 0;
            for delta in &pbf_way.refs {
                node_ref += delta;
                let idx = nodes_id_to_idx.get(node_ref as u64);
                stats.num_unresolved_node_ids += idx.is_none() as usize;

                result.push(idx);
            }
        }
    }
    (result, stats)
}

#[allow(clippy::too_many_arguments)]
fn serialize_ways(
    block: &osmpbf::PrimitiveBlock,
    nodes_id_to_idx: &[Option<u64>],
    ways: &mut flatdata::ExternalVector<osmflat::Way>,
    way_ids: &mut Option<flatdata::ExternalVector<osmflat::Id>>,
    ways_id_to_idx: &mut ids::IdTableBuilder,
    stringtable: &mut StringTable,
    tags: &mut TagSerializer,
    nodes_index: &mut flatdata::ExternalVector<osmflat::NodeIndex>,
) -> Result<Stats, Error> {
    let mut stats = Stats::default();
    let string_refs = add_string_table(&block.stringtable, stringtable)?;
    let mut nodes_idx = nodes_id_to_idx.iter().cloned();
    for group in &block.primitivegroup {
        for pbf_way in &group.ways {
            let index = ways_id_to_idx.insert(pbf_way.id as u64);
            assert_eq!(index as usize, ways.len());

            let way = ways.grow()?;
            if let Some(ids) = way_ids {
                ids.grow()?.set_value(pbf_way.id as u64);
            }

            debug_assert_eq!(pbf_way.keys.len(), pbf_way.vals.len(), "invalid input data");
            way.set_tag_first_idx(tags.next_index());

            for i in 0..pbf_way.keys.len() {
                tags.serialize(
                    string_refs[pbf_way.keys[i] as usize],
                    string_refs[pbf_way.vals[i] as usize],
                )?;
            }

            way.set_ref_first_idx(nodes_index.len() as u64);
            for _ in &pbf_way.refs {
                nodes_index.grow()?.set_value(nodes_idx.next().unwrap());
            }
        }
        stats.num_ways += group.ways.len();
    }
    Ok(stats)
}

fn build_relations_index<I>(data: &[u8], block_index: I) -> Result<ids::IdTable, Error>
where
    I: ExactSizeIterator<Item = BlockIndex> + Send + 'static,
{
    let mut result = ids::IdTableBuilder::new();
    let mut pb = ProgressBar::new(block_index.len() as u64);
    pb.message("Building relations index...");
    parallel::parallel_process(
        block_index,
        |idx| read_block(data, &idx),
        |block: Result<osmpbf::PrimitiveBlock, _>| -> Result<(), Error> {
            for group in &block?.primitivegroup {
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

#[allow(clippy::too_many_arguments)]
fn serialize_relations(
    block: &osmpbf::PrimitiveBlock,
    nodes_id_to_idx: &ids::IdTable,
    ways_id_to_idx: &ids::IdTable,
    relations_id_to_idx: &ids::IdTable,
    stringtable: &mut StringTable,
    relations: &mut flatdata::ExternalVector<osmflat::Relation>,
    relation_ids: &mut Option<flatdata::ExternalVector<osmflat::Id>>,
    relation_members: &mut flatdata::MultiVector<osmflat::RelationMembers>,
    tags: &mut TagSerializer,
) -> Result<Stats, Error> {
    let mut stats = Stats::default();
    let string_refs = add_string_table(&block.stringtable, stringtable)?;
    for group in &block.primitivegroup {
        for pbf_relation in &group.relations {
            let relation = relations.grow()?;
            if let Some(ids) = relation_ids {
                ids.grow()?.set_value(pbf_relation.id as u64);
            }

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

            debug_assert!(
                pbf_relation.roles_sid.len() == pbf_relation.memids.len()
                    && pbf_relation.memids.len() == pbf_relation.types.len(),
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
                        let idx = nodes_id_to_idx.get(memid as u64);
                        stats.num_unresolved_node_ids = idx.is_none() as usize;

                        let member = members.add_node_member();
                        member.set_node_idx(idx);
                        member.set_role_idx(string_refs[pbf_relation.roles_sid[i] as usize]);
                    }
                    osmpbf::relation::MemberType::Way => {
                        let idx = ways_id_to_idx.get(memid as u64);
                        stats.num_unresolved_way_ids = idx.is_none() as usize;

                        let member = members.add_way_member();
                        member.set_way_idx(idx);
                        member.set_role_idx(string_refs[pbf_relation.roles_sid[i] as usize]);
                    }
                    osmpbf::relation::MemberType::Relation => {
                        let idx = relations_id_to_idx.get(memid as u64);
                        stats.num_unresolved_rel_ids = idx.is_none() as usize;

                        let member = members.add_relation_member();
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

#[allow(clippy::too_many_arguments)]
fn serialize_dense_node_blocks(
    builder: &osmflat::OsmBuilder,
    granularity: i32,
    mut node_ids: Option<flatdata::ExternalVector<osmflat::Id>>,
    blocks: Vec<BlockIndex>,
    data: &[u8],
    tags: &mut TagSerializer,
    stringtable: &mut StringTable,
    stats: &mut Stats,
) -> Result<ids::IdTable, Error> {
    let mut nodes_id_to_idx = ids::IdTableBuilder::new();
    let mut nodes = builder.start_nodes()?;
    let mut pb = ProgressBar::new(blocks.len() as u64);
    pb.message("Converting dense nodes...");

    parallel::parallel_process(
        blocks.into_iter(),
        |idx| read_block(data, &idx),
        |block| -> Result<osmpbf::PrimitiveBlock, Error> {
            let block = block?;
            *stats += serialize_dense_nodes(
                &block,
                granularity,
                &mut nodes,
                &mut node_ids,
                &mut nodes_id_to_idx,
                stringtable,
                tags,
            )?;

            pb.inc();
            Ok(block)
        },
    )?;

    // fill tag_first_idx of the sentry, since it contains the end of the tag range
    // of the last node
    nodes.grow()?.set_tag_first_idx(tags.next_index());
    nodes.close()?;
    if let Some(ids) = node_ids {
        ids.close()?;
    }
    info!("Dense nodes converted.");
    info!("Building dense nodes index...");
    let nodes_id_to_idx = nodes_id_to_idx.build();
    info!("Dense nodes index built.");
    Ok(nodes_id_to_idx)
}

type PrimitiveBlockWithIds = (osmpbf::PrimitiveBlock, (Vec<Option<u64>>, Stats));

#[allow(clippy::too_many_arguments)]
fn serialize_way_blocks(
    builder: &osmflat::OsmBuilder,
    mut way_ids: Option<flatdata::ExternalVector<osmflat::Id>>,
    blocks: Vec<BlockIndex>,
    data: &[u8],
    nodes_id_to_idx: &ids::IdTable,
    tags: &mut TagSerializer,
    stringtable: &mut StringTable,
    stats: &mut Stats,
) -> Result<ids::IdTable, Error> {
    let mut ways_id_to_idx = ids::IdTableBuilder::new();
    let mut ways = builder.start_ways()?;
    let mut pb = ProgressBar::new(blocks.len() as u64);
    let mut nodes_index = builder.start_nodes_index()?;
    pb.message("Converting ways...");
    parallel::parallel_process(
        blocks.into_iter(),
        |idx| {
            let block: osmpbf::PrimitiveBlock = read_block(data, &idx)?;
            let ids = resolve_ways(&block, nodes_id_to_idx);
            Ok((block, ids))
        },
        |block: io::Result<PrimitiveBlockWithIds>| -> Result<osmpbf::PrimitiveBlock, Error> {
            let (block, (ids, stats_resolve)) = block?;
            *stats += stats_resolve;
            *stats += serialize_ways(
                &block,
                &ids,
                &mut ways,
                &mut way_ids,
                &mut ways_id_to_idx,
                stringtable,
                tags,
                &mut nodes_index,
            )?;
            pb.inc();

            Ok(block)
        },
    )?;

    {
        let sentinel = ways.grow()?;
        sentinel.set_tag_first_idx(tags.next_index());
        sentinel.set_ref_first_idx(nodes_index.len() as u64);
    }
    ways.close()?;
    if let Some(ids) = way_ids {
        ids.close()?;
    }
    nodes_index.close()?;

    info!("Ways converted.");
    info!("Building ways index...");
    let ways_id_to_idx = ways_id_to_idx.build();
    info!("Way index built.");
    Ok(ways_id_to_idx)
}

#[allow(clippy::too_many_arguments)]
fn serialize_relation_blocks(
    builder: &osmflat::OsmBuilder,
    mut relation_ids: Option<flatdata::ExternalVector<osmflat::Id>>,
    blocks: Vec<BlockIndex>,
    data: &[u8],
    nodes_id_to_idx: &ids::IdTable,
    ways_id_to_idx: &ids::IdTable,
    tags: &mut TagSerializer,
    stringtable: &mut StringTable,
    stats: &mut Stats,
) -> Result<(), Error> {
    // We need to build the index of relation ids first, since relations can refer
    // again to relations.
    let relations_id_to_idx = build_relations_index(data, blocks.clone().into_iter())?;

    let mut relations = builder.start_relations()?;
    let mut relation_members = builder.start_relation_members()?;

    let mut pb = ProgressBar::new(blocks.len() as u64);
    pb.message("Converting relations...");
    parallel::parallel_process(
        blocks.into_iter(),
        |idx| read_block(data, &idx),
        |block| -> Result<osmpbf::PrimitiveBlock, Error> {
            let block = block?;
            *stats += serialize_relations(
                &block,
                nodes_id_to_idx,
                ways_id_to_idx,
                &relations_id_to_idx,
                stringtable,
                &mut relations,
                &mut relation_ids,
                &mut relation_members,
                tags,
            )?;
            pb.inc();
            Ok(block)
        },
    )?;

    {
        let sentinel = relations.grow()?;
        sentinel.set_tag_first_idx(tags.next_index());
    }

    relations.close()?;
    if let Some(ids) = relation_ids {
        ids.close()?;
    }
    relation_members.close()?;

    info!("Relations converted.");

    Ok(())
}

fn gcd(a: i32, b: i32) -> i32 {
    let (mut x, mut y) = (a.min(b), a.max(b));
    while x > 1 {
        y %= x;
        std::mem::swap(&mut x, &mut y);
    }
    y
}

fn run(args: args::Args) -> Result<(), Error> {
    let input_file = File::open(&args.input)?;
    let input_data = unsafe { Mmap::map(&input_file)? };

    let storage = FileResourceStorage::new(args.output.clone());
    let builder = osmflat::OsmBuilder::new(storage.clone())?;

    // TODO: Would be nice not store all these strings in memory, but to flush them
    // from time to time to disk.
    let mut stringtable = StringTable::new();
    let mut tags = TagSerializer::new(&builder)?;

    info!(
        "Initialized new osmflat archive at: {}",
        &args.output.display()
    );

    info!("Building index of PBF blocks...");
    let block_index = build_block_index(&input_data);
    let mut greatest_common_granularity = 1000000000;
    for block in &block_index {
        if block.block_type == BlockType::DenseNodes {
            // only DenseNodes have coordinate we need to scale
            if let Some(block_granularity) = block.granularity {
                greatest_common_granularity =
                    gcd(greatest_common_granularity, block_granularity as i32);
            }
        }
    }
    let coord_scale = 1000000000 / greatest_common_granularity;
    info!(
        "Greatest common granularity: {}, Coordinate scaling factor: {}",
        greatest_common_granularity, coord_scale
    );

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

    // Serialize header
    if pbf_header.len() != 1 {
        return Err(format!(
            "Require exactly one header block, but found {}",
            pbf_header.len()
        )
        .into());
    }
    let idx = &pbf_header[0];
    let pbf_header: osmpbf::HeaderBlock = read_block(&input_data, idx)?;
    serialize_header(&pbf_header, coord_scale, &builder, &mut stringtable)?;
    info!("Header written.");

    let mut stats = Stats::default();

    let ids_archive;
    let mut node_ids = None;
    let mut way_ids = None;
    let mut relation_ids = None;
    if args.ids {
        ids_archive = builder.ids()?;
        node_ids = Some(ids_archive.start_nodes()?);
        way_ids = Some(ids_archive.start_ways()?);
        relation_ids = Some(ids_archive.start_relations()?);
    }

    let nodes_id_to_idx = serialize_dense_node_blocks(
        &builder,
        greatest_common_granularity,
        node_ids,
        pbf_dense_nodes,
        &input_data,
        &mut tags,
        &mut stringtable,
        &mut stats,
    )?;

    let ways_id_to_idx = serialize_way_blocks(
        &builder,
        way_ids,
        pbf_ways,
        &input_data,
        &nodes_id_to_idx,
        &mut tags,
        &mut stringtable,
        &mut stats,
    )?;

    serialize_relation_blocks(
        &builder,
        relation_ids,
        pbf_relations,
        &input_data,
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

    info!("osmflat archive built.");

    std::mem::drop(builder);
    osmflat::Osm::open(storage)?;

    info!("verified that osmflat archive can be opened.");

    println!("{stats}");
    Ok(())
}

fn main() {
    let args = args::Args::parse();
    let level = match args.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level))
        .format_module_path(false)
        .format_timestamp_nanos()
        .init();

    if let Err(e) = run(args) {
        eprintln!("{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
}
