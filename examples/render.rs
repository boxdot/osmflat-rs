extern crate flatdata;
extern crate itertools;
extern crate osmflat;
extern crate structopt;
extern crate svg;

use flatdata::{Archive, FileResourceStorage};
use structopt::StructOpt;
use svg::node::element;
use svg::Document;

use std::f64;
use std::io;
use std::ops::Range;
use std::path::PathBuf;
use std::str;

/// Helper function to read a string from osmflat.
fn substring(strings: &str, start: u64) -> &str {
    let start = start as usize;
    let end = strings[start..].find('\0').expect("invalid string");
    &strings[start..start + end]
}

#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
struct GeoCoord {
    lat: f64,
    lon: f64,
}

impl Into<String> for GeoCoord {
    fn into(self) -> String {
        // Not, we need to revert lat/lon, since lat cooresponds to y-axis and lon to
        // x-axis.
        format!("{:.5},{:.5}", self.lon, self.lat)
    }
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

impl<'a> From<osmflat::RefNode<'a>> for GeoCoord {
    fn from(node: osmflat::RefNode<'a>) -> Self {
        const COORD_SCALE: f64 = 0.000000001;
        Self {
            lat: node.lat() as f64 * COORD_SCALE,
            lon: node.lon() as f64 * COORD_SCALE,
        }
    }
}

struct Polyline {
    inner: Vec<Range<u64>>, // TODO: use small vec optimization
}

impl From<Range<u64>> for Polyline {
    fn from(range: Range<u64>) -> Self {
        Self { inner: vec![range] }
    }
}

impl Polyline {
    fn into_iter(self, archive: osmflat::Osm) -> PolylineIter {
        let next_element = self.inner.first().map(|range| range.start).unwrap_or(0);
        PolylineIter {
            archive,
            inner: self.inner,
            next_range: 0,
            next_element,
        }
    }
}

/// Iterator over osmflat nodes.
///
/// Polyline contains ranges of nodes. This iterator iterates over the ranges
/// and resolves nodes as GeoCoord's.
struct PolylineIter {
    archive: osmflat::Osm,
    inner: Vec<Range<u64>>,
    next_range: usize,
    next_element: u64,
}

impl Iterator for PolylineIter {
    type Item = GeoCoord;
    fn next(&mut self) -> Option<GeoCoord> {
        if self.next_range < self.inner.len() {
            let range = &self.inner[self.next_range];
            if self.next_element < range.end {
                let node_idx = self
                    .archive
                    .nodes_index()
                    .at(self.next_element as usize)
                    .value();
                let coord = self.archive.nodes().at(node_idx as usize).into();
                self.next_element += 1;
                if self.next_element == range.end {
                    self.next_range += 1;
                    self.next_element = self
                        .inner
                        .get(self.next_range)
                        .map(|range| range.start)
                        .unwrap_or(0);
                }
                Some(coord)
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Category {
    Road,
    Park,
    River(u32), // River with width
    Water,
}

struct Feature {
    idx: u64,
    cat: Category,
}

impl Feature {
    fn into_polyline(self, archive: osmflat::Osm) -> Polyline {
        match self.cat {
            Category::Road | Category::River(_) => way_into_polyline(archive, self.idx),
            Category::Park | Category::Water => multipolygon_into_polyline(archive, self.idx),
        }
    }
}

fn way_into_polyline(archive: osmflat::Osm, idx: u64) -> Polyline {
    let way = archive.ways().at(idx as usize);
    let next_way = archive.ways().at(idx as usize + 1);
    let first_node_idx = way.ref_first_idx();
    let last_node_idx = next_way.ref_first_idx();
    Polyline {
        inner: vec![first_node_idx..last_node_idx],
    }
}

fn multipolygon_into_polyline(archive: osmflat::Osm, idx: u64) -> Polyline {
    let members = archive.relation_members().at(idx as usize);
    let strings = unsafe { str::from_utf8_unchecked(archive.stringtable()) };
    let ways = archive.ways();

    let inner = members
        .filter_map(|m| match m {
            osmflat::RefRelationMembers::WayMember(way_member) => {
                let role = substring(strings, way_member.role_idx());
                if role == "outer" {
                    let way_idx = way_member.way_idx();
                    let way = ways.at(way_idx as usize);
                    let next_way = ways.at(way_idx as usize + 1);
                    Some(way.ref_first_idx()..next_way.ref_first_idx())
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();
    Polyline { inner }
}

fn classify(archive: osmflat::Osm) -> impl Iterator<Item = Feature> {
    let inner_archive = archive.clone();
    let ways = (0..archive.ways().len() as u64 - 2).filter_map(move |idx| {
        classify_way(inner_archive.clone(), idx).map(|cat| Feature { idx, cat })
    });
    let rels = (0..archive.relations().len() as u64 - 2).filter_map(move |idx| {
        classify_relation(archive.clone(), idx).map(|cat| Feature { idx, cat })
    });
    ways.chain(rels)
}

fn classify_way(archive: osmflat::Osm, idx: u64) -> Option<Category> {
    let way = archive.ways().at(idx as usize);
    let next_way = archive.ways().at(idx as usize + 1);
    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let strings = unsafe { str::from_utf8_unchecked(archive.stringtable()) };

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
            return Some(Category::Road);
        } else if key == "waterway" {
            let key = substring(strings, tag.key_idx());
            if key == "width" || key == "maxwidth" {
                let val = substring(strings, tag.value_idx());
                let width: u32 = val.parse().ok()?;
                return Some(Category::River(width));
            }
            return Some(Category::River(1));
        }
    }
    None
}

fn classify_relation(archive: osmflat::Osm, idx: u64) -> Option<Category> {
    let relation = archive.relations().at(idx as usize);
    let next_relation = archive.relations().at(idx as usize + 1);

    let start_tag_idx = relation.tag_first_idx();
    let end_tag_idx = next_relation.tag_first_idx();

    let mut is_multipolygon = false;
    let mut is_park = false;
    let mut is_lake = false;

    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let strings = unsafe { str::from_utf8_unchecked(archive.stringtable()) };

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

        if is_multipolygon {
            if is_park {
                return Some(Category::Park);
            } else if is_lake {
                return Some(Category::Water);
            }
        }
    }
    None
}

fn render_svg<P>(
    archive: osmflat::Osm,
    classified_polylines: P,
    output: PathBuf,
    width: u32,
    height: u32,
) -> Result<(), io::Error>
where
    P: Iterator<Item = (Polyline, Category)>,
{
    let mut document = Document::new().set("viewBox", (0, 0, width, height));
    let mut road_group = element::Group::new()
        .set("stroke", "#001F3F")
        .set("stroke-width", "0.3")
        .set("fill", "none");
    let mut park_group = element::Group::new()
        .set("stroke", "#3D9970")
        .set("fill", "#3D9970")
        .set("fill-opacity", 0.3);
    let mut river_group = element::Group::new()
        .set("stroke", "#0074D9")
        .set("fill", "none")
        .set("stroke-opacity", 0.8);
    let mut lake_group = element::Group::new()
        .set("stroke", "#0074D9")
        .set("fill", "#0074D9")
        .set("fill-opacity", 0.3);

    let mut min_coord = GeoCoord {
        lat: f64::MAX,
        lon: f64::MAX,
    };
    let mut max_coord = GeoCoord {
        lat: f64::MIN,
        lon: f64::MIN,
    };

    for (poly, cat) in classified_polylines {
        // TODO: Use itertools to avoid creation of intermediate vector and strings.
        let poly: Vec<String> = poly
            .into_iter(archive.clone())
            .map(|coord| {
                // collect extent
                min_coord = min_coord.min(coord);
                max_coord = max_coord.max(coord);
                coord.into()
            })
            .collect();
        let mut polyline = element::Polyline::new()
            .set("points", poly.join(" "))
            .set("vector-effect", "non-scaling-stroke");

        match cat {
            Category::Road => {
                road_group = road_group.add(polyline);
            }
            Category::River(width) => {
                river_group = river_group.add(polyline).set("stroke-width", width);
            }
            Category::Park => {
                park_group = park_group.add(polyline);
            }
            Category::Water => {
                lake_group = lake_group.add(polyline);
            }
        }
    }

    let mut transform = element::Group::new().set(
        "transform",
        format!(
            "scale({:.5} {:.5}) translate({:.5} {:.5})",
            width as f64 / (max_coord.lon - min_coord.lon),
            -1. * (height as f64) / (max_coord.lat - min_coord.lat),
            -min_coord.lon,
            -max_coord.lat,
        ),
    );

    transform = transform
        .add(road_group)
        .add(river_group)
        .add(lake_group)
        .add(park_group);
    document = document.add(transform);
    svg::save(output, &document)
}

#[derive(Debug, StructOpt)]
#[structopt(name = "osmflat-render")]
struct Args {
    /// Osmflat archive
    #[structopt(parse(from_os_str))]
    osmflat_archive: PathBuf,

    /// SVG filename to output
    #[structopt(parse(from_os_str))]
    output: PathBuf,

    /// Width of the image
    #[structopt(short = "w", long = "width", default_value = "800")]
    width: u32,

    /// Height of the image
    #[structopt(short = "h", long = "height", default_value = "600")]
    height: u32,
}

fn main() -> Result<(), Box<std::error::Error>> {
    let args = Args::from_args();

    let storage = FileResourceStorage::new(args.osmflat_archive);
    let archive = osmflat::Osm::open(storage)?;

    let features = classify(archive.clone());
    let archive_inner = archive.clone();
    let classified_polylines = features.map(move |f| {
        let cat = f.cat;
        (f.into_polyline(archive_inner.clone()), cat)
    });
    render_svg(
        archive,
        classified_polylines,
        args.output,
        args.width,
        args.height,
    )?;
    Ok(())
}
