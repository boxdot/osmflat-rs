//! Renders all roads by using a simple Bresenham line algorithm.
//!
//! LICENSE
//!
//! The code in this example file is released into the Public Domain.

use osmflat::{find_tag_by, Archive, FileResourceStorage, Node, Osm, Way, COORD_SCALE};

use itertools::Itertools;
use structopt::StructOpt;

use std::f64::consts::PI;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

/// Geographic coordinates represented by (latitude, longitude).
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
struct GeoCoord {
    lat: f64,
    lon: f64,
}

/// Convert osmflat Node into GeoCoord.
impl From<&Node> for GeoCoord {
    fn from(node: &Node) -> Self {
        Self {
            lat: node.lat() as f64 / COORD_SCALE as f64,
            lon: node.lon() as f64 / COORD_SCALE as f64,
        }
    }
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
            data: vec![255; (w * h) as usize],
        }
    }

    fn set_black(&mut self, x: u32, y: u32) {
        self.data[(y * self.w + x) as usize] = 0;
    }
}

fn compute_bounds(mut iter: impl Iterator<Item = GeoCoord>) -> (GeoCoord, GeoCoord) {
    let first_coord = iter.next().unwrap_or_default();
    iter.fold((first_coord, first_coord), |(min, max), coord| {
        (
            GeoCoord {
                lat: min.lat.min(coord.lat),
                lon: min.lon.min(coord.lon),
            },
            GeoCoord {
                lat: max.lat.max(coord.lat),
                lon: max.lon.max(coord.lon),
            },
        )
    })
}

fn map_transform(
    (width, height): (u32, u32),
    (min, max): (GeoCoord, GeoCoord),
) -> impl FnMut(GeoCoord) -> (i32, i32) + Copy {
    move |coord: GeoCoord| {
        (
            ((coord.lon - min.lon) * f64::from(width) / (max.lon - min.lon)) as i32,
            ((max.lat - coord.lat) * f64::from(height) / (max.lat - min.lat)) as i32,
        )
    }
}

fn way_coords<'a>(archive: &'a Osm, way: &Way) -> Option<impl Iterator<Item = GeoCoord> + 'a> {
    let nodes = archive.nodes();
    let nodes_index = archive.nodes_index();
    let path = way.refs().map(move |i| &nodes_index[i as usize]);
    match path.clone().find(|node| node.value().is_none()) {
        Some(_) => None,
        None => Some(path.map(move |node| (&nodes[node.value().unwrap() as usize]).into())),
    }
}

fn way_filter(way: &Way, archive: &Osm) -> bool {
    const UNWANTED_HIGHWAY_TYPES: [&[u8]; 9] = [
        b"pedestrian\0",
        b"steps\0",
        b"footway\0",
        b"construction\0",
        b"bic\0",
        b"cycleway\0",
        b"layby\0",
        b"bridleway\0",
        b"path\0",
    ];

    // Filter all ways that do not have desirable highway tag.
    find_tag_by(archive, way.tags(), |key_block, val_block| {
        key_block.starts_with(b"highway\0")
            && !UNWANTED_HIGHWAY_TYPES
                .iter()
                .any(|t| val_block.starts_with(t))
    })
    .is_some()
}

fn roads(archive: &Osm) -> impl Iterator<Item = &Way> {
    archive
        .ways()
        .iter()
        .filter(move |&way| way_filter(way, archive))
}

/// Bresenham's line algorithm
///
/// https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm
fn bresenham(mut x0: i32, mut y0: i32, x1: i32, y1: i32) -> impl Iterator<Item = (i32, i32)> {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    std::iter::from_fn(move || {
        if x0 == x1 && y0 == y1 {
            return None;
        }
        let res = (x0, y0);
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
        Some(res)
    })
}

fn render(archive: &Osm, width: u32) -> Image {
    // compute extent
    let coords = roads(archive)
        .filter_map(|way| way_coords(archive, way))
        .flatten();
    let (min, max) = compute_bounds(coords);

    // compute ratio and height
    let ratio = (max.lat - min.lat) / (max.lon - min.lon) / (max.lat / 180. * PI).cos();
    let height = (f64::from(width) * ratio) as u32;

    // create world -> raster transformation
    let t = map_transform((width - 1, height - 1), (min, max));

    // draw
    let mut image = Image::new(width, height);

    let line_segments = roads(archive)
        .filter_map(|way| Some(way_coords(archive, way)?.map(t).tuple_windows()))
        .flatten();

    for ((x0, y0), (x1, y1)) in line_segments {
        for (x, y) in bresenham(x0, y0, x1, y1) {
            image.set_black(x as u32, y as u32);
        }
    }

    image
}

#[derive(StructOpt, Debug)]
struct Args {
    /// Input osmflat archive
    #[structopt(parse(from_os_str))]
    input: PathBuf,
    /// Output PNG filename
    #[structopt(short, long, parse(from_os_str))]
    output: PathBuf,
    /// Width of the image (height is derived from ratio)
    #[structopt(short, long, default_value = "4320")]
    width: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::from_args();

    let archive = Osm::open(FileResourceStorage::new(args.input))?;

    let image = render(&archive, args.width);

    let buf = BufWriter::new(File::create(&args.output)?);
    let mut encoder = png::Encoder::new(buf, image.w, image.h);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.data[..])?;

    Ok(())
}
