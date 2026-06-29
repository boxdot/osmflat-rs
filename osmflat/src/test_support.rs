//! Helpers for building synthetic [`Osm`] archives in memory, laid out exactly
//! the way `osmflatc` writes them on disk (entities ordered by their
//! space-filling-curve key, every primary vector terminated by a sentinel).
//!
//! This module is compiled for unit tests and whenever the `test-support`
//! feature is enabled, so integration tests and benchmarks (which only see the
//! crate's public API) can build the same archives the in-crate tests use.

use flatdata::MemoryResourceStorage;

use crate::{
    bbox_index, node_curve, node_index, way_curve, Header, Osm, OsmBuilder, RELATION_NO_BBOX,
};

/// Coordinate scale used by the synthetic archives (matches osmium / osmflatc's
/// default 1e-7 degree precision).
pub const COORD_SCALE: i32 = 10_000_000;

/// Scale a degree value to the archive's fixed-point representation.
pub fn scale(v: f64) -> i32 {
    (v * COORD_SCALE as f64) as i32
}

/// Inverse of [`scale`].
pub fn unscale(v: i32) -> f64 {
    v as f64 / COORD_SCALE as f64
}

/// Build an in-memory archive laid out exactly the way `osmflatc` does:
/// nodes ordered by the z-order curve, ways and relations ordered by the
/// `XZ2SFC` index of their bounding box, each vector terminated by a
/// sentinel. This lets the query functions binary-search correctly.
///
/// - `node_lonlat`: `(lon, lat)` per node, in degrees.
/// - `ways`: each way as a list of indices into `node_lonlat`.
/// - `relation_bboxes`: each relation as `Some((min_lon, min_lat, max_lon,
///   max_lat))`, or `None` for a no-location (sentinel) relation.
pub fn build_archive(
    node_lonlat: &[(f64, f64)],
    ways: &[Vec<usize>],
    relation_bboxes: &[Option<(f64, f64, f64, f64)>],
) -> Osm {
    let storage = MemoryResourceStorage::new("/test");
    let builder = OsmBuilder::new(storage.clone()).unwrap();

    let mut header = Header::new();
    header.set_coord_scale(COORD_SCALE);
    builder.set_header(&header).unwrap();

    // Nodes, ordered by the z-order curve.
    let curve = node_curve();
    let mut order: Vec<usize> = (0..node_lonlat.len()).collect();
    order.sort_by_key(|&i| node_index(&curve, node_lonlat[i].0, node_lonlat[i].1));
    let mut final_idx = vec![0u64; node_lonlat.len()];
    for (pos, &orig) in order.iter().enumerate() {
        final_idx[orig] = pos as u64;
    }
    // Build via `grow()`/`close()` exactly like osmflatc (not `set_nodes`), so
    // the flatdata range-overlap sentinel is the last *grown* element and gets
    // trimmed from the slice -- making `nodes().len()` the real count, matching
    // real archives. (`set_nodes(&vec_with_sentinel)` would instead leave the
    // sentinel counted, len == real + 1, masking the off-by-one.)
    {
        let mut nodes = builder.start_nodes().unwrap();
        for &orig in &order {
            let (lon, lat) = node_lonlat[orig];
            let n = nodes.grow().unwrap();
            n.set_lon(scale(lon));
            n.set_lat(scale(lat));
            n.set_tag_first_idx(0);
        }
        nodes.grow().unwrap().set_tag_first_idx(0); // range-overlap sentinel
        nodes.close().unwrap();
    }

    // Ways, ordered by the bounding-box curve.
    let wcurve = way_curve();
    let way_bbox = |w: &[usize]| -> (f64, f64, f64, f64) {
        let mut bb: Option<(f64, f64, f64, f64)> = None;
        for &ni in w {
            let (lon, lat) = node_lonlat[ni];
            bb = Some(match bb {
                Some((a, b, c, d)) => (a.min(lon), b.min(lat), c.max(lon), d.max(lat)),
                None => (lon, lat, lon, lat),
            });
        }
        bb.unwrap()
    };
    let mut worder: Vec<usize> = (0..ways.len()).collect();
    worder.sort_by_key(|&i| {
        let (a, b, c, d) = way_bbox(&ways[i]);
        bbox_index(&wcurve, a, b, c, d)
    });
    {
        let mut ways_vec = builder.start_ways().unwrap();
        let mut nodes_index = builder.start_nodes_index().unwrap();
        for &wi in &worder {
            let w = ways_vec.grow().unwrap();
            w.set_tag_first_idx(0);
            w.set_ref_first_idx(nodes_index.len() as u64);
            for &ni in &ways[wi] {
                nodes_index.grow().unwrap().set_value(Some(final_idx[ni]));
            }
        }
        // Sentinel terminates the last way's ref range (read via `Way::refs()`).
        let s = ways_vec.grow().unwrap();
        s.set_tag_first_idx(0);
        s.set_ref_first_idx(nodes_index.len() as u64);
        ways_vec.close().unwrap();
        nodes_index.close().unwrap();
    }

    // Relations, ordered by the bounding-box curve; no-location relations
    // get the sentinel mbb and sort last (key u64::MAX).
    let rel_key = |bbox: Option<(f64, f64, f64, f64)>| match bbox {
        Some((a, b, c, d)) => bbox_index(&wcurve, a, b, c, d),
        None => u64::MAX,
    };
    let mut rorder: Vec<usize> = (0..relation_bboxes.len()).collect();
    rorder.sort_by_key(|&i| rel_key(relation_bboxes[i]));
    {
        let mut rels = builder.start_relations().unwrap();
        for &ri in &rorder {
            let mbb = relation_bboxes[ri]
                .map(|(a, b, c, d)| [scale(a), scale(b), scale(c), scale(d)])
                .unwrap_or(RELATION_NO_BBOX);
            let r = rels.grow().unwrap();
            r.set_tag_first_idx(0);
            r.set_min_lon(mbb[0]);
            r.set_min_lat(mbb[1]);
            r.set_max_lon(mbb[2]);
            r.set_max_lat(mbb[3]);
        }
        rels.grow().unwrap().set_tag_first_idx(0); // range-overlap sentinel
        rels.close().unwrap();
    }

    // One member list per relation, satisfying the `relations` <->
    // `relation_members` implicit binding (lists are empty -- members aren't
    // needed by the spatial queries these fixtures exercise).
    {
        let mut rm = builder.start_relation_members().unwrap();
        for _ in 0..rorder.len() {
            rm.grow().unwrap();
        }
        rm.close().unwrap();
    }
    builder.set_tags(&[]).unwrap();
    builder.set_tags_index(&[]).unwrap();
    builder.set_stringtable(b"\0").unwrap();

    Osm::open(storage).unwrap()
}

