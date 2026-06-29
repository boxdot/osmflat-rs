//! Benchmarks for the bounding-box spatial queries.
//!
//! Each query type builds one synthetic archive and measures latency across
//! query-box sizes (wider boxes emit more space-filling-curve ranges). The
//! `find_ways` case is the most interesting: it recomputes each candidate way's
//! bounding box from its node references *inside* the binary-search predicate,
//! so it scales worse with box size than the nodes/relations cases, which read
//! geometry directly (coords inline for nodes, stored mbb for relations).

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use osmflat::{
    find_nodes_by_bounding_box, find_relations_by_bounding_box, find_ways_by_bounding_box,
    test_support::{generate_node_archive, generate_relation_archive, generate_way_archive},
    Osm,
};

// A region roughly the size of a US state, so the curve keys spread out.
const LON_MIN: f64 = -97.5;
const LAT_MIN: f64 = 43.0;
const LON_MAX: f64 = -89.5;
const LAT_MAX: f64 = 49.5;

const SEED: u64 = 0xC0FFEE;

/// Query boxes centered in the region, as a fraction of the full region span.
/// Larger fractions select more entities and emit more curve ranges.
const BOX_FRACTIONS: [f64; 4] = [0.001, 0.01, 0.1, 0.5];

fn query_box(fraction: f64) -> (f64, f64, f64, f64) {
    let cx = (LON_MIN + LON_MAX) / 2.0;
    let cy = (LAT_MIN + LAT_MAX) / 2.0;
    let hw = (LON_MAX - LON_MIN) / 2.0 * fraction;
    let hh = (LAT_MAX - LAT_MIN) / 2.0 * fraction;
    (cx - hw, cy - hh, cx + hw, cy + hh)
}

fn bench_query(
    c: &mut Criterion,
    name: &str,
    count: usize,
    archive: &Osm,
    mut run: impl FnMut(&Osm, f64, f64, f64, f64) -> usize,
) {
    let mut group = c.benchmark_group(name);
    group.throughput(Throughput::Elements(count as u64));

    for fraction in BOX_FRACTIONS {
        let (xmin, ymin, xmax, ymax) = query_box(fraction);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("box_frac={fraction}")),
            &(xmin, ymin, xmax, ymax),
            |b, &(xmin, ymin, xmax, ymax)| b.iter(|| run(archive, xmin, ymin, xmax, ymax)),
        );
    }
    group.finish();
}

fn bench_find_nodes(c: &mut Criterion) {
    let count = 100_000;
    let archive = generate_node_archive(count, LON_MIN, LAT_MIN, LON_MAX, LAT_MAX, SEED);
    bench_query(
        c,
        "find_nodes_by_bounding_box",
        count,
        &archive,
        |a, x0, y0, x1, y1| find_nodes_by_bounding_box(a, x0, y0, x1, y1).count(),
    );
}

fn bench_find_ways(c: &mut Criterion) {
    let count = 100_000;
    let archive = generate_way_archive(count, 10, LON_MIN, LAT_MIN, LON_MAX, LAT_MAX, SEED);
    bench_query(
        c,
        "find_ways_by_bounding_box",
        count,
        &archive,
        |a, x0, y0, x1, y1| find_ways_by_bounding_box(a, x0, y0, x1, y1).count(),
    );
}

fn bench_find_relations(c: &mut Criterion) {
    let count = 100_000;
    let archive = generate_relation_archive(count, LON_MIN, LAT_MIN, LON_MAX, LAT_MAX, SEED);
    bench_query(
        c,
        "find_relations_by_bounding_box",
        count,
        &archive,
        |a, x0, y0, x1, y1| find_relations_by_bounding_box(a, x0, y0, x1, y1).count(),
    );
}

criterion_group!(
    benches,
    bench_find_nodes,
    bench_find_ways,
    bench_find_relations
);
criterion_main!(benches);
