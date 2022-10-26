//! Renders selected features from the input archive as svg.
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
//!
//! LICENSE
//!
//! The code in this example file is released into the Public Domain.

use clap::Parser;
use osmflat::{iter_tags, FileResourceStorage, Node, Osm, Relation, RelationMembersRef, Way};
use smallvec::{smallvec, SmallVec};
use svg::{
    node::{self, element},
    Document,
};

use std::f64;
use std::fmt::Write;
use std::io;
use std::ops::Range;
use std::path::PathBuf;
use std::str;

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
impl GeoCoord {
    fn from_node(node: &Node, coord_scale: i32) -> Self {
        Self {
            lat: node.lat() as f64 / coord_scale as f64,
            lon: node.lon() as f64 / coord_scale as f64,
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
    #[allow(clippy::iter_overeager_cloned)]
    fn into_iter(self, archive: &Osm) -> Option<impl Iterator<Item = GeoCoord> + '_> {
        let nodes_index = archive.nodes_index();
        let nodes = archive.nodes();
        let mut indices = self.inner.iter().cloned().flatten();
        let scale = archive.header().coord_scale();
        if indices.any(|idx| nodes_index[idx as usize].value().is_none()) {
            None
        } else {
            let indices = self.inner.into_iter().flatten();
            Some(indices.map(move |idx| {
                GeoCoord::from_node(
                    &nodes[nodes_index[idx as usize].value().unwrap() as usize],
                    scale,
                )
            }))
        }
    }
}

/// Categories of features we support in this renderer.
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
    fn into_polyline(self, archive: &Osm) -> Option<Polyline> {
        match self.cat {
            Category::Road | Category::River(_) => {
                Some(way_into_polyline(&archive.ways()[self.idx]))
            }
            Category::Park | Category::Water => multipolygon_into_polyline(archive, self.idx),
        }
    }
}

fn way_into_polyline(way: &Way) -> Polyline {
    Polyline {
        inner: smallvec![way.refs()],
    }
}

fn multipolygon_into_polyline(archive: &Osm, idx: usize) -> Option<Polyline> {
    let members = archive.relation_members().at(idx);
    let strings = archive.stringtable();
    let ways = archive.ways();

    let inner: Option<SmallVec<[Range<u64>; 4]>> = members
        .filter_map(|m| match m {
            RelationMembersRef::WayMember(way_member)
                if strings.substring(way_member.role_idx() as usize) == Ok("outer") =>
            {
                Some(way_member.way_idx().map(|idx| ways[idx as usize].refs()))
            }
            _ => None,
        })
        .collect();
    inner.map(|inner| Polyline { inner })
}

/// Classifies all features from osmflat we want to render.
fn classify(archive: &Osm) -> impl Iterator<Item = Feature> + '_ {
    let ways = archive.ways().iter().enumerate();
    let ways = ways
        .filter_map(move |(idx, way)| classify_way(archive, way).map(|cat| Feature { idx, cat }));
    let rels = archive.relations().iter().enumerate();
    let rels = rels.filter_map(move |(idx, rel)| {
        classify_relation(archive, rel).map(|cat| Feature { idx, cat })
    });
    ways.chain(rels)
}

fn classify_way(archive: &Osm, way: &Way) -> Option<Category> {
    // Filter all ways that have less than 2 nodes.
    if way.refs().end <= way.refs().start + 2 {
        return None;
    }

    const UNWANTED_HIGHWAY_TYPES: [&[u8]; 9] = [
        b"pedestrian",
        b"steps",
        b"footway",
        b"construction",
        b"bic",
        b"cycleway",
        b"layby",
        b"bridleway",
        b"path",
    ];

    // Filter all ways that do not have a highway tag. Also check for specific
    // values.
    for (key, val) in iter_tags(archive, way.tags()) {
        if key == b"highway" {
            if UNWANTED_HIGHWAY_TYPES.contains(&val) {
                return None;
            }
            return Some(Category::Road);
        } else if key == b"waterway" {
            for (key, val) in iter_tags(archive, way.tags()) {
                if key == b"width" || key == b"maxwidth" {
                    let width: u32 = str::from_utf8(val).ok()?.parse().ok()?;
                    return Some(Category::River(width));
                }
            }
            return Some(Category::River(1));
        }
    }
    None
}

fn classify_relation(archive: &Osm, relation: &Relation) -> Option<Category> {
    let mut is_multipolygon = false;
    let mut is_park = false;
    let mut is_lake = false;

    for (key, val) in iter_tags(archive, relation.tags()) {
        if key == b"type" && val == b"multipolygon" {
            if is_park {
                return Some(Category::Park);
            }
            if is_lake {
                return Some(Category::Water);
            }
            is_multipolygon = true;
        }
        if (key == b"leisure" && val == b"park")
            || (key == b"landuse" && (val == b"recreation_ground" || val == b"forest"))
        {
            if is_multipolygon {
                return Some(Category::Park);
            }
            is_park = true;
        }
        if key == b"water" && val == b"lake" {
            if is_multipolygon {
                return Some(Category::Water);
            }
            is_lake = true;
        }
    }
    None
}

/// Renders svg from classified polylines.
fn render_svg<P>(
    archive: &Osm,
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
        let poly_iter = match poly.into_iter(archive) {
            Some(x) => x,
            None => continue,
        };
        for coord in poly_iter {
            // collect extent
            min_coord = min_coord.min(coord);
            max_coord = max_coord.max(coord);
            // accumulate polyline points
            write!(&mut points, "{:.5},{:.5} ", coord.lon, coord.lat)
                .expect("failed to write coordinates");
        }

        let polyline = element::Polyline::new().set("points", &points[..]);

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

    let style = element::Style::new(
        r#"
        text {
            font-family: arial;
            font-size: 8px;
            color: #001F3F;
            opacity: 0.3;
        }

        polyline {
            vector-effect: non-scaling-stroke;
        }
    "#,
    );

    let notice = element::Text::new()
        .set("x", width.saturating_sub(10))
        .set("y", height.saturating_sub(10))
        .set("text-anchor", "end")
        .add(node::Text::new("© OpenStreetMap Contributors"));

    document = document.add(style).add(transform).add(notice);
    svg::save(output, &document)
}

/// render map features as a SVG
#[derive(Debug, Parser)]
#[clap(name = "render-features")]
struct Args {
    /// osmflat archive
    osmflat_archive: PathBuf,

    /// SVG filename to output
    #[clap(long, short = 'o')]
    output: PathBuf,

    /// width of the image
    #[clap(long, short = 'w', default_value = "800")]
    width: u32,

    /// height of the image
    #[clap(long, short = 'h', default_value = "600")]
    height: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let storage = FileResourceStorage::new(args.osmflat_archive);
    let archive = Osm::open(storage)?;

    let features = classify(&archive);
    let archive_inner = archive.clone();
    let classified_polylines = features.filter_map(move |f| {
        let cat = f.cat;
        f.into_polyline(&archive_inner).map(|p| (p, cat))
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
