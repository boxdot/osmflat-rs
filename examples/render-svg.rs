//! Example which renders selected features from a given osmflat archive into a
//! svg.
//!
//! For supported features check `Category` enum and `classify` function.
//!
//! For each feature, we retrieve the coordinates lazily from osm nodes, and
//! then produce polylines styled based on the category, cf. `render_svg`
//! function. The coordinates are in lon, lat.
//!
//! Inside of svg we just use the coordinates as is (except for swapped x/y
//! axes), plus we apply a transformation to adjust the coordinates to the
//! viewport. Obviously, it is slower the render such svg on the screen.
//! However, the final svg contains already so many polyline, that having alrady
//! transformed coordinates does not change much. If you need speed when showing
//! the svg, feel free to apply simplifications in this program.

use flatdata::{Archive, FileResourceStorage};
use smallvec::{smallvec, SmallVec};
use structopt::StructOpt;
use svg::{node::element, Document};

use std::f64;
use std::fmt::Write;
use std::io;
use std::ops::Range;
use std::path::PathBuf;

/// Geographic coordinates represented by (latitude, longitude).
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

/// Convert osmflat Node into GeoCoord.
impl From<osmflat::RefNode<'_>> for GeoCoord {
    fn from(node: osmflat::RefNode) -> Self {
        Self {
            lat: node.lat() as f64 / osmflat::COORD_SCALE as f64,
            lon: node.lon() as f64 / osmflat::COORD_SCALE as f64,
        }
    }
}

/// Polyline which can be transformed into an iterator over `GeoCoord`'s.
struct Polyline {
    inner: SmallVec<[Range<u64>; 4]>,
}

impl From<Range<u64>> for Polyline {
    fn from(range: Range<u64>) -> Self {
        Self {
            inner: smallvec![range],
        }
    }
}

impl Polyline {
    fn into_iter<'a>(self, archive: &'a osmflat::Osm) -> impl Iterator<Item = GeoCoord> + 'a {
        let nodes_index = archive.nodes_index();
        let nodes = archive.nodes();
        let to_node = move |idx| {
            let node_idx = nodes_index.at(idx as usize).value();
            nodes.at(node_idx as usize).into()
        };
        self.inner.into_iter().flatten().map(to_node)
    }
}

// Categories of features we support in this renderer.
#[derive(Debug, Clone, Copy)]
enum Category {
    Road,
    Park,
    River(u32), // River with width
    Water,
}

/// Feature in osmflat.
///
/// Idx points either into ways or relations, depending on the `Category`.
struct Feature {
    idx: usize,
    cat: Category,
}

impl Feature {
    fn into_polyline(self, archive: &osmflat::Osm) -> Polyline {
        match self.cat {
            Category::Road | Category::River(_) => way_into_polyline(archive.ways().at(self.idx)),
            Category::Park | Category::Water => multipolygon_into_polyline(&archive, self.idx),
        }
    }
}

fn way_into_polyline(way: osmflat::RefWay) -> Polyline {
    Polyline {
        inner: smallvec![way.refs()],
    }
}

fn multipolygon_into_polyline(archive: &osmflat::Osm, idx: usize) -> Polyline {
    let members = archive.relation_members().at(idx);
    let strings = archive.stringtable();

    let inner = members
        .filter_map(|m| match m {
            osmflat::RefRelationMembers::WayMember(way_member)
                if strings.substring(way_member.role_idx() as usize) == Ok("outer") =>
            {
                Some(archive.ways().at(way_member.way_idx() as usize).refs())
            }
            _ => None,
        })
        .collect();
    Polyline { inner }
}

/// Classifies all features from osmflat we want to render.
fn classify<'a>(archive: &'a osmflat::Osm) -> impl Iterator<Item = Feature> + 'a {
    let ways = archive.ways().iter().enumerate();
    let ways = ways
        .filter_map(move |(idx, way)| classify_way(archive, way).map(|cat| Feature { idx, cat }));
    let rels = archive.relations().iter().enumerate();
    let rels = rels.filter_map(move |(idx, rel)| {
        classify_relation(archive, rel).map(|cat| Feature { idx, cat })
    });
    ways.chain(rels)
}

fn classify_way(archive: &osmflat::Osm, way: osmflat::RefWay) -> Option<Category> {
    // Filter all ways that have less than 2 nodes.
    if way.refs().start < way.refs().end + 2 {
        return None;
    }

    const UNWANTED_HIGHWAY_TYPES: [&str; 9] = [
        "pedestrian",
        "steps",
        "footway",
        "construction",
        "bic",
        "cycleway",
        "layby",
        "bridleway",
        "path",
    ];

    // Filter all ways that do not have a highway tag. Also check for specific values.
    for (key, val) in osmflat::tags(archive, way.tags()).filter_map(Result::ok) {
        if key == "highway" {
            if UNWANTED_HIGHWAY_TYPES.contains(&val) {
                return None;
            }
            return Some(Category::Road);
        } else if key == "waterway" {
            for (key, val) in osmflat::tags(archive, way.tags()).filter_map(Result::ok) {
                if key == "width" || key == "maxwidth" {
                    let width: u32 = val.parse().ok()?;
                    return Some(Category::River(width));
                }
            }
            return Some(Category::River(1));
        }
    }
    None
}

fn classify_relation(archive: &osmflat::Osm, relation: osmflat::RefRelation) -> Option<Category> {
    let mut is_multipolygon = false;
    let mut is_park = false;
    let mut is_lake = false;

    for (key, val) in osmflat::tags(archive, relation.tags()).filter_map(Result::ok) {
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
            return Some(Category::Park);
        }
        if is_lake {
            return Some(Category::Water);
        }
    }
    None
}

/// Renders svg from classified polylines.
fn render_svg<P>(
    archive: &osmflat::Osm,
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

    let mut points = String::new(); // reuse string buffer inside the for-loop
    for (poly, cat) in classified_polylines {
        points.clear();
        for coord in poly.into_iter(archive) {
            // collect extent
            min_coord = min_coord.min(coord);
            max_coord = max_coord.max(coord);
            // accumulate polyline points
            write!(&mut points, "{:.5},{:.5} ", coord.lon, coord.lat)
                .expect("failed to write coordinates");
        }

        let polyline = element::Polyline::new()
            .set("points", &points[..])
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
            "scale({:.5} {:.5}) translate({:.5} {:.5})", /* Note: svg transformations are
                                                          * applied from right to left */
            f64::from(width) / (max_coord.lon - min_coord.lon),
            f64::from(height) / (min_coord.lat - max_coord.lat), // invert y-axis
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
    #[structopt(short = "o", long = "output", parse(from_os_str))]
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

    let features = classify(&archive);
    let archive_inner = archive.clone();
    let classified_polylines = features.map(move |f| {
        let cat = f.cat;
        (f.into_polyline(&archive_inner), cat)
    });
    render_svg(
        &archive,
        classified_polylines,
        args.output,
        args.width,
        args.height,
    )?;
    Ok(())
}
