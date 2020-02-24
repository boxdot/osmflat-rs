//! Reads the contents of the input archive.
//!
//! LICENSE
//!
//! The code in this example file is released into the Public Domain.

use osmflat::{Archive, FileResourceStorage, Osm};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_dir = std::env::args()
        .nth(1)
        .ok_or("USAGE: read <osmflat-archive>")?;
    let archive = Osm::open(FileResourceStorage::new(archive_dir))?;

    for _node in archive.nodes() {
        // do nothing
    }

    for _way in archive.ways() {
        // do nothing
    }

    for _relation in archive.relations() {
        // do nothing
    }

    Ok(())
}
