extern crate bresenham;
extern crate docopt;
extern crate failure;
extern crate flatdata;
extern crate haversine;
extern crate itertools;
extern crate osmflat;
#[macro_use]
extern crate serde_derive;
extern crate svg;

use bresenham::Bresenham;
use docopt::Docopt;
use failure::Error;
use flatdata::{Archive, FileResourceStorage};
use haversine::{distance, Location};
use svg::node::element::{Group, Polygon, Polyline};
use svg::Document;

use std::cell::RefCell;
use std::convert;
use std::ops::Deref;
use std::rc::Rc;
use std::str;

const USAGE: &str = "
Example renderer. Support PNG and SVG.

Usage:
  render <input> <output> [--width=<px>]

Options:
  --width=<px>    canvas width [default: 4000]
";

#[derive(Debug, Deserialize)]
pub struct Args {
    arg_input: String,
    arg_output: std::path::PathBuf,
    flag_width: u32,
}

pub fn parse_args() -> Args {
    Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
struct GeoCoord {
    lat: f64,
    lon: f64,
}

impl GeoCoord {
    fn min(self, other: Self) -> Self {
        Self {
            lat: self.lat.min(other.lat),
            lon: self.lon.min(other.lon),
        }
    }

    fn max(self, other: Self) -> Self {
        Self {
            lat: self.lat.max(other.lat),
            lon: self.lon.max(other.lon),
        }
    }
}

impl<N: Deref<Target = osmflat::Node>> convert::From<N> for GeoCoord {
    fn from(node: N) -> Self {
        const COORD_SCALE: f64 = 0.000000001;
        Self {
            lat: node.lat() as f64 * COORD_SCALE,
            lon: node.lon() as f64 * COORD_SCALE,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[derive(Debug, Clone)]
struct MapTransform {
    width: u32,
    height: u32,
    min_x: f64,
    min_y: f64,
    map_w: f64,
    map_h: f64,
}

impl MapTransform {
    fn new(width: u32, height: u32, min: GeoCoord, max: GeoCoord) -> Self {
        Self {
            width,
            height,
            min_x: min.lon,
            min_y: min.lat,
            map_w: max.lon - min.lon,
            map_h: max.lat - min.lat,
        }
    }

    fn transform(&self, coord: GeoCoord) -> (isize, isize) {
        (
            ((coord.lon - self.min_x) / self.map_w * self.width as f64) as isize,
            ((1f64 - (coord.lat - self.min_y) / self.map_h) * self.height as f64) as isize,
        )
    }

    fn transform_meters(&self, distance: u32) -> u32 {
        let start = haversine::Location {
            latitude: self.min_x,
            longitude: self.min_y,
        };
        let end = haversine::Location {
            latitude: self.map_h / self.height as f64 + self.min_y,
            longitude: self.map_w / self.width as f64 + self.min_x,
        };
        let pt_distance = haversine::distance(start, end, haversine::Units::Kilometers) / 1000.;
        (distance as f64 / pt_distance) as u32
    }
}

#[derive(Clone)]
struct NodesIterator<'a> {
    nodes: flatdata::ArrayView<'a, osmflat::Node>,
    nodes_index: flatdata::ArrayView<'a, osmflat::NodeIndex>,
    next: usize,
    end: usize,
}

impl<'a> NodesIterator<'a> {
    fn from_way(archive: &'a osmflat::Osm, way: &osmflat::Way, next_way: &osmflat::Way) -> Self {
        Self {
            nodes: archive.nodes(),
            nodes_index: archive.nodes_index(),
            next: way.ref_first_idx() as usize,
            end: next_way.ref_first_idx() as usize,
        }
    }

    fn from_way_type(archive: &'a osmflat::Osm, way_type: &WayType) -> Self {
        let (next, end) = match way_type {
            WayType::Road {
                start_node_idx,
                end_node_idx,
            } => (start_node_idx, end_node_idx),
            WayType::River {
                start_node_idx,
                end_node_idx,
            } => (start_node_idx, end_node_idx),
            WayType::Riverbank {
                start_node_idx,
                end_node_idx,
            } => (start_node_idx, end_node_idx),
        };
        Self {
            nodes: archive.nodes(),
            nodes_index: archive.nodes_index(),
            next: *next as usize,
            end: *end as usize,
        }
    }
}

impl<'a> Iterator for NodesIterator<'a> {
    type Item = flatdata::Handle<'a, osmflat::Node>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next < self.end {
            let idx = self.next;
            self.next += 1;
            Some(self.nodes.at(self.nodes_index.at(idx).value() as usize))
        } else {
            None
        }
    }
}

fn substring(strings: &str, start: u32) -> &str {
    let start = start as usize;
    let end = strings[start..].find('\0').expect("invalid string");
    &strings[start..start + end]
}

enum WayType {
    Road {
        start_node_idx: u32,
        end_node_idx: u32,
    },
    River {
        start_node_idx: u32,
        end_node_idx: u32,
    },
    Riverbank {
        start_node_idx: u32,
        end_node_idx: u32,
    },
}

enum LayerType {
    Park,
    River,
}

struct Layer {
    relation_idx: u32,
    layer_type: LayerType,
}

fn way_filter(
    way: &osmflat::Way,
    next_way: &osmflat::Way,
    tags_index: &flatdata::ArrayView<osmflat::TagIndex>,
    tags: &flatdata::ArrayView<osmflat::Tag>,
    strings: &str,
) -> Option<WayType> {
    // Filter all ways that have less than 2 nodes.
    let start_node_idx = way.ref_first_idx();
    let end_node_idx = next_way.ref_first_idx();
    if end_node_idx - start_node_idx < 2 {
        return None;
    }

    // Filter all ways that do not have a highway tag. Also check for specific
    // values.
    let start_tag_idx = way.tag_first_idx();
    let end_tag_idx = next_way.tag_first_idx();
    for tag_idx in start_tag_idx..end_tag_idx {
        let tag = tags.at(tags_index.at(tag_idx as usize).value() as usize);
        let key = substring(strings, tag.key_idx());
        let val = substring(strings, tag.value_idx());
        if key == "highway" {
            if val == "pedestrian"
                || val == "steps"
                || val == "footway"
                || val == "construction"
                || val == "bic"
                || val == "cycleway"
                || val == "layby"
                || val == "bridleway"
                || val == "path"
            {
                return None;
            }
            return Some(WayType::Road {
                start_node_idx,
                end_node_idx,
            });
        } else if key == "waterway" && (val == "canal" || val == "river") {
            return Some(WayType::River {
                start_node_idx,
                end_node_idx,
            });
        } else if (key == "waterway" && val == "riverbank") || (key == "water" && val == "lake") {
            return Some(WayType::Riverbank {
                start_node_idx,
                end_node_idx,
            });
        }
    }

    None
}

fn generate(
    archive: &osmflat::Osm,
    output_path: &std::path::Path,
    width: u32,
) -> Result<(), Error> {
    let relations = archive.relations();
    let relation_members = archive.relation_members();
    let ways = archive.ways();
    let nodes = archive.nodes();
    let tags_index = archive.tags_index();
    let tags = archive.tags();
    let strings = str::from_utf8(archive.stringtable())
        .expect("stringtable contains invalid utf8 characters");

    let multipolygons_indexes = relations
        .iter()
        .zip(relations.iter().skip(1))
        .enumerate()
        .filter_map(|(relation_idx, (relation, next_relation))| {
            let start_tag_idx = relation.tag_first_idx();
            let end_tag_idx = next_relation.tag_first_idx();

            let mut is_multipolygon = false;
            let mut is_green = false;
            let mut is_wet = false;
            for tag_idx in start_tag_idx..end_tag_idx {
                let tag = tags.at(tags_index.at(tag_idx as usize).value() as usize);
                let key = substring(strings, tag.key_idx());
                let val = substring(strings, tag.value_idx());
                if key == "type" && val == "multipolygon" {
                    is_multipolygon = true;
                }
                if key == "landuse"
                    && (val == "forest" || val == "grass" || val == "recreation_ground")
                {
                    is_green = true;
                }
                if (key == "natural" && val == "water") || (key == "waterway" && val == "river") {
                    is_wet = true;
                }
            }

            if is_multipolygon && is_green {
                Some(Layer {
                    layer_type: LayerType::Park,
                    relation_idx: relation_idx as u32,
                })
            } else if is_multipolygon && is_wet {
                Some(Layer {
                    layer_type: LayerType::River,
                    relation_idx: relation_idx as u32,
                })
            } else {
                None
            }
        });

    let roads = ways
        .iter()
        .zip(ways.iter().skip(1))
        .filter_map(|(way, next_way)| way_filter(&*way, &*next_way, &tags_index, &tags, strings));

    // compute extent
    let mut coords = roads
        .clone()
        .flat_map(|way_type| NodesIterator::from_way_type(archive, &way_type).map(GeoCoord::from));

    let first_coord = coords.next().expect("no roads found");
    let (min, max) = coords.fold((first_coord, first_coord), |(min, max), coord| {
        (min.min(coord), max.max(coord))
    });

    // compute ratio and height
    let ratio = 360. / 180. * (max.lat - min.lat) / (max.lon - min.lon);
    let height = (width as f64 * ratio) as u32;

    // create world -> raster transformation
    let t = MapTransform::new(width - 1, height - 1, min, max);

    // create paths
    let paths = roads.map(|way_type| {
        let raster_coords = NodesIterator::from_way_type(archive, &way_type)
            .map(GeoCoord::from)
            .map(|coord| t.transform(coord));
        (raster_coords, way_type)
    });

    let mut document = Document::new().set("viewBox", (0, 0, width, height));
    let mut road_group = Group::new().set("stroke", "#001F3F").set("fill", "none");
    let mut park_group = Group::new()
        .set("stroke", "#3D9970")
        .set("fill", "#3D9970")
        .set("fill-opacity", 0.7);
    let mut river_group = Group::new().set("stroke", "#0074D9").set("fill", "#0074D9");
    for (raster_coords, way_type) in paths {
        let v: Vec<String> = raster_coords.map(|(x, y)| format!("{},{}", x, y)).collect();
        match way_type {
            WayType::Road { .. } => {
                let mut polyline = Polyline::new().set("points", v.join(" "));
                road_group = road_group.add(polyline);
            }
            WayType::Riverbank {
                start_node_idx: _,
                end_node_idx: _,
            } => {
                let polygon = Polygon::new()
                    .set("points", v.join(" "))
                    .set("stroke-opacity", 1);
                river_group = river_group.add(polygon);
            }
            WayType::River {
                start_node_idx: _,
                end_node_idx: _,
            } => {
                let polyline = Polyline::new()
                    .set("points", v.join(" "))
                    .set("fill", "none")
                    .set("stroke-opacity", 1)
                    .set("stroke-width", 5);
                river_group = river_group.add(polyline);
            }
        }
    }

    for layer in multipolygons_indexes {
        let mut points: Vec<(isize, isize)> = Vec::new();
        for mut relation_member in relation_members.at(layer.relation_idx as usize) {
            let mut p = match *relation_member {
                osmflat::RelationMembers::RelationMember(ref relation_member) => {
                    // if relation_member.relation_idx() == layer.relation_idx {
                    // // println!("relation_member {} belonging to multipolygon  {}",
                    // relation_member_idx, multipolygon_idx); }
                    vec![]
                }
                osmflat::RelationMembers::WayMember(ref way_member) => {
                    //println!("{}", way_member.relation_idx());
                    let role = substring(strings, way_member.role_idx());
                    if role == "outer" {
                        // println!("way_member {} belonging to relation {}", relation_member_idx,
                        // multipolygon_idx);
                        let way = ways.at(way_member.way_idx() as usize);
                        let next_way = ways.at(way_member.way_idx() as usize + 1);
                        let points: Vec<(isize, isize)> = NodesIterator::from_way(
                            archive, &way, &next_way,
                        ).map(GeoCoord::from)
                            .map(|coord| t.transform(coord))
                            .collect();
                        points
                    } else {
                        vec![]
                    }
                }
                osmflat::RelationMembers::NodeMember(ref node_member) => {
                    let role = substring(strings, node_member.role_idx());
                    if role == "outer" {
                        // println!("node_member {} belonging to relation {}", relation_member_idx,
                        // multipolygon_idx);
                        let node = nodes.at(node_member.node_idx() as usize);
                        vec![t.transform(GeoCoord::from(node))]
                    } else {
                        vec![]
                    }
                }
            };
            points.append(&mut p);
        }

        let v: Vec<String> = points.iter().map(|(x, y)| format!("{},{}", x, y)).collect();

        match layer.layer_type {
            LayerType::Park => {
                let polygon = Polygon::new().set("points", v.join(" "));
                park_group = park_group.add(polygon);
            }
            LayerType::River => {
                let polygon = Polygon::new()
                    .set("points", v.join(" "))
                    .set("fill", "#0074D9");
                river_group = river_group.add(polygon);
            }
        }
    }

    document = document.add(road_group).add(park_group).add(river_group);
    svg::save(output_path, &document)?;
    Ok(())
}

fn main() -> Result<(), Error> {
    let args = parse_args();
    let storage = Rc::new(RefCell::new(FileResourceStorage::new(
        args.arg_input.into(),
    )));
    let archive = osmflat::Osm::open(storage)?;
    generate(&archive, &args.arg_output, args.flag_width)?;
    Ok(())
}
