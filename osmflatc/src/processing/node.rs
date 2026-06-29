use crate::error::OsmFlatcError;
use crate::processing::storage::OsmIdKey;
use crate::processing::storage::OsmIdxValue;
use crate::processing::storage::OsmKey;
use crate::processing::{
    write_batch_no_wal, RocksDB, RocksDBUnsync, TempDataCodec, WriteBatchInternal,
};
use crate::{
    add_string_table,
    osmpbf::{self, read_block, BlockIndex},
    pb_style,
    stats::Stats,
    strings::StringTable,
    Error, TagSerializer, BATCH_SIZE,
};
use indicatif::ProgressBar;
use log::info;
use parking_lot::Mutex;
use rayon::prelude::*;
use rocksdb::DB;
use storage::{NodeIdToIdxTDC, NodeIdToLonLatTDC, NodeLonLatValue, NodeValue, NodesTDC};

pub(crate) mod storage;

/// Serialize a single dense-node `PrimitiveBlock` into the temporary RocksDB
/// `batch`, computing each node's spatial-curve key. Exposed (rather than
/// private) so benchmarks can drive it with a [`mock`](crate::processing::mock)
/// batch and synthetic blocks.
pub fn serialize_dense_nodes_primative_block(
    block: &osmpbf::PrimitiveBlock,
    granularity: i32,
    batch: &mut impl RocksDBUnsync,
    string_table: &Mutex<StringTable>,
    coord_scale: i32,
) -> Result<Stats, OsmFlatcError> {
    let mut stats = Stats::default();
    // The global string table is shared across the worker threads. Hold the
    // lock only for this per-block insertion (not the per-node loop below), so
    // the expensive serialization parallelizes while string interning stays
    // consistent.
    let string_refs = {
        let mut guard = string_table.lock();
        add_string_table(&block.stringtable, &mut guard)?
    };

    let curve = osmflat::node_curve();

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

            lat += dense_nodes.lat[i];
            lon += dense_nodes.lon[i];
            let lat_ =
                ((lat_offset + (i64::from(pbf_granularity) * lat)) / granularity as i64) as i32;
            let lon_ =
                ((lon_offset + (i64::from(pbf_granularity) * lon)) / granularity as i64) as i32;

            let mut key_refs = vec![];

            if tags_offset < dense_nodes.keys_vals.len() {
                loop {
                    let k = dense_nodes.keys_vals[tags_offset];
                    tags_offset += 1;

                    if k == 0 {
                        break; // separator
                    }

                    let v = dense_nodes.keys_vals[tags_offset];
                    tags_offset += 1;

                    key_refs.push((string_refs[k as usize], string_refs[v as usize]));
                }
            }

            let spatial_index = osmflat::node_index(
                &curve,
                lon_ as f64 / coord_scale as f64,
                lat_ as f64 / coord_scale as f64,
            );
            let key = OsmKey::new(spatial_index, id);
            let value = NodeValue::new(lon_, lat_, key_refs);

            batch.put::<NodesTDC>(key, value);

            let key2 = OsmIdKey::new(id);
            let value2 = NodeLonLatValue::new(lon_, lat_);
            batch.put::<NodeIdToLonLatTDC>(key2, value2);
        }
        assert_eq!(tags_offset, dense_nodes.keys_vals.len());
        stats.num_nodes += dense_nodes.id.len();
    }

    Ok(stats)
}

