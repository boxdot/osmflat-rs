//! Sweep of the `XZ2SFC::ranges` `max_ranges` cap for the ways/relations
//! queries, to pick the value to bake into `BBOX_QUERY_MAX_RANGES`.
//!
//! Smaller caps make `ranges()` cheaper (the profiled hotspot) but over-select
//! more candidates for the exact-overlap filter. This sweep finds the optimum.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use osmflat::{
    find_relations_by_bounding_box_capped, find_ways_by_bounding_box_capped,
    test_support::{generate_relation_archive, generate_way_archive},
};

const LON_MIN: f64 = -97.5;
const LAT_MIN: f64 = 43.0;
const LON_MAX: f64 = -89.5;
const LAT_MAX: f64 = 49.5;
const SEED: u64 = 0xC0FFEE;
const COUNT: usize = 100_000;
const CAPS: [Option<u16>; 7] = [
    Some(8),
    Some(32),
    Some(64),
    Some(128),
    Some(512),
    Some(2048),
    None,
];
/// Query-box sizes as a fraction of the data extent: a tight, a medium, and a
/// wide box. The optimal `max_ranges` typically shifts with box size, so a
/// single baked constant must hold up across all three.
const FRACS: [f64; 3] = [0.02, 0.1, 0.3];

fn query_box(fraction: f64) -> (f64, f64, f64, f64) {
    let cx = (LON_MIN + LON_MAX) / 2.0;
    let cy = (LAT_MIN + LAT_MAX) / 2.0;
    let hw = (LON_MAX - LON_MIN) / 2.0 * fraction;
    let hh = (LAT_MAX - LAT_MIN) / 2.0 * fraction;
    (cx - hw, cy - hh, cx + hw, cy + hh)
}

fn label(cap: Option<u16>) -> String {
    match cap {
        Some(n) => n.to_string(),
        None => "none".to_string(),
    }
}

fn bench_ways(c: &mut Criterion) {
    let archive = generate_way_archive(COUNT, 10, LON_MIN, LAT_MIN, LON_MAX, LAT_MAX, SEED);
    for frac in FRACS {
        let (x0, y0, x1, y1) = query_box(frac);
        let mut group = c.benchmark_group(format!("ways_max_ranges@frac={frac}"));
        for cap in CAPS {
            group.bench_with_input(BenchmarkId::from_parameter(label(cap)), &cap, |b, &cap| {
                b.iter(|| find_ways_by_bounding_box_capped(&archive, x0, y0, x1, y1, cap).count())
            });
        }
        group.finish();
    }
}

fn bench_relations(c: &mut Criterion) {
    let archive = generate_relation_archive(COUNT, LON_MIN, LAT_MIN, LON_MAX, LAT_MAX, SEED);
    for frac in FRACS {
        let (x0, y0, x1, y1) = query_box(frac);
        let mut group = c.benchmark_group(format!("relations_max_ranges@frac={frac}"));
        for cap in CAPS {
            group.bench_with_input(BenchmarkId::from_parameter(label(cap)), &cap, |b, &cap| {
                b.iter(|| {
                    find_relations_by_bounding_box_capped(&archive, x0, y0, x1, y1, cap).count()
                })
            });
        }
        group.finish();
    }
}

criterion_group!(benches, bench_ways, bench_relations);
criterion_main!(benches);
