//! Shows the names and addresses of all pubs.
//!
//! Demonstrates
//!
//!  * iteration through tags belonging to a node and a way
//!  * accessing of tags by key
//!  * filtering of tags
//!
//! LICENSE
//!
//! The code in this example file is released into the Public Domain.

use osmflat::{find_tag, has_tag, iter_tags, FileResourceStorage, Osm};
use std::str;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_dir = std::env::args()
        .nth(1)
        .ok_or("USAGE: pub_names <osmflat-archive>")?;
    let archive = Osm::open(FileResourceStorage::new(archive_dir))?;

    let nodes_tags = archive.nodes().iter().map(|node| node.tags());
    let ways_tags = archive.ways().iter().map(|way| way.tags());

    for tag_range in nodes_tags.chain(ways_tags) {
        if has_tag(&archive, tag_range.clone(), b"amenity", b"pub") {
            let name = find_tag(&archive, tag_range.clone(), b"name");
            let name = name.map(|s| str::from_utf8(s).unwrap_or("broken pub name"));
            println!("{}", name.unwrap_or("unknown pub name"));

            let addrs = iter_tags(&archive, tag_range).filter(|(k, _)| k.starts_with(b"addr:"));
            for (k, v) in addrs {
                if let (Ok(addr_type), Ok(addr)) = (str::from_utf8(k), str::from_utf8(v)) {
                    println!("  {addr_type}: {addr}");
                }
            }
        }
    }

    Ok(())
}
