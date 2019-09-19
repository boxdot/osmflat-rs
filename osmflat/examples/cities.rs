//! This example program scans all OSM nodes and extracts list of cities with
//! name and population in JSON format.

use flatdata::Archive;
use serde::Serialize;

#[derive(Debug, Default, Serialize)]
struct City {
    name: String,
    population: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_dir = std::env::args()
        .nth(1)
        .ok_or("USAGE: cities <osmflat-archive>")?;
    let archive = osmflat::Osm::open(osmflat::FileResourceStorage::new(archive_dir))?;

    // Iterate through all nodes
    let cities: Vec<City> = archive
        .nodes()
        .iter()
        // filter nodes that does not have a place=city tag
        .filter(|node| osmflat::tags(&archive, node.tags()).any(|tag| tag == Ok(("place", "city"))))
        .filter_map(|node| {
            // try to collect population and country
            let get_tag = |key: &str| {
                osmflat::tags(&archive, node.tags()).find_map(|tag| match tag {
                    Ok((k, v)) if key == k => Some(v),
                    _ => None,
                })
            };
            Some(City {
                name: get_tag("name")?.into(),
                population: get_tag("population")?.parse().ok()?,
            })
        })
        .collect();

    let stdout = std::io::stdout();
    serde_json::to_writer(stdout.lock(), &cities)?;

    Ok(())
}
