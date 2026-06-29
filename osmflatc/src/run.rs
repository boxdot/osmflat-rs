//! End-to-end pbf -> flatdata conversion driver.
//!
//! This is the work that the `osmflatc` binary performs; it lives in the
//! library so the binary stays a thin shim and so the internal stages remain
//! `pub(crate)` rather than being forced public.

use crate::args::Args;
use crate::osmpbf::{self, build_block_index, read_block, BlockType};
use crate::processing::{
    create_db, node::serialize_dense_node_blocks, relation::serialize_relation_blocks,
    way::serialize_way_blocks,
};
use crate::stats::{MissingRefs, Stats};
use crate::strings::StringTable;
use crate::{Error, TagSerializer};

use flatdata::FileResourceStorage;
use itertools::Itertools;
use log::info;
use memmap2::Mmap;

use std::fs::File;
use std::io;
use std::path::Path;

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

fn gcd(a: i32, b: i32) -> i32 {
    let (mut x, mut y) = (a.min(b), a.max(b));
    while x > 1 {
        y %= x;
        std::mem::swap(&mut x, &mut y);
    }
    y
}

/// Run the full pbf -> flatdata conversion described by `args`.
pub fn run(args: Args) -> Result<(), Error> {
    let input_file = File::open(&args.input)?;
    let input_data = unsafe { Mmap::map(&input_file)? };

    // Each conversion phase scans the pbf front-to-back (blocks are read in file
    // order), so hint the kernel for sequential access: more aggressive
    // readahead plus drop-behind of already-read pages, which improves
    // throughput and keeps the page cache from competing with the RocksDB block
    // cache and memtables. Best-effort -- a failure (e.g. unsupported platform)
    // is not fatal.
    if let Err(e) = input_data.advise(memmap2::Advice::Sequential) {
        log::warn!("madvise(MADV_SEQUENTIAL) on input failed, continuing: {e}");
    }

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
    let groups = block_index.into_iter().chunk_by(|b| b.block_type);
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

    // Keep `_scratch` alive for the whole conversion; dropping it removes the
    // temporary RocksDB directory. `db` (declared here) is dropped before
    // `_scratch`, closing the database before its files are deleted.
    //
    // The scratch DB is I/O-heavy and huge for a planet, so honor an explicit
    // `--scratch-dir` (point it at a fast SSD) and otherwise fall back to the
    // output's parent directory.
    let scratch_parent = args
        .scratch_dir
        .as_deref()
        .unwrap_or_else(|| args.output.parent().unwrap_or_else(|| Path::new(".")));

    // RocksDB keeps an fd open per cached SST file; a large ingest produces
    // thousands of SSTs across the scratch DB's column families. The cap is set
    // via `--max-open-files` (-1 = unlimited) and must stay below the process
    // open-file limit, which the caller raises with `ulimit -n` as needed.
    info!("RocksDB max_open_files: {}", args.max_open_files);

    let (db, _scratch) = create_db(
        scratch_parent,
        args.block_cache_mb * 1024 * 1024,
        args.write_buffer_mb * 1024 * 1024,
        args.max_open_files,
    )?;

    let mut stats = Stats::default();
    let mut missing = MissingRefs::default();

    // The reverse id index indirects through the positional id vectors, so it
    // requires the forward `--ids` data; `--reverse-ids` therefore implies it.
    let want_ids = args.ids || args.reverse_ids;

    let ids_archive;
    let mut node_ids = None;
    let mut way_ids = None;
    let mut relation_ids = None;
    let mut node_by_id = None;
    let mut way_by_id = None;
    let mut relation_by_id = None;
    if want_ids {
        ids_archive = builder.ids()?;
        node_ids = Some(ids_archive.start_nodes()?);
        way_ids = Some(ids_archive.start_ways()?);
        relation_ids = Some(ids_archive.start_relations()?);
        if args.reverse_ids {
            node_by_id = Some(ids_archive.start_nodes_by_id()?);
            way_by_id = Some(ids_archive.start_ways_by_id()?);
            relation_by_id = Some(ids_archive.start_relations_by_id()?);
        }
    }

    serialize_dense_node_blocks(
        &builder,
        greatest_common_granularity,
        node_ids,
        node_by_id,
        &db,
        pbf_dense_nodes,
        &input_data,
        &mut tags,
        &mut stringtable,
        &mut stats,
        coord_scale,
    )?;

    serialize_way_blocks(
        &builder,
        &db,
        way_ids,
        way_by_id,
        pbf_ways,
        &input_data,
        &mut tags,
        &mut stringtable,
        &mut stats,
        &mut missing,
        coord_scale,
    )?;

    serialize_relation_blocks(
        &builder,
        &db,
        relation_ids,
        relation_by_id,
        pbf_relations,
        &input_data,
        &mut tags,
        &mut stringtable,
        &mut stats,
        &mut missing,
        coord_scale,
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
    println!("{missing}");
    Ok(())
}
