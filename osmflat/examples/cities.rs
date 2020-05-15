//! Scans all OSM nodes and extracts list of cities with name and
//! population in JSON format.
//!
//! LICENSE
//!
//! The code in this example file is released into the Public Domain.

use osmflat::{find_tag, has_tag, Osm};
use serde::Serialize;
use std::str;

#[derive(Debug, Default, Serialize)]
struct City<'a> {
    name: &'a str,
    population: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_dir = std::env::args()
        .nth(1)
        .ok_or("USAGE: cities <osmflat-archive>")?;
    let archive = Osm::open(osmflat::FileResourceStorage::new(archive_dir))?;

    // Iterate through all nodes
    let cities: Vec<City> = archive
        .nodes()
        .iter()
        // filter nodes that does not have a place=city tag
        .filter(|node| has_tag(&archive, node.tags(), b"place", b"city"))
        .filter_map(|node| {
            // try to collect population and country
            Some(City {
                name: str::from_utf8(find_tag(&archive, node.tags(), b"name")?).ok()?,
                population: str::from_utf8(find_tag(&archive, node.tags(), b"population")?)
                    .ok()?
                    .parse()
                    .ok()?,
            })
        })
        .collect();

    let stdout = std::io::stdout();
    serde_json::to_writer(stdout.lock(), &cities)?;

    Ok(())
}
