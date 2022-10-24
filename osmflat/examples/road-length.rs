//! Calculates the length of the road network (everything tagged `highway=*`)
//! in the input archive.
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

use itertools::Itertools;
use osmflat::{FileResourceStorage, Node, Osm};

struct Coords {
    lat: f64,
    lon: f64,
}

impl Coords {
    fn from_node(node: &Node, coord_scale: u64) -> Self {
        Self {
            lat: node.lat() as f64 / coord_scale as f64,
            lon: node.lon() as f64 / coord_scale as f64,
        }
    }
}

fn haversine_distance(c1: Coords, c2: Coords) -> f64 {
    /// Earth's radius for WGS84 in meters
    const EARTH_RADIUS_IN_METERS: f64 = 6_372_797.560_856;

    let mut lonh = ((c1.lon - c2.lon).to_radians() * 0.5).sin();
    lonh *= lonh;
    let mut lath = ((c1.lat - c2.lat).to_radians() * 0.5).sin();
    lath *= lath;
    let tmp = c1.lat.to_radians().cos() * c2.lat.to_radians().cos();
    2.0 * EARTH_RADIUS_IN_METERS * (lath + tmp * lonh).sqrt().asin()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_dir = std::env::args()
        .nth(1)
        .ok_or("USAGE: road_length <osmflat-archive>")?;
    let archive = Osm::open(FileResourceStorage::new(archive_dir))?;
    let header = archive.header();

    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let strings = archive.stringtable();

    let highways = archive.ways().iter().filter(|way| {
        way.tags().any(|idx| {
            // A way reference a range of tags by storing a contiguous range of
            // indexes in `tags_index`. Each of these references a tag in `tags`.
            // This is a common pattern when flattening 1 to n relations.
            let tag = &tags[tags_index[idx as usize].value() as usize];
            strings.substring_raw(tag.key_idx() as usize) == b"highway"
        })
    });

    let nodes = archive.nodes();
    let nodes_index = archive.nodes_index();

    let lengths = highways.filter_map(|way| {
        let coords = way.refs().map(|idx| {
            // A way references a range of nodes by storing a contiguous range of
            // indexes in `nodes_index`. Each of these references a node in `nodes`.
            // This is a common pattern when flattening 1 to n relations.
            Some(Coords::from_node(
                &nodes[nodes_index[idx as usize].value()? as usize],
                header.coord_scale(),
            ))
        });
        let length: Option<f64> = coords
            .clone()
            .zip(coords.skip(1))
            .map(|(from, to)| Some(haversine_distance(from?, to?)))
            .fold_options(0.0, |acc, x| acc + x);
        length
    });

    let length: f64 = lengths.sum();
    println!("Length: {:.0} km", length / 1000.0);

    Ok(())
}
