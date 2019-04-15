use bresenham::Bresenham;
use failure::Error;
use itertools::Itertools;
use osmflat::*;
use png::HasParameters;
use structopt::StructOpt;

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
impl<'a> From<osmflat::RefNode<'a>> for GeoCoord {
    fn from(node: osmflat::RefNode<'a>) -> Self {
        const COORD_SCALE: f64 = 1. / osmflat::COORD_SCALE as f64;
        Self {
            lat: node.lat() as f64 * COORD_SCALE,
            lon: node.lon() as f64 * COORD_SCALE,
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
    let first_coord = iter.next().unwrap_or(Default::default());
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
) -> impl FnMut(GeoCoord) -> (isize, isize) + Copy {
    move |coord: GeoCoord| {
        (
            ((coord.lon - min.lon) * width as f64 / (max.lon - min.lon)) as isize,
            ((max.lat - coord.lat) * height as f64 / (max.lat - min.lat)) as isize,
        )
    }
}

fn way_coords<'a>(
    archive: &'a osmflat::Osm,
    way: &osmflat::RefWay,
) -> impl Iterator<Item = GeoCoord> + 'a {
    let nodes = archive.nodes();
    let nodes_index = archive.nodes_index();
    way.refs()
        .map(move |i| nodes.at(nodes_index.at(i as usize).value() as usize).into())
}

fn way_filter(way: &osmflat::RefWay, archive: &osmflat::Osm) -> bool {
    let unwanted_highway_types = [
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

    // Filter all ways that do not have desirable highway tag.
    osmflat::tags(archive, way.tags())
        .filter_map(Result::ok)
        .any(|(key, val)| key == "highway" && !unwanted_highway_types.contains(&val))
}

fn roads<'a>(archive: &'a osmflat::Osm) -> impl Iterator<Item = osmflat::RefWay> {
    archive
        .ways()
        .iter()
        .filter(move |way| way_filter(&way, archive))
}

fn render(archive: &osmflat::Osm, width: u32) -> Image {
    // compute extent
    let coords = roads(archive).flat_map(|way| way_coords(archive, &way));
    let (min, max) = compute_bounds(coords);

    // compute ratio and height
    let ratio =
        (max.lat - min.lat) / (max.lon - min.lon) / (max.lat / 180. * std::f64::consts::PI).cos();
    let height = (width as f64 * ratio) as u32;

    // create world -> raster transformation
    let t = map_transform((width - 1, height - 1), (min, max));

    // draw
    let mut image = Image::new(width, height);

    let line_segments =
        roads(archive).flat_map(|way| way_coords(archive, &way).map(t).tuple_windows());

    for (from, to) in line_segments {
        for (x, y) in Bresenham::new(from, to) {
            image.set_black(x as u32, y as u32);
        }
    }

    image
}

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(parse(from_os_str))]
    input: PathBuf,

    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: PathBuf,

    #[structopt(short = "w", long = "width", default_value = "4320")]
    width: u32,
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();

    let archive = Osm::open(FileResourceStorage::new(opt.input))?;

    let image = render(&archive, opt.width);

    let buf = BufWriter::new(File::create(&opt.output)?);
    let mut encoder = png::Encoder::new(buf, image.w, image.h);
    encoder
        .set(png::ColorType::Grayscale)
        .set(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.data[..])?;

    Ok(())
}
