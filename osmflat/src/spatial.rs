use space_time::{xzorder::xz2_sfc::XZ2SFC, zorder::z_curve_2d::ZCurve2D};

use crate::{Node, Osm, Relation, Way};

/// Resolution of the [XZ2SFC] curve used to order ways and relations.
///
/// This value is the single source of truth shared by the `osmflatc`
/// compiler (which writes the archives in this order) and the query
/// functions below (which binary-search assuming this order). Changing it
/// here changes both sides at once; the two must never be configured
/// independently or the spatial ordering would silently desync.
pub const SPATIAL_RESOLUTION: u32 = 18;

/// The curve used to order and query [Node]s by location.
pub fn node_curve() -> ZCurve2D {
    ZCurve2D::default()
}

/// The curve used to order and query [Way]s (and relations) by their
/// minimum bounding box.
pub fn way_curve() -> XZ2SFC {
    XZ2SFC::wgs84(SPATIAL_RESOLUTION)
}

/// Spatial index of a single point. Both the compiler and the query side
/// must compute node indices through this function so they stay identical.
pub fn node_index(curve: &ZCurve2D, lon: f64, lat: f64) -> u64 {
    curve.index(lon, lat)
}

/// Spatial index of a bounding box, used for ways and relations. Both the
/// compiler and the query side must compute indices through this function.
pub fn bbox_index(curve: &XZ2SFC, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> u64 {
    curve.index(min_x, min_y, max_x, max_y)
}

/// Sentinel bounding box stored for relations that have no resolvable member
/// geometry (every member lies outside the extract). It is an inverted box
/// using unreachable coordinates, so it never overlaps a query and the relation
/// is never returned spatially — but the relation is still present in the
/// archive and readable by iteration or id. The compiler writes this value and
/// the query side recognizes it; the two must agree.
pub const RELATION_NO_BBOX: [i32; 4] = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];

/// Return [Node]s from the archive that are inside the
/// bounding box.
pub fn find_nodes_by_bounding_box(
    archive: &Osm,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
) -> impl Iterator<Item = &Node> {
    let curve = node_curve();

    let coord_scale = archive.header().coord_scale();
    let ranges = curve.ranges(xmin, ymin, xmax, ymax, &[]);

    // `Node` carries a `@range` field, so flatdata trims the trailing
    // range-overlap sentinel out of the slice: `len()` is already the count of
    // real nodes, all of which take part in the search.
    let num_nodes = archive.nodes().len();

    ranges
        .into_iter()
        .flat_map(move |b| {
            let nodes = &archive.nodes()[..num_nodes];
            let lower_index =
                nodes.partition_point(|n| spatial_index_node(&curve, n, coord_scale) < b.lower());
            let upper_relative_index = nodes[lower_index..]
                .partition_point(|n| spatial_index_node(&curve, n, coord_scale) <= b.upper());

            nodes[lower_index..(lower_index + upper_relative_index)].iter()
        })
        .filter(move |n| {
            let lat = n.lat() as f64 / coord_scale as f64;
            let lon = n.lon() as f64 / coord_scale as f64;

            lat >= ymin && lat <= ymax && lon >= xmin && lon <= xmax
        })
}

pub(crate) fn spatial_index_node(curve: &ZCurve2D, node: &Node, coord_scale: i32) -> u64 {
    let lat = node.lat() as f64 / coord_scale as f64;
    let lon = node.lon() as f64 / coord_scale as f64;

    node_index(curve, lon, lat)
}

