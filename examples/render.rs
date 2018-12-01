extern crate bresenham;
extern crate structopt;
#[macro_use]
extern crate failure;
extern crate flatdata;
extern crate haversine;
extern crate itertools;
extern crate osmflat;
extern crate png;
extern crate svg;

use failure::Error;
use flatdata::{Archive, FileResourceStorage};
use png::HasParameters;
use structopt::StructOpt;
use svg::node::element::{Group, Polyline};
use svg::Document;

use std::convert;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use std::str;

#[derive(Debug, StructOpt)]
#[structopt(name = "osmflat-render")]
struct Args {
    #[structopt(short = "f", long = "flatdata-archive", parse(from_os_str))]
    flatdata_archive: PathBuf,

    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: PathBuf,

    #[structopt(short = "w", long = "width", default_value = "4000")]
    flag_width: u32,
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

impl<'a> convert::From<osmflat::RefNode<'a>> for GeoCoord {
    fn from(node: osmflat::RefNode<'a>) -> Self {
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

#[derive(Debug)]
struct Image {
    w: u32,
    h: u32,
    data: Vec<u8>,
}

impl Image {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            data: vec![255; (w * h) as usize * 4],
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
    fn from_way_type(archive: &'a osmflat::Osm, way_type: &WayType) -> Self {
        let (next, end) = match way_type {
            WayType::Road {
                start_node_idx,
                end_node_idx,
            } => (start_node_idx, end_node_idx),
            WayType::River {
                start_node_idx,
                end_node_idx,
                ..
            } => (start_node_idx, end_node_idx),
            _ => {
                panic!("Can't interate on this WayType");
            }
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
    type Item = osmflat::RefNode<'a>;
    fn next<'b>(&'b mut self) -> Option<Self::Item> {
        if self.next < self.end {
            let idx = self.next;
            self.next += 1;
            Some(self.nodes.at(self.nodes_index.at(idx).value() as usize))
        } else {
            None
        }
    }
}

fn substring(strings: &str, start: u64) -> &str {
    let start = start as usize;
    let end = strings[start..].find('\0').expect("invalid string");
    &strings[start..start + end]
}

enum WayType {
    Park,
    Lake,
    Road {
        start_node_idx: u64,
        end_node_idx: u64,
    },
    River {
        start_node_idx: u64,
        end_node_idx: u64,
        width: u32,
    },
}

fn way_filter(
    way: &osmflat::RefWay,
    next_way: &osmflat::RefWay,
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
        if key == "highway" {
            let val = substring(strings, tag.value_idx());
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
        } else if key == "waterway" {
            for tag_idx in start_tag_idx..end_tag_idx {
                //let tag = tags.at(tags_index.at(tag_idx as usize).value() as usize);
                let key = substring(strings, tag.key_idx());
                if key == "width" || key == "maxwidth" {
                    let val = substring(strings, tag.value_idx());
                    let width: u32 = val.parse().ok()?;
                    return Some(WayType::River {
                        start_node_idx,
                        end_node_idx,
                        width,
                    });
                }
            }
            return Some(WayType::River {
                start_node_idx,
                end_node_idx,
                width: 1,
            });
        }
    }

    None
}

fn render(archive: &osmflat::Osm, output_path: &std::path::Path, width: u32) -> Result<(), Error> {
    let relations = archive.relations();
    let relation_members = archive.relation_members();
    let ways = archive.ways();
    let tags_index = archive.tags_index();
    let tags = archive.tags();
    let strings = str::from_utf8(archive.stringtable())
        .expect("stringtable contains invalid utf8 characters");

    println!("Relations: {}", relations.len());
    let parks = relations
        .iter()
        .zip(relations.iter().skip(1))
        .enumerate()
        .filter_map(|(relation_idx, (relation, next_relation))| {
            let start_tag_idx = relation.tag_first_idx();
            let end_tag_idx = next_relation.tag_first_idx();
            let mut is_multipolygon = false;
            let mut is_park = false;
            let mut is_lake = false;
            for tag_idx in start_tag_idx..end_tag_idx {
                let tag = tags.at(tags_index.at(tag_idx as usize).value() as usize);
                let key = substring(strings, tag.key_idx());
                let val = substring(strings, tag.value_idx());
                if key == "type" && val == "multipolygon" {
                    is_multipolygon = true;
                }
                if (key == "leisure" && val == "park")
                    || (key == "landuse" && (val == "recreation_ground" || val == "forest"))
                {
                    is_park = true;
                }
                if key == "water" && val == "lake" {
                    is_lake = true;
                }
            }
            if is_multipolygon {
                if is_park {
                    return Some((WayType::Park, relation_idx));
                }
            }
            if is_lake {
                return Some((WayType::Lake, relation_idx));
            }
            None
        })
        .map(|(way_type, relation_idx)| {
            let members = relation_members.at(relation_idx);
            let outline = members
                .filter_map(|m| match m {
                    osmflat::RefRelationMembers::WayMember(way_member) => {
                        let role = substring(strings, way_member.role_idx());
                        if role == "outer" {
                            Some(way_member.way_idx())
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .map(|way_idx| {
                    let way = archive.ways().at(way_idx as usize);
                    let next_way = archive.ways().at(way_idx as usize + 1);
                    (way.ref_first_idx(), next_way.ref_first_idx())
                });

            (way_type, outline)
        });

    let roads = ways
        .iter()
        .zip(ways.iter().skip(1))
        .filter_map(|(way, next_way)| way_filter(&way, &next_way, &tags_index, &tags, strings));

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
        (raster_coords.collect(), way_type)
    });

    let park_paths = parks.map(|(way_type, park)| {
        let coordinates = park.fold(vec![], |mut acc, (start_node_idx, end_node_idx)| {
            let nodes = NodesIterator {
                nodes: archive.nodes(),
                nodes_index: archive.nodes_index(),
                next: start_node_idx as usize,
                end: end_node_idx as usize,
            };
            for node in nodes {
                acc.push(t.transform(GeoCoord::from(node)))
            }
            acc
        });

        (coordinates, way_type)
    });

    // detect whether we export svg or render to png
    match output_path.extension() {
        Some(os_str) => match os_str.to_str() {
            Some("png") => {
                let file = File::create(output_path)?;
                let buf = BufWriter::new(file);
                let mut image = Image::new(width, height);
                let mut encoder = png::Encoder::new(buf, width, image.h);
                encoder.set(png::ColorType::RGBA).set(png::BitDepth::Eight);
                let mut writer = encoder.write_header()?;
                // for (nodes_iterator, width) in paths {
                //     //for (x, y) in Bresenham::new(from, to) {
                //     //    image.set(x as u32, y as u32, Color::new(0, 0, 0, 255));
                //     //}
                // }
                writer.write_image_data(&image.data[..])?;
            }
            Some("svg") => {
                let mut document = Document::new().set("viewBox", (0, 0, width, height));
                let mut road_group = Group::new().set("stroke", "#001F3F").set("fill", "none");
                let mut park_group = Group::new()
                    .set("stroke", "#3D9970")
                    .set("fill", "#3D9970")
                    .set("fill-opacity", 0.3);
                let mut river_group = Group::new().set("stroke", "#0074D9").set("fill", "none");
                let mut lake_group = Group::new()
                    .set("stroke", "#0074D9")
                    .set("fill", "#0074D9")
                    .set("fill-opacity", 0.3);
                for (mut nodes_iterator, way_type) in paths.chain(park_paths) {
                    let v: Vec<String> = nodes_iterator
                        .into_iter()
                        .map(|(x, y)| format!("{},{}", x, y))
                        .collect();
                    match way_type {
                        WayType::Road { .. } => {
                            let mut polyline = Polyline::new().set("points", v.join(" "));
                            road_group = road_group.add(polyline);
                        }
                        WayType::River {
                            start_node_idx: _,
                            end_node_idx: _,
                            width,
                        } => {
                            let mut polyline = Polyline::new()
                                .set("points", v.join(" "))
                                .set("stroke-opacity", 0.8)
                                .set("stroke-width", t.transform_meters(width * 20));
                            river_group = river_group.add(polyline);
                        }
                        WayType::Park => {
                            let mut polyline = Polyline::new().set("points", v.join(" "));
                            park_group = park_group.add(polyline);
                        }
                        WayType::Lake => {
                            println!("Rendering Lake");
                            let mut polyline = Polyline::new().set("points", v.join(" "));
                            lake_group = lake_group.add(polyline);
                        }
                    }
                }

                document = document
                    .add(road_group)
                    .add(park_group)
                    .add(river_group)
                    .add(lake_group);
                svg::save(output_path, &document)?;
            }
            _ => bail!("File extension not supported."),
        },
        _ => bail!("Unable to guess format from file name (no extension)."),
    }
    Ok(())
}

fn main() -> Result<(), Error> {
    let args = Args::from_args();

    let storage = FileResourceStorage::new(args.flatdata_archive);
    let archive = osmflat::Osm::open(storage)?;
    render(&archive, &args.output, args.flag_width)?;
    Ok(())
}
