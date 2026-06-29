use super::{write_batch_no_wal, WriteBatchInternal};
use crate::error::OsmFlatcError;
use crate::{
    add_string_table,
    osmpbf::{self, read_block, BlockIndex},
    pb_style,
    processing::{
        node::storage::{NodeIdToIdxTDC, NodeIdToLonLatTDC},
        storage::{OsmIdKey, OsmIdxValue, OsmKey},
        RocksDB, RocksDBSync, RocksDBUnsync, TempDataCodec,
    },
    stats::{MissingRefs, Stats},
    strings::StringTable,
    Error, TagSerializer, BATCH_SIZE,
};
use geo::{BoundingRect, MultiPoint};
use indicatif::ProgressBar;
use log::info;
use parking_lot::Mutex;
use rayon::prelude::*;
use rocksdb::DB;
use storage::{WayIdToIdxTDC, WayIdToMbbTDC, WayMbbValue, WayTDC, WayValue};

pub(crate) mod storage;

/// Number of ways resolved per parallel batch in the ordering pass. Large
/// enough to keep the Rayon pool busy and amortize the sequential write of each
/// chunk; small enough that the buffered ways and their resolved indices stay a
/// modest fraction of memory.
const ORDER_CHUNK: usize = 100_000;

fn serialize_ways(
    block: &osmpbf::PrimitiveBlock,
    batch: &mut impl RocksDBUnsync,
    db: &DB,
    stringtable: &Mutex<StringTable>,
    coord_scale: i32,
) -> Result<Stats, OsmFlatcError> {
    let mut stats = Stats::default();

    let curve = osmflat::way_curve();

    // Shared across the worker threads; hold the lock only for the per-block
    // string interning, not the per-way serialization below.
    let string_refs = {
        let mut guard = stringtable.lock();
        add_string_table(&block.stringtable, &mut guard)?
    };

    for group in &block.primitivegroup {
        for pbf_way in &group.ways {
            debug_assert_eq!(pbf_way.keys.len(), pbf_way.vals.len(), "invalid input data");

            let mut key_vals = vec![];

            for i in 0..pbf_way.keys.len() {
                key_vals.push((
                    string_refs[pbf_way.keys[i] as usize],
                    string_refs[pbf_way.vals[i] as usize],
                ));
            }

            let mut node_locations = vec![];
            let mut node_refs = vec![];
            let mut ref_id = 0;
            for r in &pbf_way.refs {
                ref_id += r;

                node_refs.push(ref_id);

                let node_id = OsmIdKey::new(ref_id);

                if let Some(n) = <DB as RocksDBSync>::get::<NodeIdToLonLatTDC>(db, &node_id)? {
                    node_locations.push((
                        n.lon as f64 / coord_scale as f64,
                        n.lat as f64 / coord_scale as f64,
                    ));
                }
            }

            let points: MultiPoint<_> = node_locations.into();

            let mbr = points.bounding_rect();
            if let Some(br) = mbr {
                let min_x = br.min().x;
                let min_y = br.min().y;
                let max_x = br.max().x;
                let max_y = br.max().y;

                let spatial_index = osmflat::bbox_index(&curve, min_x, min_y, max_x, max_y);

                let key = OsmKey::new(spatial_index, pbf_way.id);
                let value = WayValue::new(node_refs, key_vals);

                batch.put::<WayTDC>(key, value);

                let id_key = OsmIdKey::new(pbf_way.id);

                batch.put::<WayIdToMbbTDC>(
                    id_key,
                    WayMbbValue::new(
                        [min_x, min_y, max_x, max_y]
                            .into_iter()
                            .map(|v| (v * coord_scale as f64) as i32)
                            .collect(),
                    ),
                );
            }
        }
        stats.num_ways += group.ways.len();
    }

    Ok(stats)
}