/// Build a node-only archive that also carries the optional `ids` sub-archive
/// with both the forward (index -> id) vectors and the reverse (id -> index)
/// permutation, laid out exactly as `osmflatc --reverse-ids` writes them: nodes
/// in z-order, `ids.nodes` parallel to that order, and `ids.nodes_by_id` a
/// permutation ordered so `ids.nodes[nodes_by_id[k]]` ascends by id.
///
/// `nodes` is `(lon, lat, osm_id)` per node. Used to exercise the id<->index
/// lookups in [`crate::ids`].
pub fn build_node_archive_with_ids(nodes: &[(f64, f64, u64)]) -> Osm {
    use crate::{Id, IdxRef};

    let storage = MemoryResourceStorage::new("/test-ids");
    let builder = OsmBuilder::new(storage.clone()).unwrap();

    let mut header = Header::new();
    header.set_coord_scale(COORD_SCALE);
    builder.set_header(&header).unwrap();

    // Nodes in z-order (the order osmflatc emits, recovered here by sorting).
    let curve = node_curve();
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&i| node_index(&curve, nodes[i].0, nodes[i].1));

    // grow()-based like osmflatc, so `nodes().len()` is the real count.
    {
        let mut nodes_ev = builder.start_nodes().unwrap();
        for &orig in &order {
            let (lon, lat, _) = nodes[orig];
            let n = nodes_ev.grow().unwrap();
            n.set_lon(scale(lon));
            n.set_lat(scale(lat));
            n.set_tag_first_idx(0);
        }
        nodes_ev.grow().unwrap().set_tag_first_idx(0); // range-overlap sentinel
        nodes_ev.close().unwrap();
    }

    let ids = builder.ids().unwrap();

    // Forward: ids.nodes[i] is the id of the node at spatial index i.
    let mut fwd: Vec<Id> = order
        .iter()
        .map(|&orig| {
            let mut e = Id::new();
            e.set_value(nodes[orig].2);
            e
        })
        .collect();
    ids.set_nodes(&fwd).unwrap();

    // Reverse: permutation of spatial indices ordered by ascending id.
    let mut perm: Vec<u64> = (0..order.len() as u64).collect();
    perm.sort_by_key(|&spatial_idx| fwd[spatial_idx as usize].value());
    let by_id: Vec<IdxRef> = perm
        .iter()
        .map(|&spatial_idx| {
            let mut e = IdxRef::new();
            e.set_value(spatial_idx);
            e
        })
        .collect();
    ids.set_nodes_by_id(&by_id).unwrap();

    // Ways/relations are empty here but their forward id vectors must exist for
    // the sub-archive to open.
    fwd.clear();
    ids.set_ways(&fwd).unwrap();
    ids.set_relations(&fwd).unwrap();

    // Empty ways / relations, built via grow() of just the range-overlap
    // sentinel (len 0). NB: `set_ways(&[])` would crash -- `as_bytes` for a
    // `@range` (overlapping) struct reads one element past the slice base, and
    // an empty slice's base pointer is dangling. `nodes_index` is not
    // overlapping, so an empty `set_` is fine there.
    {
        let mut w = builder.start_ways().unwrap();
        w.grow().unwrap();
        w.close().unwrap();
    }
    {
        let mut r = builder.start_relations().unwrap();
        r.grow().unwrap();
        r.close().unwrap();
    }
    builder.set_nodes_index(&[]).unwrap();
    builder.start_relation_members().unwrap().close().unwrap();
    builder.set_tags(&[]).unwrap();
    builder.set_tags_index(&[]).unwrap();
    builder.set_stringtable(b"\0").unwrap();

    Osm::open(storage).unwrap()
}

