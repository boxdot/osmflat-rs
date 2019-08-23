//! Calculate the length of the road network (everything tagged `highway=*`)
//! from the input archive.
//!
//! Demonstrates
//!
//!  * iteration through ways
//!  * accessing of tags belonging to a way
//!  * accessing of nodes belonging to a way
//!  * length calculation on the Earth using the haversine function
//!
//! LICENSE
//!
//! The code in this example file is released into the Public Domain.

use osmflat::{Archive, FileResourceStorage, Osm};
use std::str;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_dir = std::env::args()
        .nth(1)
        .ok_or("USAGE: pub_names <osmflat-archive>")?;
    let archive = Osm::open(FileResourceStorage::new(archive_dir))?;

    let nodes_tags = archive.nodes().iter().map(|node| node.tags());
    let ways_tags = archive.ways().iter().map(|way| way.tags());

    for tag_range in nodes_tags.chain(ways_tags) {
        if osmflat::get_tag_raw(&archive, tag_range.clone(), b"amenity") == Some(b"pub") {
            let name = osmflat::get_tag(&archive, tag_range.clone(), b"name");
            println!(
                "{}",
                name.unwrap_or(Some("broken pub name"))
                    .unwrap_or("unknown pub name")
            );

            // TODO: Also expose find_tag(archive, range, pred)?
            let addrs =
                osmflat::tags_raw(&archive, tag_range).filter(|(k, _)| k.starts_with(b"addr:"));
            for (k, v) in addrs {
                match (str::from_utf8(k), str::from_utf8(v)) {
                    (Ok(addr_type), Ok(addr)) => println!("  {}: {}", addr_type, addr),
                    _ => (),
                }
            }
        }
    }

    Ok(())
}
