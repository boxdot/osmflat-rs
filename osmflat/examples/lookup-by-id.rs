//! Looks up an entity by its original OSM id and prints it.
//!
//! Demonstrates the id <-> archive-index helpers in [`osmflat`]:
//!
//! * `node_idx_by_id` / `way_idx_by_id` / `relation_idx_by_id` resolve an OSM
//!   id to its position in the spatially-ordered archive (reverse lookup), and
//! * `node_id` / `way_id` / `relation_id` go the other way (index -> id).
//!
//! Both require the optional `ids` sub-archive; the reverse lookups
//! additionally require the permutation written by `osmflatc --reverse-ids`.
//! Build a suitable archive with:
//!
//! ```text
//! osmflatc --reverse-ids input.osm.pbf out.osm.flatdata
//! ```
//!
//! Then, e.g.:
//!
//! ```text
//! cargo run --example lookup-by-id -- out.osm.flatdata --osm-type way --id 12345678
//! ```
//!
//! LICENSE
//!
//! The code in this example file is released into the Public Domain.

use clap::Parser;
use osmflat::{
    iter_tags, node_id, node_idx_by_id, relation_id, relation_idx_by_id, way_id, way_idx_by_id,
    FileResourceStorage, Osm,
};
use std::path::PathBuf;
use std::str;

#[derive(Debug, clap::ValueEnum, Clone, Copy)]
enum OsmType {
    Node,
    Way,
    Relation,
}

/// Looks up an entity in an osmflat archive by its OSM id.
#[derive(Debug, Parser)]
struct Args {
    /// input osmflat archive (built with `osmflatc --reverse-ids`)
    input: PathBuf,
    /// the OSM id to look up
    #[clap(long)]
    id: u64,
    /// one of node, way, relation
    #[clap(long, default_value = "node")]
    osm_type: OsmType,
}

fn print_tags(archive: &Osm, range: std::ops::Range<u64>) {
    for (k, v) in iter_tags(archive, range) {
        if let (Ok(k), Ok(v)) = (str::from_utf8(k), str::from_utf8(v)) {
            println!("  {k} = {v}");
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let archive = Osm::open(FileResourceStorage::new(&args.input))?;

    if archive.ids().is_none() {
        eprintln!(
            "archive has no `ids` sub-archive; rebuild with `osmflatc --reverse-ids <pbf> <out>`"
        );
        std::process::exit(1);
    }

    let scale = |v: i32| v as f64 / archive.header().coord_scale() as f64;

    // Reverse lookup: OSM id -> archive index.
    let (idx, kind) = match args.osm_type {
        OsmType::Node => (node_idx_by_id(&archive, args.id), "node"),
        OsmType::Way => (way_idx_by_id(&archive, args.id), "way"),
        OsmType::Relation => (relation_idx_by_id(&archive, args.id), "relation"),
    };

    let Some(idx) = idx else {
        println!("{kind} {} not found in archive", args.id);
        return Ok(());
    };
    println!("{kind} {} is at archive index {idx}", args.id);

    match args.osm_type {
        OsmType::Node => {
            let n = &archive.nodes()[idx];
            println!("  lon = {}, lat = {}", scale(n.lon()), scale(n.lat()));
            print_tags(&archive, n.tags());
        }
        OsmType::Way => {
            let w = &archive.ways()[idx];
            println!("  {} node refs", w.refs().count());
            print_tags(&archive, w.tags());
        }
        OsmType::Relation => {
            let r = &archive.relations()[idx];
            println!(
                "  bbox lon=[{}, {}] lat=[{}, {}]",
                scale(r.min_lon()),
                scale(r.max_lon()),
                scale(r.min_lat()),
                scale(r.max_lat()),
            );
            print_tags(&archive, r.tags());
        }
    }

    // Forward lookup round-trip: index -> id must return what we started with.
    let back = match args.osm_type {
        OsmType::Node => node_id(&archive, idx),
        OsmType::Way => way_id(&archive, idx),
        OsmType::Relation => relation_id(&archive, idx),
    };
    assert_eq!(back, Some(args.id), "index -> id round-trip mismatch");
    println!("index {idx} maps back to id {} (round-trip ok)", args.id);

    Ok(())
}