#[allow(clippy::too_many_arguments)]
pub fn serialize_dense_node_blocks(
    builder: &osmflat::OsmBuilder,
    granularity: i32,
    mut node_ids: Option<flatdata::ExternalVector<osmflat::Id>>,
    node_by_id: Option<flatdata::ExternalVector<osmflat::IdxRef>>,
    db: &DB,
    blocks: Vec<BlockIndex>,
    data: &[u8],
    tags: &mut TagSerializer,
    stringtable: &mut StringTable,
    stats: &mut Stats,
    coord_scale: i32,
) -> Result<(), Error> {
    let mut nodes = builder.start_nodes()?;
    let pb = ProgressBar::new(blocks.len() as u64)
        .with_style(pb_style())
        .with_prefix("Converting dense nodes");

    // Serialization (spatial-curve indexing + value encoding + the RocksDB
    // writes) dominates this pass, so fan it out across all Rayon workers. The
    // only cross-block shared state is the string table -- behind a Mutex locked
    // once per block -- and per-block `stats`, which are commutative and merged
    // with `try_reduce`. Block order is irrelevant: the spatial ordering is
    // recovered later by iterating RocksDB in sorted key order. `std::mem::take`
    // moves the caller's table in for the duration and it is restored below.
    let string_table = Mutex::new(std::mem::take(stringtable));
    let total = blocks
        .into_par_iter()
        .map(|idx| -> Result<Stats, OsmFlatcError> {
            let block: osmpbf::PrimitiveBlock = read_block(data, &idx)?;
            let mut batch = WriteBatchInternal::default();

            for cf in [NodesTDC::NAME, NodeIdToLonLatTDC::NAME] {
                if let Some(cf_handle) = db.cf_handle(cf) {
                    batch.insert_cf(cf, cf_handle);
                }
            }

            let block_stats = serialize_dense_nodes_primative_block(
                &block,
                granularity,
                &mut batch,
                &string_table,
                coord_scale,
            )?;
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

    let pb = ProgressBar::new(stats.num_nodes as u64)
        .with_style(pb_style())
        .with_prefix("Ordering dense nodes in spatial index order");

    let mut batch = WriteBatchInternal::default();

    let cf = db.cf_handle(NodeIdToIdxTDC::NAME).unwrap();
    batch.insert_cf(NodeIdToIdxTDC::NAME, cf);

    for (i, r) in <DB as RocksDB>::iterator::<NodesTDC>(db)?.enumerate() {
        let (k, v) = r?;

        let idx = i as u64;

        let node_id = OsmIdKey::new(k.id);
        let node = nodes.grow()?;
        // Coordinates travel inline in the NodesTDC value, so the previous
        // per-node random `NodeIdToLonLat` lookup is gone -- this is now a pure
        // sequential scan.
        node.set_lon(v.lon);
        node.set_lat(v.lat);

        node.set_tag_first_idx(tags.next_index());

        if !v.refs.is_empty() {
            for &(key_ref, val_ref) in &v.refs {
                tags.serialize(key_ref, val_ref)?;
            }
        }

        if let Some(ids) = &mut node_ids {
            ids.grow()?.set_value(k.id as u64);
        }

        let node_idx = OsmIdxValue::new(idx);

        batch.put::<NodeIdToIdxTDC>(node_id, node_idx);
        pb.inc(1);

        if i % BATCH_SIZE == 0 {
            write_batch_no_wal(db, batch.inner())?;
            batch = WriteBatchInternal::default();
            let cf = db.cf_handle(NodeIdToIdxTDC::NAME).unwrap();
            batch.insert_cf(NodeIdToIdxTDC::NAME, cf);
        }
    }

    write_batch_no_wal(db, batch.inner())?;
    pb.finish();

    // fill tag_first_idx of the sentry, since it contains the end of the tag range
    // of the last node
    nodes.grow()?.set_tag_first_idx(tags.next_index());
    nodes.close()?;
    if let Some(ids) = node_ids {
        ids.close()?;
    }

    // Reverse index: `NodeIdToIdx` is keyed by OSM id (big-endian), so iterating
    // it yields `(id, final_idx)` in ascending-id order. Emitting just the
    // index gives a permutation `p` with `ids.nodes[p[k]]` ascending by id --
    // exactly what the query side binary-searches.
    if let Some(mut by_id) = node_by_id {
        for r in <DB as RocksDB>::iterator::<NodeIdToIdxTDC>(db)? {
            let (_id, idx) = r?;
            by_id.grow()?.set_value(idx.idx);
        }
        by_id.close()?;
    }

    info!("Dense nodes converted.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        osmpbf::{DenseNodes, PrimitiveBlock, PrimitiveGroup},
        processing::{mock::MockRocksBatch, node::storage::NodesTDC, storage::OsmKey, RocksDB},
        strings::StringTable,
    };
    use parking_lot::Mutex;

    use super::serialize_dense_nodes_primative_block;

    #[test]
    fn test_serialize_dense_nodes_primative_block() {
        let block = construct_node_block();
        assert_serialize_dense_nodes_primative_block(block)
    }

    fn assert_serialize_dense_nodes_primative_block(block: PrimitiveBlock) {
        let mut batch = MockRocksBatch::default();

        let stringtable = Mutex::new(StringTable::default());

        let stats =
            serialize_dense_nodes_primative_block(&block, 100, &mut batch, &stringtable, 1_000_000);

        assert!(stats.is_ok());

        let stats = stats.unwrap();

        let mut iter = batch.iterator::<NodesTDC>().unwrap();

        let mut prev_key: Option<OsmKey> = None;
        let mut keys = vec![];
        while let Some(Ok((k, v))) = iter.next() {
            assert!(v.refs.is_empty());

            keys.push(k);
            if let Some(prev) = &mut prev_key {
                assert!(k.spatial_index >= prev.spatial_index);
                *prev = k;
            } else {
                prev_key = Some(k);
            }
        }

        assert_eq!(keys.len(), 4);
        assert_eq!(stats.num_nodes, 4);
    }

    fn construct_node_block() -> PrimitiveBlock {
        let dense_nodes = DenseNodes {
            id: vec![32, 4, -6, 12],
            keys_vals: vec![],
            denseinfo: None,
            lat: vec![45_000_000, 33, -33, -90_000_000],
            lon: vec![97_000_000, -33, 33, -180_000_000],
        };

        let primitivegroup = PrimitiveGroup {
            dense: Some(dense_nodes),
            ..Default::default()
        };

        PrimitiveBlock {
            stringtable: crate::osmpbf::StringTable::default(),
            primitivegroup: vec![primitivegroup],
            ..Default::default()
        }
    }

    #[test]
    fn test_serialize_dense_nodes_with_tags() {
        let block = construct_node_block_with_tags();
        assert_serialize_dense_nodes_with_tags(block)
    }

    fn assert_serialize_dense_nodes_with_tags(block: PrimitiveBlock) {
        let mut batch = MockRocksBatch::default();
        let stringtable = Mutex::new(StringTable::default());

        let stats =
            serialize_dense_nodes_primative_block(&block, 100, &mut batch, &stringtable, 1_000_000)
                .unwrap();

        assert_eq!(stats.num_nodes, 3);

        let mut iter = batch.iterator::<NodesTDC>().unwrap();
        let mut nodes = vec![];
        while let Some(Ok((k, v))) = iter.next() {
            nodes.push((k, v));
        }
        assert_eq!(nodes.len(), 3);

        let mut node_map = std::collections::HashMap::new();
        for (k, v) in nodes {
            node_map.insert(k.id, v);
        }

        fn get_string(stringtable: &mut StringTable, s: &str) -> u64 {
            stringtable.insert(s)
        }

        // Rebuild the stringtable in the same order serialization used, so the
        // expected indices line up.
        let mut test_stringtable = StringTable::default();
        for s in &block.stringtable.s {
            test_stringtable.insert(&String::from_utf8(s.clone()).unwrap());
        }

        for id in [1, 2, 3] {
            let node_value = node_map.get(&id).expect("Node not found");

            if id == 1 {
                let key_refs = vec![
                    (
                        get_string(&mut test_stringtable, "amenity"),
                        get_string(&mut test_stringtable, "school"),
                    ),
                    (
                        get_string(&mut test_stringtable, "name"),
                        get_string(&mut test_stringtable, "ABC High School"),
                    ),
                ];
                assert_eq!(node_value.refs, key_refs);
            } else if id == 2 {
                let key_refs = vec![
                    (
                        get_string(&mut test_stringtable, "amenity"),
                        get_string(&mut test_stringtable, "hospital"),
                    ),
                    (
                        get_string(&mut test_stringtable, "name"),
                        get_string(&mut test_stringtable, "City Hospital"),
                    ),
                ];
                assert_eq!(node_value.refs, key_refs);
            } else if id == 3 {
                assert!(node_value.refs.is_empty());
            }
        }
    }

    fn construct_node_block_with_tags() -> PrimitiveBlock {
        // String table indices (index 0 is reserved for delimiter)
        // 1: "amenity", 2: "school", 3: "name", 4: "ABC High School"
        // 5: "hospital", 6: "City Hospital"
        let block_stringtable = crate::osmpbf::StringTable {
            s: vec![
                vec![],                      // index 0 (reserved)
                b"amenity".to_vec(),         // index 1
                b"school".to_vec(),          // index 2
                b"name".to_vec(),            // index 3
                b"ABC High School".to_vec(), // index 4
                b"hospital".to_vec(),        // index 5
                b"City Hospital".to_vec(),   // index 6
            ],
        };

        // keys_vals: [key_idx, val_idx, ..., 0] per node
        let keys_vals = vec![
            // Node 1 tags
            1, 2, 3, 4, 0, // Node 2 tags
            1, 5, 3, 6, 0, // Node 3 tags (no tags)
            0,
        ];

        // Delta-encoded IDs, lats, and lons
        let dense_nodes = DenseNodes {
            id: vec![1, 1, 1], // IDs: 1, 2, 3
            keys_vals,
            denseinfo: None,
            lat: vec![10_000_000, 10_000_000, 10_000_000],
            lon: vec![40_000_000, 10_000_000, 10_000_000],
        };

        let primitivegroup = PrimitiveGroup {
            dense: Some(dense_nodes),
            ..Default::default()
        };

        PrimitiveBlock {
            stringtable: block_stringtable,
            primitivegroup: vec![primitivegroup],
            granularity: Some(100),
            lat_offset: Some(0),
            lon_offset: Some(0),
            ..Default::default()
        }
    }
}
