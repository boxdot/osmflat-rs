//! Counts the number of nodes, ways, and relations in the input archive.
//!
//! LICENSE
//!
//! The code in this example file is released into the Public Domain.

use osmflat::{FileResourceStorage, Osm};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_dir = std::env::args()
        .nth(1)
        .ok_or("USAGE: debug <osmflat-archive>")?;
    let archive = Osm::open(FileResourceStorage::new(archive_dir))?;

    println!("Nodes: {}", archive.nodes().len());
    println!("Ways: {}", archive.ways().len());
    println!("Relations: {}", archive.relations().len());

    Ok(())
}