#[allow(clippy::too_many_arguments)]
pub fn serialize_way_blocks(
    builder: &osmflat::OsmBuilder,
    db: &DB,
    mut way_ids: Option<flatdata::ExternalVector<osmflat::Id>>,
    way_by_id: Option<flatdata::ExternalVector<osmflat::IdxRef>>,
    blocks: Vec<BlockIndex>,
    data: &[u8],
    tags: &mut TagSerializer,
    stringtable: &mut StringTable,
    stats: &mut Stats,
    missing: &mut MissingRefs,
    coord_scale: i32,
) -> Result<(), Error> {
    let mut ways = builder.start_ways()?;
    let pb = ProgressBar::new(blocks.len() as u64)
        .with_style(pb_style())
        .with_prefix("Converting ways");
    let mut nodes_index = builder.start_nodes_index()?;

    // The per-member RocksDB node-location lookups dominate this pass, so fan it
    // out across all Rayon workers rather than running on a single consumer. The
    // string table is shared behind a Mutex (locked once per block) and the
    // per-block `stats` are commutative, merged with `try_reduce`. Order is
    // irrelevant: ways are emitted later by iterating RocksDB in sorted key
    // order. `std::mem::take` moves the caller's table in for the duration and
    // it is restored below.
    let string_table = Mutex::new(std::mem::take(stringtable));
    let total = blocks
        .into_par_iter()
        .map(|idx| -> Result<Stats, OsmFlatcError> {
            let block: osmpbf::PrimitiveBlock = read_block(data, &idx)?;

            let mut batch = WriteBatchInternal::default();
            for cf_name in [WayTDC::NAME, WayIdToMbbTDC::NAME] {
                let cf = db.cf_handle(cf_name).unwrap();
                batch.insert_cf(cf_name, cf);
            }

            let block_stats = serialize_ways(&block, &mut batch, db, &string_table, coord_scale)?;

            write_batch_no_wal(db, batch.inner())?;

            pb.inc(1);

            Ok(block_stats)
        })
        .try_reduce(Stats::default, |mut a, b| {
            a += b;
            Ok(a)
        })?;
    *stats += total;
    *stringtable = string_table.into_inner();

    pb.finish();
    info!("Ways converted.");

    let mut batch = WriteBatchInternal::default();
    let cf = db.cf_handle(WayIdToIdxTDC::NAME).unwrap();
    batch.insert_cf(WayIdToIdxTDC::NAME, cf);

    let pb = ProgressBar::new(stats.num_ways as u64)
        .with_style(pb_style())
        .with_prefix("Ordering ways by spatial index order");

    // The per-ref `NodeIdToIdx` lookups are random reads against the big node CF
    // and dominate this pass, but the flatdata writes (`ways`, `nodes_index`,
    // tag index) must stay in spatial-iteration order. So pull a chunk of ways
    // out of the (sequential) RocksDB iterator, resolve every chunk's node refs
    // to their final indices in parallel -- `par_iter().collect()` preserves
    // order -- then write the resolved chunk sequentially. The random reads fan
    // out across all Rayon workers; only the cheap, ordered appends run serially.
    let mut base: usize = 0;
    let mut iter = <DB as RocksDB>::iterator::<WayTDC>(db)?;
    loop {
        let mut chunk: Vec<(i64, WayValue)> = Vec::with_capacity(ORDER_CHUNK);
        for r in iter.by_ref().take(ORDER_CHUNK) {
            let (k, v) = r?;
            chunk.push((k.id, v));
        }
        if chunk.is_empty() {
            break;
        }

        // Parallel: resolve each way's node-id refs to final indices.
        let resolved: Vec<Vec<Option<u64>>> = chunk
            .par_iter()
            .map(|(_, v)| -> Result<Vec<Option<u64>>, OsmFlatcError> {
                v.node_refs
                    .iter()
                    .map(|&n| {
                        Ok(
                            <DB as RocksDBSync>::get::<NodeIdToIdxTDC>(db, &OsmIdKey::new(n))?
                                .map(|v| v.idx),
                        )
                    })
                    .collect()
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Sequential: write the chunk in spatial-iteration order.
        for (j, ((way_id, v), resolved_refs)) in chunk.iter().zip(resolved).enumerate() {
            let way = ways.grow()?;

            let tag_first_idx = tags.next_index();
            for &(k, v) in &v.key_vals {
                tags.serialize(k, v)?;
            }

            way.set_tag_first_idx(tag_first_idx);
            way.set_ref_first_idx(nodes_index.len() as u64);

            batch.put::<WayIdToIdxTDC>(OsmIdKey::new(*way_id), OsmIdxValue::new((base + j) as u64));

            for (&n, resolved_idx) in v.node_refs.iter().zip(resolved_refs) {
                if resolved_idx.is_none() {
                    // A node referenced by a way but absent from the archive.
                    missing.nodes_in_ways.insert(n);
                }
                nodes_index.grow()?.set_value(resolved_idx);
            }

            if let Some(ids) = &mut way_ids {
                ids.grow()?.set_value(*way_id as u64);
            }

            pb.inc(1);

            if j % BATCH_SIZE == 0 {
                write_batch_no_wal(db, batch.inner())?;
                batch = WriteBatchInternal::default();
                let cf = db.cf_handle(WayIdToIdxTDC::NAME).unwrap();
                batch.insert_cf(WayIdToIdxTDC::NAME, cf);
            }
        }

        base += chunk.len();
    }

    write_batch_no_wal(db, batch.inner())?;

    pb.finish();

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

    // Reverse index: `WayIdToIdx` is keyed by OSM id (big-endian), so iterating
    // it yields `(id, final_idx)` in ascending-id order. Emitting just the index
    // gives a permutation `p` with `ids.ways[p[k]]` ascending by id, which the
    // query side binary-searches. See the node path for the rationale.
    if let Some(mut by_id) = way_by_id {
        for r in <DB as RocksDB>::iterator::<WayIdToIdxTDC>(db)? {
            let (_id, idx) = r?;
            by_id.grow()?.set_value(idx.idx);
        }
        by_id.close()?;
    }

    info!("Ways processed");
    Ok(())
}