/// Recompute the minimum bounding box `(min_x, min_y, max_x, max_y)` of the
/// way at `way_idx` from its node references, in degrees. Returns `None` when
/// the way has no resolvable node locations.
///
/// This mirrors the bounding box the compiler computed when it placed the way
/// in spatial order, so binary searching the ways with the index derived from
/// it is valid.
fn way_bounding_box(
    archive: &Osm,
    way_idx: usize,
    coord_scale: f64,
) -> Option<(f64, f64, f64, f64)> {
    let ways = archive.ways();
    // The generated `refs()` range reads the *next* way's `ref_first_idx`
    // straight from the backing buffer (which includes the trailing overlap
    // sentinel), so it is valid even for the last real way -- unlike indexing
    // `ways[way_idx + 1]`, which would run off the sentinel-trimmed slice.
    let refs = ways[way_idx].refs();
    let (begin, end) = (refs.start as usize, refs.end as usize);

    let nodes_index = archive.nodes_index();
    let nodes = archive.nodes();

    let mut bbox: Option<(f64, f64, f64, f64)> = None;
    for ni in &nodes_index[begin..end] {
        let Some(node_idx) = ni.value() else { continue };
        let node = &nodes[node_idx as usize];
        let x = node.lon() as f64 / coord_scale;
        let y = node.lat() as f64 / coord_scale;
        bbox = Some(match bbox {
            Some((min_x, min_y, max_x, max_y)) => {
                (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
            }
            None => (x, y, x, y),
        });
    }
    bbox
}

pub(crate) fn spatial_index_way(
    archive: &Osm,
    curve: &XZ2SFC,
    way_idx: usize,
    coord_scale: f64,
) -> u64 {
    match way_bounding_box(archive, way_idx, coord_scale) {
        Some((min_x, min_y, max_x, max_y)) => bbox_index(curve, min_x, min_y, max_x, max_y),
        None => 0,
    }
}

/// Partition point over the index range `0..len`, mirroring the semantics of
/// [`slice::partition_point`] but allowing the predicate to look at neighbours
/// (needed because a way's geometry spans the following entry).
fn partition_point_by(len: usize, mut pred: impl FnMut(usize) -> bool) -> usize {
    let mut lo = 0;
    let mut hi = len;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if pred(mid) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Maximum number of space-filling-curve ranges the bounding-box queries ask
/// [`XZ2SFC::ranges`] to produce for ways and relations. `None` lets it refine
/// fully (the most ranges, and the most range-generation cost); a cap stops
/// refinement early, yielding coarser ranges that are cheaper to produce but
/// over-select more candidates for the exact-overlap filter to drop.
/// Correctness is unaffected either way — coverage is preserved.
///
/// `Some(64)` is the sweet spot of the `max_ranges` bench (the `ranges()` alloc
/// is the profiled hotspot). The optimum cap grows with query-box size, so 64
/// is chosen to balance across box sizes: near-optimal on tight/medium boxes
/// (the common case — ways ~130/180 µs, relations ~95/100 µs) with a bounded
/// worst case on wide boxes (ways ~520 µs, relations ~160 µs). That worst case
/// is still ~44×/114× faster than `None`, whose cost explodes with box size
/// (ways reach ~23 ms on a wide box). `Some(8)` is uniformly worse — too
/// coarse, it over-selects a huge candidate set.
const BBOX_QUERY_MAX_RANGES: Option<u16> = Some(64);

/// Return [Way]s that are in the archive and inside the bounding box.
///
/// #Note: Includes those [Way]s that overlap the bounding box without
///        being fully contained by it.
pub fn find_ways_by_bounding_box(
    archive: &Osm,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
) -> impl Iterator<Item = &Way> {
    find_ways_capped(archive, xmin, ymin, xmax, ymax, BBOX_QUERY_MAX_RANGES)
}

/// [`find_ways_by_bounding_box`] with an explicit `XZ2SFC::ranges` cap, exposed
/// for benchmarking the effect of [`BBOX_QUERY_MAX_RANGES`].
#[cfg(any(test, feature = "test-support"))]
pub fn find_ways_by_bounding_box_capped(
    archive: &Osm,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
    max_ranges: Option<u16>,
) -> impl Iterator<Item = &Way> {
    find_ways_capped(archive, xmin, ymin, xmax, ymax, max_ranges)
}

fn find_ways_capped(
    archive: &Osm,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
    max_ranges: Option<u16>,
) -> impl Iterator<Item = &Way> {
    let curve = way_curve();
    let coord_scale = archive.header().coord_scale() as f64;
    // `Way` has `@range` fields, so flatdata trims its overlap sentinel out of
    // the slice: `len()` is the real way count.
    let num_ways = archive.ways().len();

    curve
        .ranges(xmin, ymin, xmax, ymax, max_ranges)
        .into_iter()
        .flat_map(move |b| {
            let lower = partition_point_by(num_ways, |i| {
                spatial_index_way(archive, &curve, i, coord_scale) < b.lower()
            });
            let upper = partition_point_by(num_ways, |i| {
                spatial_index_way(archive, &curve, i, coord_scale) <= b.upper()
            });
            lower..upper
        })
        .filter(move |&i| match way_bounding_box(archive, i, coord_scale) {
            // Exact overlap test to drop the false positives the curve ranges
            // over-select.
            Some((min_x, min_y, max_x, max_y)) => {
                !(max_x < xmin || min_x > xmax || max_y < ymin || min_y > ymax)
            }
            None => false,
        })
        .map(move |i| &archive.ways()[i])
}

/// Bounding box of a relation in degrees, or `None` if it carries the
/// [`RELATION_NO_BBOX`] sentinel (no resolvable member geometry).
fn relation_bbox(relation: &Relation, coord_scale: f64) -> Option<(f64, f64, f64, f64)> {
    let raw = [
        relation.min_lon(),
        relation.min_lat(),
        relation.max_lon(),
        relation.max_lat(),
    ];
    if raw == RELATION_NO_BBOX {
        return None;
    }
    Some((
        raw[0] as f64 / coord_scale,
        raw[1] as f64 / coord_scale,
        raw[2] as f64 / coord_scale,
        raw[3] as f64 / coord_scale,
    ))
}

pub(crate) fn spatial_index_relation(curve: &XZ2SFC, relation: &Relation, coord_scale: f64) -> u64 {
    // Sentinel relations sort last and are never matched; they share the key
    // the compiler used, keeping the binary search monotonic.
    match relation_bbox(relation, coord_scale) {
        Some((min_x, min_y, max_x, max_y)) => bbox_index(curve, min_x, min_y, max_x, max_y),
        None => u64::MAX,
    }
}

/// Return [Relation]s in the archive whose bounding box overlaps the query box.
///
/// #Note: Includes those [Relation]s that overlap the bounding box without
///        being fully contained by it.
pub fn find_relations_by_bounding_box(
    archive: &Osm,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
) -> impl Iterator<Item = &Relation> {
    find_relations_capped(archive, xmin, ymin, xmax, ymax, BBOX_QUERY_MAX_RANGES)
}

/// [`find_relations_by_bounding_box`] with an explicit `XZ2SFC::ranges` cap,
/// exposed for benchmarking the effect of [`BBOX_QUERY_MAX_RANGES`].
#[cfg(any(test, feature = "test-support"))]
pub fn find_relations_by_bounding_box_capped(
    archive: &Osm,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
    max_ranges: Option<u16>,
) -> impl Iterator<Item = &Relation> {
    find_relations_capped(archive, xmin, ymin, xmax, ymax, max_ranges)
}

fn find_relations_capped(
    archive: &Osm,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
    max_ranges: Option<u16>,
) -> impl Iterator<Item = &Relation> {
    let curve = way_curve();
    let coord_scale = archive.header().coord_scale() as f64;
    // `Relation` has a `@range` field, so flatdata trims its overlap sentinel
    // out of the slice: `len()` is the real relation count.
    let num_relations = archive.relations().len();

    curve
        .ranges(xmin, ymin, xmax, ymax, max_ranges)
        .into_iter()
        .flat_map(move |b| {
            let relations = &archive.relations()[..num_relations];
            let lower = relations
                .partition_point(|r| spatial_index_relation(&curve, r, coord_scale) < b.lower());
            let upper_relative = relations[lower..]
                .partition_point(|r| spatial_index_relation(&curve, r, coord_scale) <= b.upper());

            relations[lower..(lower + upper_relative)].iter()
        })
        .filter(move |r| match relation_bbox(r, coord_scale) {
            // Exact overlap test to drop the false positives the curve ranges
            // over-select. Sentinel (no-location) relations are never returned.
            Some((min_x, min_y, max_x, max_y)) => {
                !(max_x < xmin || min_x > xmax || max_y < ymin || min_y > ymax)
            }
            None => false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{build_archive, unscale};

    #[test]
    fn finds_nodes_in_box_and_excludes_outside() {
        let nodes = vec![
            (-93.0, 45.0),  // 0 inside
            (-93.1, 45.1),  // 1 inside
            (-100.0, 40.0), // 2 outside (west + south)
            (-80.0, 30.0),  // 3 outside (east + south)
            (-92.5, 44.5),  // 4 inside
        ];
        let archive = build_archive(&nodes, &[], &[]);

        let mut found: Vec<(f64, f64)> =
            find_nodes_by_bounding_box(&archive, -93.5, 44.4, -92.4, 45.5)
                .map(|n| (unscale(n.lon()), unscale(n.lat())))
                .collect();
        found.sort_by(|a, b| a.partial_cmp(b).unwrap());

        assert_eq!(found.len(), 3, "expected 3 nodes inside, got {found:?}");
        let inside = |lon: f64, lat: f64| {
            found
                .iter()
                .any(|&(x, y)| (x - lon).abs() < 1e-6 && (y - lat).abs() < 1e-6)
        };
        assert!(inside(-93.0, 45.0));
        assert!(inside(-93.1, 45.1));
        assert!(inside(-92.5, 44.5));
    }

    #[test]
    fn finds_ways_overlapping_box() {
        let nodes = vec![(-93.0, 45.0), (-93.1, 45.1), (-100.0, 30.0), (-80.0, 40.0)];
        // way 0 is inside the query box; way 1 is far south (no lat overlap).
        let ways = vec![vec![0, 1], vec![2, 3]];
        let archive = build_archive(&nodes, &ways, &[]);

        let found: Vec<usize> = find_ways_by_bounding_box(&archive, -93.5, 44.4, -92.4, 45.5)
            .map(|w| w.refs().count())
            .collect();

        assert_eq!(found.len(), 1, "expected exactly one overlapping way");
        assert_eq!(found[0], 2, "the matching way has two node refs");
    }

    #[test]
    fn finds_relations_overlapping_box() {
        let relations = vec![
            Some((-93.2, 44.9, -92.9, 45.2)),  // overlaps
            Some((-100.0, 30.0, -80.0, 40.0)), // far south, no overlap
        ];
        let archive = build_archive(&[], &[], &relations);

        let found: Vec<(f64, f64, f64, f64)> =
            find_relations_by_bounding_box(&archive, -93.5, 44.4, -92.4, 45.5)
                .map(|r| {
                    (
                        unscale(r.min_lon()),
                        unscale(r.min_lat()),
                        unscale(r.max_lon()),
                        unscale(r.max_lat()),
                    )
                })
                .collect();

        assert_eq!(found.len(), 1, "expected one overlapping relation");
        assert!((found[0].0 - -93.2).abs() < 1e-6);
        assert!((found[0].3 - 45.2).abs() < 1e-6);
    }

    #[test]
    fn sentinel_relation_is_never_returned() {
        // One located relation overlapping the box, one no-location relation.
        let relations = vec![Some((-93.2, 44.9, -92.9, 45.2)), None];
        let archive = build_archive(&[], &[], &relations);

        // The query over the located relation returns exactly it, not the sentinel.
        assert_eq!(
            find_relations_by_bounding_box(&archive, -93.5, 44.4, -92.4, 45.5).count(),
            1
        );
        // A query over null island must not surface the sentinel either.
        assert_eq!(
            find_relations_by_bounding_box(&archive, -0.001, -0.001, 0.001, 0.001).count(),
            0
        );
    }

    #[test]
    fn last_entity_in_z_order_is_not_dropped() {
        // A world-covering box must return *every* entity, including whichever
        // sorts last in spatial order. This guards the off-by-one where the
        // queries excluded the last real entity by over-applying
        // `saturating_sub(1)` to a `len()` that already omits the flatdata
        // range-overlap sentinel.
        let nodes = vec![
            (-93.0, 45.0),
            (2.3, 48.8),
            (139.7, 35.7),
            (-0.1, 51.5),
            (151.2, -33.9),
        ];
        let world = (-180.0, -90.0, 180.0, 90.0);

        let a = build_archive(&nodes, &[], &[]);
        assert_eq!(
            find_nodes_by_bounding_box(&a, world.0, world.1, world.2, world.3).count(),
            nodes.len(),
            "every node must be returned"
        );

        let ways = vec![vec![0, 1], vec![2, 3], vec![3, 4]];
        let a = build_archive(&nodes, &ways, &[]);
        assert_eq!(
            find_ways_by_bounding_box(&a, world.0, world.1, world.2, world.3).count(),
            ways.len(),
            "every way must be returned"
        );

        let rels = vec![
            Some((-93.2, 44.9, -92.9, 45.2)),
            Some((2.0, 48.0, 3.0, 49.0)),
            Some((150.0, -34.0, 152.0, -33.0)),
        ];
        let a = build_archive(&[], &[], &rels);
        assert_eq!(
            find_relations_by_bounding_box(&a, world.0, world.1, world.2, world.3).count(),
            rels.len(),
            "every relation must be returned"
        );
    }

    #[test]
    fn empty_box_far_away_finds_nothing() {
        let nodes = vec![(-93.0, 45.0), (-93.1, 45.1)];
        let archive = build_archive(&nodes, &[], &[]);
        assert_eq!(
            find_nodes_by_bounding_box(&archive, 10.0, 10.0, 11.0, 11.0).count(),
            0
        );
    }

    /// Ground-truth regression for the bbox queries: across a spread of
    /// off-center boxes, a small `XZ2SFC::ranges` cap must return exactly the
    /// entities whose bbox overlaps the query (brute force). This guards the
    /// query path against the under-coverage that full refinement (`None`)
    /// exhibits at box boundaries.
    #[test]
    fn small_cap_matches_brute_force() {
        use crate::test_support::{generate_relation_archive, generate_way_archive};

        let (lon_min, lat_min, lon_max, lat_max) = (-97.5, 43.0, -89.5, 49.5);
        let ways = generate_way_archive(5000, 8, lon_min, lat_min, lon_max, lat_max, 7);
        let rels = generate_relation_archive(5000, lon_min, lat_min, lon_max, lat_max, 7);
        let cs = ways.header().coord_scale() as f64;

        let mut s = 0x1234_5678u64;
        let mut u = || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            (s >> 33) as f64 / (1u64 << 31) as f64
        };

        for _ in 0..12 {
            let frac = 0.05 + u() * 0.55;
            let wlon = (lon_max - lon_min) * frac;
            let wlat = (lat_max - lat_min) * frac;
            let x0 = lon_min + u() * (lon_max - lon_min - wlon);
            let y0 = lat_min + u() * (lat_max - lat_min - wlat);
            let (x1, y1) = (x0 + wlon, y0 + wlat);
            let overlaps = |min_x: f64, min_y: f64, max_x: f64, max_y: f64| {
                !(max_x < x0 || min_x > x1 || max_y < y0 || min_y > y1)
            };

            let nw = ways.ways().len();
            let bf_w = (0..nw)
                .filter(|&i| {
                    way_bounding_box(&ways, i, cs).is_some_and(|(a, b, c, d)| overlaps(a, b, c, d))
                })
                .count();
            let nr = rels.relations().len();
            let bf_r = rels.relations()[..nr]
                .iter()
                .filter(|r| relation_bbox(r, cs).is_some_and(|(a, b, c, d)| overlaps(a, b, c, d)))
                .count();

            for cap in [Some(1u16), Some(8), Some(64), None] {
                assert_eq!(
                    find_ways_by_bounding_box_capped(&ways, x0, y0, x1, y1, cap).count(),
                    bf_w,
                    "ways cap={cap:?} box=({x0:.2},{y0:.2},{x1:.2},{y1:.2})"
                );
                assert_eq!(
                    find_relations_by_bounding_box_capped(&rels, x0, y0, x1, y1, cap).count(),
                    bf_r,
                    "relations cap={cap:?} box=({x0:.2},{y0:.2},{x1:.2},{y1:.2})"
                );
            }
        }
    }
}
