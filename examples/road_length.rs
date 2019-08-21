//! Calculate the length of the road network (everything tagged `highway=*`)
//! from the input archive.
//!
//! Demonstrates
//!
//!  * iteration through way
//!  * accessing of tags
//!  * length calculation on the Earth using the haversine function
//!
//! LICENSE
//!
//! The code in this example file is released into the Public Domain.

use osmflat::{Archive, FileResourceStorage, Osm, RefNode};

struct Coords {
    lat: f64,
    lon: f64,
}

impl Coords {
    fn from_node(node: RefNode) -> Self {
        Self {
            lat: node.lat() as f64 / osmflat::COORD_SCALE as f64,
            lon: node.lon() as f64 / osmflat::COORD_SCALE as f64,
        }
    }
}

fn haversine_distance(c1: Coords, c2: Coords) -> f64 {
    /// Earth's radius for WGS84 in meters
    const EARTH_RADIUS_IN_METERS: f64 = 6372797.560856;

    let mut lonh = ((c1.lon - c2.lon).to_radians() * 0.5).sin();
    lonh *= lonh;
    let mut lath = ((c1.lat - c2.lat).to_radians() * 0.5).sin();
    lath *= lath;
    let tmp = c1.lat.to_radians().cos() * c2.lat.to_radians().cos();
    return 2.0 * EARTH_RADIUS_IN_METERS * (lath + tmp * lonh).sqrt().asin();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_dir = std::env::args()
        .nth(1)
        .ok_or("USAGE: road_length <osmflat-archive>")?;
    let archive = Osm::open(FileResourceStorage::new(archive_dir))?;

    let nodes = archive.nodes();
    let nodes_index = archive.nodes_index();

    let is_highway = |tag: Result<_, _>| tag.map(|(k, _v)| k == "highway").unwrap_or(false);

    let highways = archive
        .ways()
        .iter()
        .filter(|way| osmflat::tags(&archive, way.tags()).any(is_highway));
    let lengths = highways.map(|way| {
        let coords = way.refs().map(|idx| {
            // A way references a range of nodes by storing a contiguous range of
            // indexes in `nodes_index`. Each of these references a node in `nodes`.
            Coords::from_node(nodes.at(nodes_index.at(idx as usize).value() as usize))
        });
        let length: f64 = coords
            .clone()
            .zip(coords.skip(1))
            .map(|(from, to)| haversine_distance(from, to))
            .sum();
        length
    });
    let length: f64 = lengths.sum();
    println!("Length: {} km", length / 1000.0);

    Ok(())
}
