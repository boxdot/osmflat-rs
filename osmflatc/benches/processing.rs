//! Benchmark for the per-block node serialization stage of `osmflatc`.
//!
//! This is the Layer-2 analog of the osmflat `find_ways` query bench: it drives
//! the internal `serialize_dense_nodes_primative_block` with a synthetic
//! `PrimitiveBlock` and a mock (in-memory) RocksDB batch, so it measures the
//! pure decode -> spatial-curve-key -> encode work without touching disk or a
//! real RocksDB.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use osmflatc::osmpbf::{DenseNodes, PrimitiveBlock, PrimitiveGroup, StringTable};
use osmflatc::processing::mock::MockRocksBatch;
use osmflatc::processing::node::serialize_dense_nodes_primative_block;
use osmflatc::strings::StringTable as OsmStringTable;
use parking_lot::Mutex;

/// Build a synthetic dense-node `PrimitiveBlock` with `n` nodes spread across a
/// region, using delta-encoded ids/coords as a real PBF does. No tags, so the
/// benchmark isolates the coordinate/spatial-key path.
fn synthetic_block(n: i64) -> PrimitiveBlock {
    // Delta-encoded: each entry is the step from the previous node.
    let id = vec![1i64; n as usize];
    // Walk a diagonal across roughly a state-sized region in 1e-7 degree units,
    // wrapping so coordinates stay in range.
    let lat_step = 50_000i64; // ~0.005 deg per node
    let lon_step = 70_000i64;
    let lat = (0..n)
        .map(|i| {
            if i == 0 {
                440_000_000
            } else {
                lat_step * (1 - 2 * (i % 2))
            }
        })
        .collect();
    let lon = (0..n)
        .map(|i| {
            if i == 0 {
                -930_000_000
            } else {
                lon_step * (1 - 2 * (i % 2))
            }
        })
        .collect();

    let dense = DenseNodes {
        id,
        keys_vals: vec![],
        denseinfo: None,
        lat,
        lon,
    };
    let group = PrimitiveGroup {
        dense: Some(dense),
        ..Default::default()
    };
    PrimitiveBlock {
        stringtable: StringTable::default(),
        primitivegroup: vec![group],
        ..Default::default()
    }
}

fn bench_serialize_dense_nodes(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize_dense_nodes_primative_block");

    for n in [1_000i64, 8_000, 64_000] {
        let block = synthetic_block(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &block, |b, block| {
            b.iter(|| {
                let mut batch = MockRocksBatch::default();
                let string_table = Mutex::new(OsmStringTable::default());
                let stats = serialize_dense_nodes_primative_block(
                    black_box(block),
                    100,
                    &mut batch,
                    &string_table,
                    1_000_000,
                )
                .unwrap();
                black_box(stats);
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_serialize_dense_nodes);
criterion_main!(benches);
