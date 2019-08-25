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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_dir = std::env::args()
        .nth(1)
        .ok_or("USAGE: pub_names <osmflat-archive>")?;
    let archive = Osm::open(FileResourceStorage::new(archive_dir))?;

    let nodes_tags = archive.nodes().iter().map(|node| node.tags());
    let ways_tags = archive.ways().iter().map(|way| way.tags());

    let tags = archive.tags();
    let tag_index = archive.tags_index();
    let get_tag = |idx| tags.at(tag_index.at(idx as usize).value() as usize);

    let strings = archive.stringtable();

    for tag_range in nodes_tags.chain(ways_tags) {
        let is_pub = tag_range.clone().map(get_tag).any(|tag| {
            strings[tag.key_idx() as usize..].starts_with(b"amenity\0")
                && strings[tag.value_idx() as usize..].starts_with(b"pub\0")
        });

        if is_pub {
            let name = osmflat::get_tag(&archive, tag_range.clone(), b"name");
            println!(
                "{}",
                name.unwrap_or(Some("broken pub name"))
                    .unwrap_or("unknown pub name")
            );

            let addrs = tag_range
                .clone()
                .map(get_tag)
                .filter(|tag| strings[tag.key_idx() as usize..].starts_with(b"addr:"));
            for tag in addrs {
                let key = strings.substring(tag.key_idx() as usize);
                let value = strings.substring(tag.value_idx() as usize);
                match (key, value) {
                    (Ok(addr_type), Ok(addr)) => println!("  {}: {}", addr_type, addr),
                    _ => (),
                }
            }
        }
    }

    Ok(())
}
