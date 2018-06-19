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

use docopt::Docopt;
use failure::Error;
use flatdata::{Archive, FileResourceStorage};
use itertools::Itertools;
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

    fn _transform_meters(&self, distance: u32) -> u32 {
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

#[derive(Debug, Clone, Copy)]
enum Multipolygon {
    Park(usize),
    River(usize),
}

impl Into<usize> for Multipolygon {
    fn into(self) -> usize {
        match self {
            Multipolygon::Park(idx) => idx,
            Multipolygon::River(idx) => idx,
        }
    }
}

enum PathType {
    Road,
    River,
    Riverbank,
    Park,
}

struct Path {
    start_node_idx: u32,
    end_node_idx: u32,
    is_multipolygon: bool,
    path_type: PathType,
}

fn way_filter(
    way: &osmflat::Way,
    next_way: &osmflat::Way,
    tags_index: &flatdata::ArrayView<osmflat::TagIndex>,
    tags: &flatdata::ArrayView<osmflat::Tag>,
    strings: &str,
) -> Option<Path> {
    // Filter all ways that have less than 2 nodes.
    let start_node_idx = way.ref_first_idx();
    let end_node_idx = next_way.ref_first_idx();
    if end_node_idx - start_node_idx < 2 {
        return None;
    }

    // Filter all ways that we want to render.
    let start_tag_idx = way.tag_first_idx();
    let end_tag_idx = next_way.tag_first_idx();
    for tag_idx in start_tag_idx..end_tag_idx {
        let tag = tags.at(tags_index.at(tag_idx as usize).value() as usize);
        let key = substring(strings, tag.key_idx());
        let val = substring(strings, tag.value_idx());
        if key == "highway"
            && !(val == "pedestrian"
                || val == "steps"
                || val == "footway"
                || val == "construction"
                || val == "bic"
                || val == "cycleway"
                || val == "layby"
                || val == "bridleway"
                || val == "path")
        {
            return Some(Path {
                start_node_idx,
                end_node_idx,
                is_multipolygon: false,
                path_type: PathType::Road,
            });
        } else if key == "waterway" && (val == "canal" || val == "river") {
            return Some(Path {
                start_node_idx,
                end_node_idx,
                is_multipolygon: false,
                path_type: PathType::River,
            });
        } else if (key == "waterway" && val == "riverbank") || (key == "water" && val == "lake") {
            return Some(Path {
                start_node_idx,
                end_node_idx,
                is_multipolygon: false,
                path_type: PathType::Riverbank,
            });
        }
    }
    None
}

fn relation_filter(
    relation_idx: usize,
    relation: &osmflat::Relation,
    next_relation: &osmflat::Relation,
    tags_index: &flatdata::ArrayView<osmflat::TagIndex>,
    tags: &flatdata::ArrayView<osmflat::Tag>,
    strings: &str,
) -> Option<Multipolygon> {
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
            && (val == "forest"
                || val == "grass"
                || val == "recreation_ground"
                || val == "cemetery") || (key == "leisure" && val == "park")
        {
            is_green = true;
        }
        if (key == "natural" && val == "water") || (key == "waterway" && val == "river") {
            is_wet = true;
        }
    }

    if is_multipolygon && is_green {
        Some(Multipolygon::Park(relation_idx))
    } else if is_multipolygon && is_wet {
        Some(Multipolygon::River(relation_idx))
    } else {
        None
    }
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

    // multipolygons compose stuff like parks, lakes, etc.
    let multipolygons = relations
        .iter()
        .zip(relations.iter().skip(1))
        .enumerate()
        .filter_map(|(relation_idx, (relation, next_relation))| {
            relation_filter(
                relation_idx,
                &*relation,
                &*next_relation,
                &tags_index,
                &tags,
                strings,
            )
        })
        .flat_map(|multipolygon| {
            let nodes = &nodes;
            let ways = &ways;
            relation_members
                .at(multipolygon.into())
                .filter_map(move |relation_member| {
                    let path_type = match multipolygon {
                        Multipolygon::Park(_) => PathType::Park,
                        Multipolygon::River(_) => PathType::River,
                    };
                    match *relation_member {
                        osmflat::RelationMembers::RelationMember(ref _relation_member) => None,
                        osmflat::RelationMembers::WayMember(ref way_member) => {
                            let role = substring(strings, way_member.role_idx());
                            if role == "outer" {
                                let way = ways.at(way_member.way_idx() as usize);
                                let next_way = ways.at(way_member.way_idx() as usize + 1);
                                Some(Path {
                                    start_node_idx: way.ref_first_idx(),
                                    end_node_idx: next_way.ref_first_idx(),
                                    is_multipolygon: true,
                                    path_type: path_type,
                                })
                            } else {
                                None
                            }
                        }
                        osmflat::RelationMembers::NodeMember(ref node_member) => {
                            let role = substring(strings, node_member.role_idx());
                            if role == "outer" {
                                let node = nodes.at(node_member.node_idx() as usize);
                                Some(Path {
                                    start_node_idx: node_member.node_idx(),
                                    end_node_idx: node_member.node_idx() + 1,
                                    is_multipolygon: true,
                                    path_type: path_type,
                                })
                            } else {
                                None
                            }
                        }
                    }
                })
        }).fold(
            Vec::<Path>::new(),
            |mut paths, mut path| {
                let first_polygon_point = paths.last().and_then(|path| {
                    GeoCoord::from(nodes.at(path.start_node_idx))
                });
                let first_polygon_point = paths.last().and_then(|path| {
                    GeoCoord::from(nodes.at(path.end_node_idx - 1))
                });
                let first_point = new_points.first().map(|p| *p);
                let last_point = new_points.last().map(|p| *p);

                if last_polygon_point.is_none() || last_polygon_point == first_point {
                    polygon.append(&mut new_points);
                }

                if first_polygon_point == last_point
                    || !(last_polygon_point.is_none() || last_polygon_point == first_point)
                {
                    let points = polygon
                        .iter()
                        .map(|(x, y)| format!("{},{}", x, y))
                        .join(" ");
                    match multipolygon {
                        Multipolygon::Park(_) => {
                            let polygon = Polygon::new().set("points", points);
                            park_group = park_group.add(polygon);
                        }
                        Multipolygon::River(_) => {
                            let polygon = Polygon::new().set("points", points);
                            river_group = river_group.add(polygon);
                        }
                    };
                    polygon.clear();
                }
                (polygon, park_group, river_group)
            }
        });

    // all the driveable roads
    let roads = ways
        .iter()
        .zip(ways.iter().skip(1))
        .filter_map(|(way, next_way)| way_filter(&*way, &*next_way, &tags_index, &tags, strings));

    let work_to_do: Vec<_> = multipolygons.chain(roads).collect();

    work_to_do.par_iter().map(|unit_of_work| {

    });

    // compute extent
    // let mut coords = roads
    //     .clone()
    //     .flat_map(|way_type| NodesIterator::from_way_type(archive, &way_type).map(GeoCoord::from));

    // let first_coord = coords.next().expect("no roads found");
    // let (min, max) = coords.fold((first_coord, first_coord), |(min, max), coord| {
    //     (min.min(coord), max.max(coord))
    // });

    // // compute ratio and height
    // let ratio = 360. / 180. * (max.lat - min.lat) / (max.lon - min.lon);
    // let height = (width as f64 * ratio) as u32;

    // // create world -> raster transformation
    // let t = MapTransform::new(width - 1, height - 1, min, max);

    // create paths
    // let paths = roads.map(|way_type| {
    //     let raster_coords = NodesIterator::from_way_type(archive, &way_type)
    //         .map(GeoCoord::from)
    //         .map(|coord| t.transform(coord));
    //     (raster_coords, way_type)
    // });

    let mut document = Document::new().set("viewBox", (0, 0, width, width));
    let mut road_group = Group::new()
        .set("stroke", "#001F3F")
        .set("opacity", 0.7)
        .set("fill", "none");
    let park_group = Group::new()
        .set("stroke", "black")
        .set("fill", "#3D9970")
        .set("opacity", 0.3);
    let mut river_ways_group = Group::new()
        .set("fill", "none")
        .set("stroke", "#B5D5F3")
        .set("stroke-width", 5);
    let mut river_group = Group::new().set("stroke", "#B5D5F3").set("fill", "#B5D5F3");

    // for (raster_coords, way_type) in paths {
    //     let v: Vec<String> = raster_coords.map(|(x, y)| format!("{},{}", x, y)).collect();
    //     match way_type {
    //         WayType::Road { .. } => {
    //             let mut polyline = Polyline::new().set("points", v.join(" "));
    //             road_group = road_group.add(polyline);
    //         }
    //         WayType::Riverbank { .. } => {
    //             let polygon = Polygon::new().set("points", v.join(" "));
    //             river_group = river_group.add(polygon);
    //         }
    //         WayType::River { .. } => {
    //             let polyline = Polyline::new().set("points", v.join(" "));
    //             river_ways_group = river_ways_group.add(polyline);
    //         }
    //     }
    // }

    // multipolygons.fold(
    //     (Vec::<(isize, isize)>::new(), park_group, river_group),
    //     |(mut polygon, mut park_group, mut river_group), (multipolygon, mut new_points)| {
    //         let first_polygon_point = polygon.first().map(|p| *p);
    //         let last_polygon_point = polygon.last().map(|p| *p);
    //         let first_point = new_points.first().map(|p| *p);
    //         let last_point = new_points.last().map(|p| *p);

    //         if last_polygon_point.is_none() || last_polygon_point == first_point {
    //             polygon.append(&mut new_points);
    //         }

    //         if first_polygon_point == last_point
    //             || !(last_polygon_point.is_none() || last_polygon_point == first_point)
    //         {
    //             let points = polygon
    //                 .iter()
    //                 .map(|(x, y)| format!("{},{}", x, y))
    //                 .join(" ");
    //             match multipolygon {
    //                 Multipolygon::Park(_) => {
    //                     let polygon = Polygon::new().set("points", points);
    //                     park_group = park_group.add(polygon);
    //                 }
    //                 Multipolygon::River(_) => {
    //                     let polygon = Polygon::new().set("points", points);
    //                     river_group = river_group.add(polygon);
    //                 }
    //             };
    //             polygon.clear();
    //         }
    //         (polygon, park_group, river_group)
    //     },
    // );

    document = document
        .add(road_group)
        .add(park_group)
        .add(river_group)
        .add(river_ways_group);
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
