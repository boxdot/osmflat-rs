//! Verifies the internal consistency of an osmflat archive.
//!
//! Runs [`osmflat::verify`], which checks referential integrity, `@range`
//! tiling, the relations/relation_members implicit binding, spatial ordering,
//! and the id permutation + reverse-lookup round-trip -- all from the archive
//! alone (no source PBF needed). Exits non-zero if any invariant is violated.
//!
//! ```text
//! cargo run --example verify -- archive.osm.flatdata
//! ```
//!
//! LICENSE
//!
//! The code in this example file is released into the Public Domain.

use clap::Parser;
use osmflat::{verify, FileResourceStorage, Osm};
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    /// input osmflat archive
    input: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let archive = Osm::open(FileResourceStorage::new(&args.input))?;

    let report = verify(&archive);
    let s = &report.stats;

    println!(
        "scanned: nodes={} ways={} relations={} relation-members={}",
        s.nodes, s.ways, s.relations, s.members
    );
    println!(
        "ids sub-archive: present={} reverse-index={}",
        s.ids_present, s.reverse_index_present
    );
    println!(
        "missing refs/members (None, expected at boundaries): {}",
        s.missing_refs
    );

    if report.is_clean() {
        println!("OK: no invariant violations");
        Ok(())
    } else {
        println!(
            "FOUND {} violation(s) (showing first {}):",
            report.total,
            report.violations.len()
        );
        for v in &report.violations {
            println!("  {v:?}");
        }
        std::process::exit(1);
    }
}