/// Small deterministic RNG (SplitMix64) used by the synthetic-archive
/// generators so benchmark runs are reproducible from their seed.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }

    /// Next value in `[0, 1)`.
    fn next_unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Deterministically generate a synthetic archive of `num_ways` ways, each
/// referencing `refs_per_way` nodes, scattered across the bounding box
/// `(lon_min, lat_min, lon_max, lat_max)`. Returns an archive laid out via
/// [`build_archive`], suitable for benchmarking the spatial query functions.
///
/// The generator is a pure function of its arguments (seeded by `seed`), so
/// benchmark runs are reproducible.
pub fn generate_way_archive(
    num_ways: usize,
    refs_per_way: usize,
    lon_min: f64,
    lat_min: f64,
    lon_max: f64,
    lat_max: f64,
    seed: u64,
) -> Osm {
    let mut rng = Rng::new(seed);
    let lon_span = lon_max - lon_min;
    let lat_span = lat_max - lat_min;
    let mut node_lonlat: Vec<(f64, f64)> = Vec::with_capacity(num_ways * refs_per_way);
    let mut ways: Vec<Vec<usize>> = Vec::with_capacity(num_ways);

    for _ in 0..num_ways {
        // Anchor the way somewhere in the box, then jitter its nodes in a small
        // neighbourhood so ways have a realistic, non-degenerate bounding box.
        let anchor_lon = lon_min + rng.next_unit() * lon_span;
        let anchor_lat = lat_min + rng.next_unit() * lat_span;
        let mut refs = Vec::with_capacity(refs_per_way);
        for _ in 0..refs_per_way {
            let lon =
                (anchor_lon + (rng.next_unit() - 0.5) * lon_span * 0.01).clamp(lon_min, lon_max);
            let lat =
                (anchor_lat + (rng.next_unit() - 0.5) * lat_span * 0.01).clamp(lat_min, lat_max);
            refs.push(node_lonlat.len());
            node_lonlat.push((lon, lat));
        }
        ways.push(refs);
    }

    build_archive(&node_lonlat, &ways, &[])
}

/// Deterministically generate a synthetic archive of `num_nodes` standalone
/// nodes scattered uniformly across `(lon_min, lat_min, lon_max, lat_max)`.
/// Laid out via [`build_archive`], for benchmarking
/// [`find_nodes_by_bounding_box`](crate::find_nodes_by_bounding_box).
pub fn generate_node_archive(
    num_nodes: usize,
    lon_min: f64,
    lat_min: f64,
    lon_max: f64,
    lat_max: f64,
    seed: u64,
) -> Osm {
    let mut rng = Rng::new(seed);
    let lon_span = lon_max - lon_min;
    let lat_span = lat_max - lat_min;
    let node_lonlat: Vec<(f64, f64)> = (0..num_nodes)
        .map(|_| {
            (
                lon_min + rng.next_unit() * lon_span,
                lat_min + rng.next_unit() * lat_span,
            )
        })
        .collect();

    build_archive(&node_lonlat, &[], &[])
}

/// Deterministically generate a synthetic archive of `num_relations` relations,
/// each with a small bounding box anchored uniformly across `(lon_min, lat_min,
/// lon_max, lat_max)`. Laid out via [`build_archive`], for benchmarking
/// [`find_relations_by_bounding_box`](crate::find_relations_by_bounding_box).
pub fn generate_relation_archive(
    num_relations: usize,
    lon_min: f64,
    lat_min: f64,
    lon_max: f64,
    lat_max: f64,
    seed: u64,
) -> Osm {
    let mut rng = Rng::new(seed);
    let lon_span = lon_max - lon_min;
    let lat_span = lat_max - lat_min;
    let relation_bboxes: Vec<Option<(f64, f64, f64, f64)>> = (0..num_relations)
        .map(|_| {
            // Anchor a small box in the region; half-extents stay positive so
            // min <= max always holds after clamping to the region.
            let cx = lon_min + rng.next_unit() * lon_span;
            let cy = lat_min + rng.next_unit() * lat_span;
            let hw = rng.next_unit() * lon_span * 0.01;
            let hh = rng.next_unit() * lat_span * 0.01;
            Some((
                (cx - hw).max(lon_min),
                (cy - hh).max(lat_min),
                (cx + hw).min(lon_max),
                (cy + hh).min(lat_max),
            ))
        })
        .collect();

    build_archive(&[], &[], &relation_bboxes)
}
