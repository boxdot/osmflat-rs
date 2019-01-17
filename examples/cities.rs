//! This example program scans all OSM nodes and extracts list of cities with
//! name and population in JSON format.

use flatdata::Archive;
use osmflat::{FileResourceStorage, Osm};
use serde::Serialize;

use std::env;
use std::error::Error;
use std::io;

#[derive(Debug, Default, Serialize)]
struct City {
    name: String,
    population: usize,
}

/// Strings in an osmflat archive are stored in a single bytes blob separated by
/// `0` character. Given an index, this function extracts a utf8 string starting
/// at this index from the blob.
fn get_str(strings: &[u8], start: usize) -> &str {
    let end = strings[start..]
        .iter()
        .position(|&c| c == 0)
        .expect("invalid string");
    std::str::from_utf8(&strings[start..start + end]).expect("invalid string")
}

// Returns an iterator though all tags of a node given by its index in osmflat
// archive.
fn tags_for_node(archive: &Osm, node_idx: usize) -> impl Iterator<Item = (&str, &str)> + Clone {
    // first tag index of the node
    let tags_start = archive.nodes().at(node_idx).tag_first_idx();
    // the last tag index of the node is the first tag index of the next node
    let tags_end = archive.nodes().at(node_idx + 1).tag_first_idx();
    (tags_start..tags_end).map(move |idx| {
        // tag_index vector maps tags ranges in nodes, ways, and relations
        // to actual tags
        let index = archive.tags_index().at(idx as usize);
        let tag = archive.tags().at(index.value() as usize);
        let key = get_str(archive.stringtable(), tag.key_idx() as usize);
        let value = get_str(archive.stringtable(), tag.value_idx() as usize);
        (key, value)
    })
}

fn main() -> Result<(), Box<Error>> {
    let archive_dir = env::args()
        .nth(1)
        .ok_or_else(|| "USAGE: cities <osmflat-archive>")?
        .into();
    let archive = Osm::open(FileResourceStorage::new(archive_dir))?;

    // Iterate through all nodes (except last one, which is a sentinel i.e. does not
    // contain data).
    let cities: Vec<City> = (0..archive.nodes().len() - 1)
        .filter_map(|node_idx| {
            let mut tags = tags_for_node(&archive, node_idx);
            // filter nodes that does not have a place=city tag
            if tags.clone().any(|(k, v)| k == "place" && v == "city") {
                // ry to collect population and country
                let city = tags.try_fold(City::default(), |mut city, (k, v)| {
                    if k == "name" {
                        city.name = v.into();
                    } else if k == "population" {
                        city.population = v.parse().ok()?;
                    }
                    Some(city)
                });
                // filter out cities without names
                city.and_then(|c| if !c.name.is_empty() { Some(c) } else { None })
            } else {
                None
            }
        })
        .collect();

    let stdout = io::stdout();
    serde_json::to_writer(stdout.lock(), &cities)?;

    Ok(())
}
